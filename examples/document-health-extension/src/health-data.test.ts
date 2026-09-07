import { describe, expect, test } from "bun:test";
import type { Macro } from "@macro/sdk/browser";
import {
	fetchDocumentHealthSnapshot,
	subscribeToDocumentHealth,
} from "./health-data";
import {
	analyzeDocument,
	DOCUMENT_HEALTH_PROPERTY,
	type DocumentHealth,
} from "./score";

type UpdateHandler = (event: { document: { id: string } }) => void;

function createMacroStub(options: {
	/** Snapshots returned by successive property reads; the last one repeats. */
	snapshots?: Array<DocumentHealth | undefined>;
	listen?: () => Promise<() => void>;
}) {
	const handlers: UpdateHandler[] = [];
	const snapshots = options.snapshots ?? [];
	let reads = 0;
	let unsubscribed = 0;
	const macro = {
		documents: {
			byId: () => ({
				properties: async () => {
					const snapshot = snapshots[Math.min(reads, snapshots.length - 1)];
					reads += 1;
					return snapshot
						? [
								{
									definition: { display_name: DOCUMENT_HEALTH_PROPERTY },
									value: { type: "String", value: JSON.stringify(snapshot) },
								},
							]
						: [];
				},
			}),
		},
		events: {
			on: (_event: string, handler: UpdateHandler) => {
				handlers.push(handler);
				return () => {
					unsubscribed += 1;
				};
			},
			listen: options.listen ?? (async () => () => {}),
		},
	} as unknown as Macro;
	return {
		macro,
		emit: (documentId: string) => {
			for (const handler of handlers) handler({ document: { id: documentId } });
		},
		reads: () => reads,
		unsubscribedCount: () => unsubscribed,
	};
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 5));

describe("fetchDocumentHealthSnapshot", () => {
	test("parses the stored Document Health string property", async () => {
		const snapshot = analyzeDocument("TODO: ship it");
		const { macro } = createMacroStub({ snapshots: [snapshot] });
		expect(await fetchDocumentHealthSnapshot(macro, "doc")).toEqual(snapshot);
	});

	test("returns undefined when the workflow has not written yet", async () => {
		const { macro } = createMacroStub({ snapshots: [undefined] });
		expect(await fetchDocumentHealthSnapshot(macro, "doc")).toBeUndefined();
	});
});

describe("subscribeToDocumentHealth", () => {
	test("refetches until the stored content hash changes", async () => {
		const before = analyzeDocument("first draft");
		const after = analyzeDocument("second draft");
		const stub = createMacroStub({ snapshots: [before, before, after] });
		const received: Array<DocumentHealth | undefined> = [];

		const stop = subscribeToDocumentHealth({
			macro: stub.macro,
			documentId: "doc",
			current: () => before,
			onSnapshot: (snapshot) => received.push(snapshot),
			retryDelaysMs: [0, 0, 0],
		});
		stub.emit("doc");
		await settle();

		expect(received).toEqual([after]);
		expect(stub.reads()).toBe(3);
		stop();
	});

	test("gives up after the last delay when nothing changed", async () => {
		const same = analyzeDocument("unchanged");
		const stub = createMacroStub({ snapshots: [same] });
		const received: unknown[] = [];

		const stop = subscribeToDocumentHealth({
			macro: stub.macro,
			documentId: "doc",
			current: () => same,
			onSnapshot: (snapshot) => received.push(snapshot),
			retryDelaysMs: [0],
		});
		stub.emit("doc");
		await settle();

		expect(received).toEqual([]);
		expect(stub.reads()).toBe(2);
		stop();
	});

	test("ignores events for other documents", async () => {
		const stub = createMacroStub({ snapshots: [analyzeDocument("x")] });
		const stop = subscribeToDocumentHealth({
			macro: stub.macro,
			documentId: "doc",
			current: () => undefined,
			onSnapshot: () => {},
			retryDelaysMs: [],
		});
		stub.emit("other");
		await settle();
		expect(stub.reads()).toBe(0);
		stop();
	});

	test("stop before listen resolves still closes the stream", async () => {
		let stopped = 0;
		let resolveListen: (stop: () => void) => void = () => {};
		const stub = createMacroStub({
			listen: () =>
				new Promise((resolve) => {
					resolveListen = resolve;
				}),
		});

		const stop = subscribeToDocumentHealth({
			macro: stub.macro,
			documentId: "doc",
			current: () => undefined,
			onSnapshot: () => {},
		});
		stop();
		stop();
		resolveListen(() => {
			stopped += 1;
		});
		await settle();

		expect(stopped).toBe(1);
		expect(stub.unsubscribedCount()).toBe(1);
	});

	test("reports a stream that cannot be established", async () => {
		const errors: unknown[] = [];
		const stub = createMacroStub({
			listen: async () => {
				throw new Error("offline");
			},
		});
		const stop = subscribeToDocumentHealth({
			macro: stub.macro,
			documentId: "doc",
			current: () => undefined,
			onSnapshot: () => {},
			onStreamError: (error) => errors.push(error),
		});
		await settle();
		expect(errors).toHaveLength(1);
		stop();
	});
});
