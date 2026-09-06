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

export interface SubscribeToDocumentHealthOptions {
	macro: Macro;
	documentId: string;
	/**
	 * Called when the document changed and the canonical snapshot should be
	 * refetched. Fired immediately for each matching event, then once more
	 * after `refreshDelayMs` in case the workflow write races the event.
	 */
	onChange: () => void;
	/** Called once if the upstream SSE stream cannot be established. */
	onStreamError?: (error: unknown) => void;
	/** Delay for the trailing refetch. Defaults to 750ms. */
	refreshDelayMs?: number;
}

/**
 * Subscribe to `document.updated` for one document over the upstream
 * `macro.events.listen()` SSE transport.
 *
 * Handles the async mount/unmount race: calling the returned stop function
 * before `listen()` resolves still closes the stream once it opens.
 * The stop function is synchronous and idempotent.
 */
export function subscribeToDocumentHealth(
	options: SubscribeToDocumentHealthOptions,
): () => void {
	const { macro, documentId, onChange } = options;
	const refreshDelayMs = options.refreshDelayMs ?? 750;

	let disposed = false;
	let delayedRefresh: ReturnType<typeof setTimeout> | undefined;
	let stopListening: (() => void) | undefined;

	const unsubscribe = macro.events.on("document.updated", (event) => {
		if (disposed || event.document.id !== documentId) return;
		onChange();
		clearTimeout(delayedRefresh);
		delayedRefresh = setTimeout(() => {
			if (!disposed) onChange();
		}, refreshDelayMs);
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
		clearTimeout(delayedRefresh);
		unsubscribe();
		stopListening?.();
	};
}
