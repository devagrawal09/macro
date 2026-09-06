import type { Macro } from "@macro/sdk/browser";
import { createEffect, createResource, For, onCleanup, Show } from "solid-js";
import {
	fetchDocumentHealthSnapshot,
	subscribeToDocumentHealth,
} from "./health-data";

export interface DocumentHealthPluginProps {
	macro: Macro;
	documentId: string;
}

/** Document Health rendered as an in-process Solid plugin. */
export default function DocumentHealthPlugin(props: DocumentHealthPluginProps) {
	const [health, { refetch }] = createResource(
		() => props.documentId,
		async (documentId) => {
			const snapshot = await fetchDocumentHealthSnapshot(
				props.macro,
				documentId,
			);
			if (!snapshot) throw new Error("Waiting for the server workflow");
			return snapshot;
		},
	);

	createEffect(() => {
		const stop = subscribeToDocumentHealth({
			macro: props.macro,
			documentId: props.documentId,
			onChange: () => void refetch(),
			onStreamError: (error) =>
				console.error("Document Health event stream failed", error),
		});
		onCleanup(stop);
	});

	return (
		<div class="space-y-3 text-xs">
			<Show when={health.loading}>
				<p class="text-ink-muted">Reading the Macro property...</p>
			</Show>
			<Show when={health.error}>
				<div class="flex items-center justify-between gap-2">
					<p class="text-status-warning">
						{health.error instanceof Error
							? health.error.message
							: "Document Health is unavailable"}
					</p>
					<button
						type="button"
						class="rounded-md border border-edge-muted px-2 py-1 text-ink hover:bg-hover"
						onClick={() => void refetch()}
					>
						Retry
					</button>
				</div>
			</Show>
			<Show when={health.error ? undefined : health()}>
				{(snapshot) => (
					<>
						<div class="flex items-end justify-between">
							<div>
								<div class="text-3xl font-semibold text-ink">
									{snapshot().score}
								</div>
								<div class="text-ink-muted">health score</div>
							</div>
							<button
								type="button"
								class="rounded-md border border-edge-muted px-2 py-1 text-ink hover:bg-hover"
								onClick={() => void refetch()}
							>
								Refresh
							</button>
						</div>
						<div class="h-1 overflow-hidden rounded-full bg-edge-muted">
							<div
								class="h-full rounded-full bg-accent"
								style={{ width: `${snapshot().score}%` }}
							/>
						</div>
						<div class="grid grid-cols-3 gap-2">
							<Metric label="Words" value={snapshot().words} />
							<Metric label="Read" value={`${snapshot().readingMinutes}m`} />
							<Metric label="Long" value={snapshot().longSentences} />
						</div>
						<Show when={snapshot().openTodos.length > 0}>
							<div class="border-t border-edge-muted pt-3">
								<div class="mb-2 font-medium text-ink">Open work</div>
								<ul class="space-y-1 text-ink-muted">
									<For each={snapshot().openTodos}>
										{(todo) => <li>- {todo}</li>}
									</For>
								</ul>
							</div>
						</Show>
					</>
				)}
			</Show>
		</div>
	);
}

function Metric(props: { label: string; value: number | string }) {
	return (
		<div class="rounded-md bg-hover p-2 text-center">
			<div class="font-medium text-ink">{props.value}</div>
			<div class="text-ink-muted">{props.label}</div>
		</div>
	);
}
