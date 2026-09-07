import { Button, Card, Stack, Text } from "@macro/ui";
import { createResource, Show } from "solid-js";
import { DocumentHealthSummary } from "./DocumentHealthSummary";
import "./styles.css";
import {
	createDocumentHealth,
	type DocumentHealthSource,
} from "./use-document-health";

export interface DocumentHealthPageProps extends DocumentHealthSource {
	/** Navigate back to the source document inside the host application. */
	onOpenDocument?: (info: { fileType?: string }) => void;
}

/**
 * Document Health as a dedicated full-page view. Reads the same canonical
 * Macro property as the sidebar card and follows the same SSE subscription.
 */
export function DocumentHealthPage(props: DocumentHealthPageProps) {
	const health = createDocumentHealth(props);
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
	const documentName = () =>
		identity.error ? undefined : identity()?.name?.trim();

	return (
		<div class="dh-page">
			<Card
				title={documentName() || "Untitled document"}
				description={props.documentId}
				padding="lg"
				actions={
					<Stack direction="row" gap="sm">
						<Button variant="secondary" size="sm" onClick={health.refresh}>
							Refresh
						</Button>
						<Show when={props.onOpenDocument}>
							<Button
								size="sm"
								onClick={() =>
									props.onOpenDocument?.({
										fileType: identity.error ? undefined : identity()?.fileType,
									})
								}
							>
								Open document
							</Button>
						</Show>
					</Stack>
				}
			>
				<Stack gap="lg">
					<Show when={health.streamDown()}>
						<Text role="caption" tone="warning">
							Live updates are unavailable. Use Refresh to pull the latest
							snapshot.
						</Text>
					</Show>
					<Show when={health.errorMessage()}>
						{(message) => <Text tone="failure">{message()}</Text>}
					</Show>
					<Show
						when={health.snapshot()}
						fallback={
							<Show when={!health.error()}>
								<Text tone="muted">
									{health.loading()
										? "Reading the Macro property..."
										: "Waiting for the server workflow to store a snapshot. Edit the document to trigger it."}
								</Text>
							</Show>
						}
					>
						{(snapshot) => (
							<DocumentHealthSummary snapshot={snapshot()} layout="full" />
						)}
					</Show>
				</Stack>
			</Card>
		</div>
	);
}
