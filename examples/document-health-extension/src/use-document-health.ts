import type { Macro } from "@macro/sdk/browser";
import { createEffect, createResource, createSignal, onCleanup } from "solid-js";
import {
	fetchDocumentHealthSnapshot,
	subscribeToDocumentHealth,
} from "./health-data";
import type { DocumentHealth } from "./score";

export interface DocumentHealthSource {
	macro: Macro;
	documentId: string;
}

/**
 * Solid primitive shared by every placement: loads the canonical snapshot,
 * follows `document.updated` over SSE, and converges on the workflow's write.
 */
export function createDocumentHealth(props: DocumentHealthSource) {
	const [snapshot, { refetch, mutate }] = createResource(
		() => props.documentId,
		(documentId) => fetchDocumentHealthSnapshot(props.macro, documentId),
	);
	const [streamDown, setStreamDown] = createSignal(false);
	const [fetchError, setFetchError] = createSignal<unknown>();

	createEffect(() => {
		const documentId = props.documentId;
		setStreamDown(false);
		setFetchError(undefined);
		const stop = subscribeToDocumentHealth({
			macro: props.macro,
			documentId,
			current: () => (snapshot.error ? undefined : snapshot.latest),
			onSnapshot: (next) => {
				setFetchError(undefined);
				mutate(() => next);
			},
			onStreamError: (error) => {
				console.error("Document Health event stream failed", error);
				setStreamDown(true);
			},
			onFetchError: setFetchError,
		});
		onCleanup(stop);
	});

	const error = (): unknown => snapshot.error ?? fetchError();

	return {
		/** The stored snapshot; `undefined` until the workflow writes one. */
		snapshot: (): DocumentHealth | undefined =>
			snapshot.error ? undefined : snapshot(),
		loading: () => snapshot.loading,
		error,
		errorMessage: () => {
			const value = error();
			if (value === undefined) return undefined;
			return value instanceof Error
				? value.message
				: "Document Health is unavailable";
		},
		streamDown,
		refresh: () => void refetch(),
	};
}
