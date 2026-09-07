import { Button, Stack, Text } from "@macro/ui";
import { Show } from "solid-js";
import { DocumentHealthSummary } from "./DocumentHealthSummary";
import "./styles.css";
import {
	createDocumentHealth,
	type DocumentHealthSource,
} from "./use-document-health";

/**
 * Document Health as a compact card. Rendered directly in the local Macro
 * sidebar and inside the browser extension's iframe from the same source.
 */
export function DocumentHealthCard(props: DocumentHealthSource) {
	const health = createDocumentHealth(props);

	return (
		<Stack gap="md" class="dh-card">
			<Show when={health.errorMessage()}>
				{(message) => (
					<Text role="caption" tone="failure">
						{message()}
					</Text>
				)}
			</Show>
			<Show
				when={health.snapshot()}
				fallback={
					<Show when={!health.error()}>
						<Text role="caption" tone="muted">
							{health.loading()
								? "Reading the Macro property..."
								: "Waiting for the server workflow to store a snapshot. Edit the document to trigger it."}
						</Text>
					</Show>
				}
			>
				{(snapshot) => (
					<DocumentHealthSummary snapshot={snapshot()} layout="compact" />
				)}
			</Show>
			<Show when={health.streamDown()}>
				<Text role="caption" tone="warning">
					Live updates are unavailable. Use Refresh to pull the latest
					snapshot.
				</Text>
			</Show>
			<Button size="sm" variant="secondary" onClick={health.refresh}>
				Refresh
			</Button>
		</Stack>
	);
}
