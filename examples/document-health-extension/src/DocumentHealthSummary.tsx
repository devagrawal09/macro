import { Badge, Card, Progress, Stack, Text } from "@macro/ui";
import { For, Show } from "solid-js";
import { type DocumentHealth, describeHealthScore } from "./score";

export interface DocumentHealthSummaryProps {
	snapshot: DocumentHealth;
	/** `compact` fits a sidebar card; `full` spreads out for a page. */
	layout?: "compact" | "full";
}

function tone(score: number): "success" | "warning" | "failure" {
	if (score >= 90) return "success";
	if (score >= 70) return "warning";
	return "failure";
}

/** Score, signals, and outstanding work for one stored snapshot. */
export function DocumentHealthSummary(props: DocumentHealthSummaryProps) {
	const full = () => props.layout === "full";
	const score = () => props.snapshot.score;

	return (
		<Stack gap={full() ? "lg" : "md"}>
			<Stack gap="sm">
				<Stack direction="row" align="baseline" justify="between">
					<Text role={full() ? "display" : "title"}>{score()}</Text>
					<Badge tone={tone(score())}>{describeHealthScore(score())}</Badge>
				</Stack>
				<Progress
					value={score()}
					tone={tone(score())}
					label="Document health score"
				/>
				<Text role="caption" tone="muted">
					health score out of 100
				</Text>
			</Stack>

			<div class="dh-metrics">
				<Metric label="Words" value={props.snapshot.words} />
				<Metric label="Read" value={`${props.snapshot.readingMinutes}m`} />
				<Metric label="Long sentences" value={props.snapshot.longSentences} />
			</div>

			<Stack gap="sm">
				<Stack direction="row" align="center" justify="between">
					<Text role="heading">Open work</Text>
					<Badge
						tone={props.snapshot.openTodos.length === 0 ? "success" : "warning"}
					>
						{props.snapshot.openTodos.length}
					</Badge>
				</Stack>
				<Show
					when={props.snapshot.openTodos.length > 0}
					fallback={
						<Text role="caption" tone="muted">
							No open TODOs or unchecked tasks.
						</Text>
					}
				>
					<ol class="dh-todos">
						<For each={props.snapshot.openTodos}>
							{(todo) => (
								<li>
									<Text role="caption">{todo}</Text>
								</li>
							)}
						</For>
					</ol>
				</Show>
			</Stack>
		</Stack>
	);
}

function Metric(props: { label: string; value: number | string }) {
	return (
		<Card padding="sm" class="dh-metric">
			<Stack gap="xs" align="center">
				<Text role="heading">{props.value}</Text>
				<Text role="caption" tone="muted">
					{props.label}
				</Text>
			</Stack>
		</Card>
	);
}
