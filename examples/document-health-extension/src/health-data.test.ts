import { describe, expect, test } from "bun:test";
import type { Macro } from "@macro/sdk/browser";
import {
	fetchDocumentHealthSnapshot,
	subscribeToDocumentHealth,
} from "./health-data";
import { analyzeDocument, DOCUMENT_HEALTH_PROPERTY } from "./score";

type UpdateHandler = (event: { document: { id: string } }) => void;

function createMacroStub(options: {
	properties?: Array<{
		definition: { display_name: string };
		value?: { type: string; value: unknown };
	}>;
	listen?: () => Promise<() => void>;
}) {
	const handlers: UpdateHandler[] = [];
	let unsubscribed = 0;
	const macro = {
		documents: {
			byId: () => ({
				properties: async () => options.properties ?? [],
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
		unsubscribedCount: () => unsubscribed,
	};
}

describe("fetchDocumentHealthSnapshot", () => {
	test("parses the stored Document Health string property", async () => {
		const snapshot = analyzeDocument("TODO: ship it");
		const { macro } = createMacroStub({
			properties: [
				{
					definition: { display_name: "Other" },
					value: { type: "String", value: "junk" },
				},
				{
					definition: { display_name: DOCUMENT_HEALTH_PROPERTY },
					value: { type: "String", value: JSON.stringify(snapshot) },
				},
			],
		});

		expect(await fetchDocumentHealthSnapshot(macro, "doc_1")).toEqual(snapshot);
	});

	test("returns undefined for a missing or non-string property", async () => {
		const missing = createMacroStub({ properties: [] });
		expect(await fetchDocumentHealthSnapshot(missing.macro, "doc_1")).toBe(
			undefined,
		);

		const wrongType = createMacroStub({
			properties: [
				{
					definition: { display_name: DOCUMENT_HEALTH_PROPERTY },
					value: { type: "Number", value: 42 },
				},
			],
		});
		expect(await fetchDocumentHealthSnapshot(wrongType.macro, "doc_1")).toBe(
			undefined,
		);
	});
});

describe("subscribeToDocumentHealth", () => {
	test("notifies for matching events only, with a trailing refetch", async () => {
		const stub = createMacroStub({});
		let changes = 0;
		const stop = subscribeToDocumentHealth({
			macro: stub.macro,
			documentId: "doc_1",
			onChange: () => {
				changes += 1;
			},
			refreshDelayMs: 1,
		});

		stub.emit("doc_other");
		expect(changes).toBe(0);

		stub.emit("doc_1");
		expect(changes).toBe(1);
		await new Promise((resolve) => setTimeout(resolve, 10));
		expect(changes).toBe(2);

		stop();
		stub.emit("doc_1");
		expect(changes).toBe(2);
		expect(stub.unsubscribedCount()).toBe(1);
	});

	test("closes a stream that resolves after the subscription stopped", async () => {
		let streamStops = 0;
		let resolveListen: ((stop: () => void) => void) | undefined;
		const stub = createMacroStub({
			listen: () =>
				new Promise<() => void>((resolve) => {
					resolveListen = resolve;
				}),
		});

		const stop = subscribeToDocumentHealth({
			macro: stub.macro,
			documentId: "doc_1",
			onChange: () => {},
		});
		stop();
		stop();

		resolveListen?.(() => {
			streamStops += 1;
		});
		await Promise.resolve();
		expect(streamStops).toBe(1);
		expect(stub.unsubscribedCount()).toBe(1);
	});

	test("reports stream failures without throwing, unless already stopped", async () => {
		const failure = new Error("no stream");
		const failing = createMacroStub({
			listen: async () => {
				throw failure;
			},
		});
		const errors: unknown[] = [];
		const stop = subscribeToDocumentHealth({
			macro: failing.macro,
			documentId: "doc_1",
			onChange: () => {},
			onStreamError: (error) => errors.push(error),
		});
		await Promise.resolve();
		await Promise.resolve();
		expect(errors).toEqual([failure]);
		stop();

		const failingLate = createMacroStub({
			listen: async () => {
				throw failure;
			},
		});
		const lateErrors: unknown[] = [];
		subscribeToDocumentHealth({
			macro: failingLate.macro,
			documentId: "doc_1",
			onChange: () => {},
			onStreamError: (error) => lateErrors.push(error),
		})();
		await Promise.resolve();
		await Promise.resolve();
		expect(lateErrors).toEqual([]);
	});
});
