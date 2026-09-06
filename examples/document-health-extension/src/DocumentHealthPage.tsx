import type { Macro } from "@macro/sdk/browser";
import {
	createEffect,
	createResource,
	createSignal,
	For,
	onCleanup,
	Show,
} from "solid-js";
import {
	fetchDocumentHealthSnapshot,
	subscribeToDocumentHealth,
} from "./health-data";
import { type DocumentHealth, describeHealthScore } from "./score";

export interface DocumentHealthPageProps {
	macro: Macro;
	documentId: string;
	/** Navigate back to the source document inside the host application. */
	onOpenDocument?: (info: { fileType?: string }) => void;
}

/**
 * Document Health as a dedicated full-page view: document identity, the
 * score front and center, per-signal metrics, and outstanding work. Reads
 * the same canonical Macro property as the sidebar card and follows
 * `document.updated` over the upstream SSE stream.
 */
export default function DocumentHealthPage(props: DocumentHealthPageProps) {
	const [identity] = createResource(
		() => props.documentId,
		async (documentId) => {
			const document = props.macro.documents.byId(documentId);
			const [name, fileType] = await Promise.all([
				document.name(),
				document.fileType().catch(() => undefined),
			]);
			return { name, fileType };
		},
	);

	// `undefined` means the workflow has not stored a snapshot yet — an
	// expected empty state, not an error.
	const [health, { refetch }] = createResource(
		() => props.documentId,
		(documentId) => fetchDocumentHealthSnapshot(props.macro, documentId),
	);

	const [streamDown, setStreamDown] = createSignal(false);

	createEffect(() => {
		setStreamDown(false);
		const stop = subscribeToDocumentHealth({
			macro: props.macro,
			documentId: props.documentId,
			onChange: () => void refetch(),
			onStreamError: (error) => {
				console.error("Document Health event stream failed", error);
				setStreamDown(true);
			},
		});
		onCleanup(stop);
	});

	const documentName = () =>
		identity.error ? undefined : identity()?.name?.trim();

	return (
		<div class="h-full overflow-y-auto bg-surface">
			<div class="mx-auto flex w-full max-w-3xl flex-col gap-6 px-4 py-6 sm:px-8 sm:py-10">
				<header class="flex flex-wrap items-start justify-between gap-3">
					<div class="min-w-0">
						<p class="text-xs font-medium uppercase tracking-wide text-ink-muted">
							Document Health
						</p>
						<h1 class="mt-1 truncate text-xl font-semibold text-ink sm:text-2xl">
							{documentName() ?? "Untitled document"}
						</h1>
						<p class="mt-1 truncate text-xs text-ink-muted">
							{props.documentId}
						</p>
					</div>
					<div class="flex shrink-0 items-center gap-2">
						<button
							type="button"
							class="rounded-md border border-edge-muted px-3 py-1.5 text-xs text-ink hover:bg-hover"
							onClick={() => void refetch()}
						>
							Refresh
						</button>
						<Show when={props.onOpenDocument}>
							<button
								type="button"
								class="rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-surface hover:opacity-90"
								onClick={() =>
									props.onOpenDocument?.({
										fileType: identity.error ? undefined : identity()?.fileType,
									})
								}
							>
								Open document
							</button>
						</Show>
					</div>
				</header>

				<Show when={streamDown()}>
					<p class="rounded-md border border-edge-muted bg-hover px-3 py-2 text-xs text-ink-muted">
						Live updates are unavailable. Use Refresh to pull the latest
						snapshot.
					</p>
				</Show>

				<Show when={health.error}>
					<p class="rounded-md border border-edge-muted px-3 py-2 text-xs text-warning">
						{health.error instanceof Error
							? health.error.message
							: "Document Health is unavailable"}
					</p>
				</Show>

				<Show when={!health.error}>
					<Show
						when={health()}
						fallback={
							<div class="rounded-lg border border-edge-muted px-4 py-8 text-center text-xs text-ink-muted">
								{health.loading
									? "Reading the Macro property..."
									: "Waiting for the server workflow to store a snapshot. Edit the document to trigger it."}
							</div>
						}
					>
						{(snapshot) => <HealthOverview snapshot={snapshot()} />}
					</Show>
				</Show>
			</div>
		</div>
	);
}

function HealthOverview(props: { snapshot: DocumentHealth }) {
	return (
		<div class="flex flex-col gap-6">
			<section class="rounded-lg border border-edge-muted p-4 sm:p-6">
				<div class="flex flex-wrap items-end justify-between gap-3">
					<div>
						<div class="text-5xl font-semibold text-ink sm:text-6xl">
							{props.snapshot.score}
						</div>
						<div class="mt-1 text-xs text-ink-muted">health score / 100</div>
					</div>
					<div class="text-sm font-medium text-ink">
						{describeHealthScore(props.snapshot.score)}
					</div>
				</div>
				<div class="mt-4 h-1.5 overflow-hidden rounded-full bg-edge-muted">
					<div
						class="h-full rounded-full bg-accent"
						style={{ width: `${props.snapshot.score}%` }}
					/>
				</div>
			</section>

			<section class="grid grid-cols-1 gap-3 sm:grid-cols-3">
				<PageMetric
					label="Words"
					value={props.snapshot.words}
					detail="total in the document"
				/>
				<PageMetric
					label="Reading time"
					value={`${props.snapshot.readingMinutes}m`}
					detail="at 220 words per minute"
				/>
				<PageMetric
					label="Long sentences"
					value={props.snapshot.longSentences}
					detail="over 24 words"
				/>
			</section>

			<section class="rounded-lg border border-edge-muted p-4 sm:p-6">
				<h2 class="text-sm font-medium text-ink">Open work</h2>
				<Show
					when={props.snapshot.openTodos.length > 0}
					fallback={
						<p class="mt-2 text-xs text-ink-muted">
							No open TODOs or unchecked tasks.
						</p>
					}
				>
					<ul class="mt-3 space-y-2">
						<For each={props.snapshot.openTodos}>
							{(todo) => (
								<li class="rounded-md bg-hover px-3 py-2 text-xs text-ink">
									{todo}
								</li>
							)}
						</For>
					</ul>
				</Show>
			</section>
		</div>
	);
}

function PageMetric(props: {
	label: string;
	value: number | string;
	detail: string;
}) {
	return (
		<div class="rounded-lg border border-edge-muted p-4">
			<div class="text-xs text-ink-muted">{props.label}</div>
			<div class="mt-1 text-2xl font-semibold text-ink">{props.value}</div>
			<div class="mt-1 text-xs text-ink-muted">{props.detail}</div>
		</div>
	);
}
