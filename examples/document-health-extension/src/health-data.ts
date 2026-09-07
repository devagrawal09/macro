import type { Macro } from "@macro/sdk/browser";
import {
	DOCUMENT_HEALTH_PROPERTY,
	type DocumentHealth,
	parseDocumentHealth,
} from "./score";

/**
 * Read the canonical Document Health snapshot stored by the server workflow.
 * Returns `undefined` while the workflow has not stored a valid snapshot yet.
 */
export async function fetchDocumentHealthSnapshot(
	macro: Macro,
	documentId: string,
): Promise<DocumentHealth | undefined> {
	const properties = await macro.documents.byId(documentId).properties();
	const stored = properties.find(
		({ definition }) => definition.display_name === DOCUMENT_HEALTH_PROPERTY,
	);
	return parseDocumentHealth(
		stored?.value?.type === "String" ? stored.value.value : undefined,
	);
}

/** Backoff used while waiting for the workflow's write to land. */
export const DEFAULT_RETRY_DELAYS_MS: readonly number[] = [400, 800, 1600, 3200];

export interface SubscribeToDocumentHealthOptions {
	macro: Macro;
	documentId: string;
	/** The snapshot the caller currently displays, if any. */
	current: () => DocumentHealth | undefined;
	/** Called once a refetch returns a snapshot with a different content hash. */
	onSnapshot: (snapshot: DocumentHealth | undefined) => void;
	/** Called once if the upstream SSE stream cannot be established. */
	onStreamError?: (error: unknown) => void;
	/** Called when a refetch fails; convergence for that event stops. */
	onFetchError?: (error: unknown) => void;
	/** Delays between refetches. Defaults to {@link DEFAULT_RETRY_DELAYS_MS}. */
	retryDelaysMs?: readonly number[];
}

/**
 * Follow `document.updated` for one document over the upstream
 * `macro.events.listen()` SSE transport and converge on the workflow's write.
 *
 * The event that reaches the browser is the same one that triggers the server
 * workflow, so the property is usually not written yet when it arrives. Rather
 * than a fixed timer, this refetches with backoff until the stored snapshot's
 * `contentHash` differs from the one currently displayed, then stops. If no
 * refetch shows a change (for example a rename that did not touch content),
 * it gives up after the last delay.
 *
 * A new event restarts convergence. The returned stop function is synchronous
 * and idempotent, and closes the stream even if it opens after stop is called.
 */
export function subscribeToDocumentHealth(
	options: SubscribeToDocumentHealthOptions,
): () => void {
	const { macro, documentId } = options;
	const delays = options.retryDelaysMs ?? DEFAULT_RETRY_DELAYS_MS;

	let disposed = false;
	let version = 0;
	let pending: ReturnType<typeof setTimeout> | undefined;
	let stopListening: (() => void) | undefined;

	const converge = async () => {
		const run = ++version;
		clearTimeout(pending);
		const before = options.current()?.contentHash;

		for (let attempt = 0; ; attempt++) {
			let snapshot: DocumentHealth | undefined;
			try {
				snapshot = await fetchDocumentHealthSnapshot(macro, documentId);
			} catch (error) {
				if (!disposed && run === version) options.onFetchError?.(error);
				return;
			}
			if (disposed || run !== version) return;
			if (snapshot?.contentHash !== before) {
				options.onSnapshot(snapshot);
				return;
			}
			const delay = delays[attempt];
			if (delay === undefined) return;
			await new Promise<void>((resolve) => {
				pending = setTimeout(resolve, delay);
			});
			if (disposed || run !== version) return;
		}
	};

	const unsubscribe = macro.events.on("document.updated", (event) => {
		if (disposed || event.document.id !== documentId) return;
		void converge();
	});

	void macro.events
		.listen({ filters: [{ events: ["document.updated"], ids: [documentId] }] })
		.then((stop) => {
			if (disposed) stop();
			else stopListening = stop;
		})
		.catch((error) => {
			if (!disposed) options.onStreamError?.(error);
		});

	return () => {
		if (disposed) return;
		disposed = true;
		version += 1;
		clearTimeout(pending);
		unsubscribe();
		stopListening?.();
	};
}
