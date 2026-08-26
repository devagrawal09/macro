/**
 * Runnable catalog compositions: the Task Inbox page (project.page) and Task
 * details (entity.side_panel) variants rendered with only public kit exports.
 * Icons are defined locally; dispatch records intents into a DOM note so the
 * catalog proves typed intents without any navigation.
 */
import { createSignal, For } from "solid-js";
import {
	Badge,
	Button,
	EmptyState,
	ErrorState,
	FormField,
	HostAction,
	HostLink,
	IconButton,
	List,
	PluginUIProvider,
	Spinner,
	Stack,
	Surface,
	TextArea,
	Text,
	TextField,
} from "../src/index";
import type { PluginIconProps } from "../src/icon";
import type {
	ClientHostIntent,
	DispatchIntent,
} from "../src/intent";
import type {
	PluginSurfaceContextV1,
	PluginThemeContractV1,
} from "../src/theme";

function icon(path: string) {
	return function CatalogIcon(props: PluginIconProps) {
		return (
			<svg
				viewBox="0 0 16 16"
				width={props.size ?? 16}
				height={props.size ?? 16}
				fill="none"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				<path d={path} />
			</svg>
		);
	};
}

const PlusIcon = icon("M8 3v10M3 8h10");
const SearchIcon = icon("M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10Zm9 4-4.5-4.5");
const MoreIcon = icon("M4 8h.01M8 8h.01M12 8h.01");
const InboxIcon = icon("M2 9h3l1.5 2h3L11 9h3M2 9V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v5m0 0v3a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V9");

interface DemoTask {
	id: string;
	title: string;
	project: string;
	status: string;
	statusTone: "success" | "accent" | "neutral";
	priority: string;
	priorityTone: "warning" | "neutral";
	updated: string;
}

const TASKS: DemoTask[] = [
	{ id: "t1", title: "Prepare launch checklist", project: "Atlas", status: "In progress", statusTone: "success", priority: "High", priorityTone: "warning", updated: "8m ago" },
	{ id: "t2", title: "Review sandbox permissions", project: "Atlas", status: "In review", statusTone: "accent", priority: "Medium", priorityTone: "neutral", updated: "42m ago" },
	{ id: "t3", title: "Draft plugin onboarding guide", project: "Helios", status: "Not started", statusTone: "neutral", priority: "Low", priorityTone: "neutral", updated: "3h ago" },
];

export interface DemoFrameProps {
	theme: PluginThemeContractV1;
	surface: PluginSurfaceContextV1;
	dispatchIntent: DispatchIntent;
	/** Optional observer for dispatched intents (used by the catalog note). */
	onIntent?(intent: ClientHostIntent): void;
}

export function DemoTaskInboxPage(props: DemoFrameProps) {
	const [query, setQuery] = createSignal("");
	const visible = () =>
		TASKS.filter((task) => task.title.toLowerCase().includes(query().toLowerCase()));
	return (
		<PluginUIProvider
			theme={props.theme}
			surface={props.surface}
			dispatchIntent={(intent) => {
				props.onIntent?.(intent);
				return props.dispatchIntent(intent);
			}}
		>
			<Surface as="main" variant="panel" elevation={0} padding={0} aria-label="Task Inbox">
				<Stack direction="column" gap={0}>
					<Stack direction="row" gap={3} align="center" justify="between" style={{ padding: "20px 22px 16px", "border-bottom": "1px solid var(--macro-color-edge-muted)" }}>
						<Stack direction="column" gap={1}>
							<Text as="h1" role="title" weight="semibold">Task Inbox</Text>
							<Text role="caption" tone="muted">12 open tasks in Project Atlas</Text>
						</Stack>
						<Button variant="primary" startIcon={PlusIcon}>New task</Button>
					</Stack>
					<Stack direction="row" gap={2} align="center" style={{ padding: "12px 22px", "border-bottom": "1px solid var(--macro-color-edge-muted)" }}>
						<FormField label="Search tasks" style={{ flex: 1, "max-width": "430px" }}>
							<TextField
								value={query()}
								onValueChange={setQuery}
								leadingIcon={SearchIcon}
								placeholder="Search tasks"
							/>
						</FormField>
						<IconButton icon={MoreIcon} label="Task Inbox options" variant="ghost" />
					</Stack>
					<List label="Tasks" divided selectionMode="multiple">
						<For each={visible()}>{(task) => (
							<List.Item
								selectionLabel={`Select ${task.title}`}
								title={
									<HostLink intent={{ type: "entity.open", entity: { type: "task", id: task.id } }}>
										{task.title}
									</HostLink>
								}
								description={task.project}
								badges={
									<>
										<Badge tone={task.statusTone}>{task.status}</Badge>
										<Badge tone={task.priorityTone}>{task.priority}</Badge>
									</>
								}
								meta={<Text role="caption" tone="subtle">{task.updated}</Text>}
								onActivate={() => {}}
							/>
						)}</For>
					</List>
					{visible().length === 0 ? (
						<div style={{ margin: "12px 18px 20px" }}>
							<EmptyState
								icon={InboxIcon}
								title="No matching tasks"
								description={<span>Try a different search or create a task.</span>}
								action={<Button variant="secondary" onClick={() => setQuery("")}>Clear search</Button>}
							/>
						</div>
					) : null}
					<Stack direction="row" gap={4} style={{ margin: "6px 18px 20px" }}>
						<Spinner size={16} label="Watching for updates" />
						<Text role="caption" tone="subtle">Live task events are connected.</Text>
					</Stack>
				</Stack>
			</Surface>
		</PluginUIProvider>
	);
}

export function DemoTaskSidePanel(props: DemoFrameProps) {
	return (
		<PluginUIProvider
			theme={props.theme}
			surface={props.surface}
			dispatchIntent={(intent) => {
				props.onIntent?.(intent);
				return props.dispatchIntent(intent);
			}}
		>
			<Surface as="aside" variant="panel" elevation={0} padding={0} aria-label="Task details">
				<Stack direction="column" gap={0}>
					<Stack direction="row" gap={2} align="center" justify="between" style={{ padding: "13px 14px", "border-bottom": "1px solid var(--macro-color-edge-muted)" }}>
						<Text role="caption" tone="muted" weight="medium">TASK</Text>
						<IconButton icon={MoreIcon} label="Task actions" size="sm" variant="ghost" />
					</Stack>
					<Stack direction="column" gap={4} style={{ padding: "17px 16px 22px" }}>
						<Text as="h2" role="heading" weight="semibold">Review sandbox permissions</Text>
						<Stack direction="row" gap={2} wrap>
							<Badge tone="success">In progress</Badge>
							<Badge tone="warning">High priority</Badge>
						</Stack>
						<FormField label="Notes" description="Visible to project members">
							<TextArea rows={5} resize="vertical" placeholder="Add notes…" aria-label="Notes body" />
						</FormField>
						<ErrorState
							compact
							description={<span>Sync failed 2 minutes ago.</span>}
							retryLabel="Retry sync"
							onRetry={() => {}}
						/>
						<Stack direction="row" gap={2} wrap>
							<HostAction
								variant="primary"
								intent={{ type: "entity.open", entity: { type: "task", id: "t2" } }}
							>
								Open task
							</HostAction>
							<Button variant="secondary">Mark complete</Button>
						</Stack>
					</Stack>
				</Stack>
			</Surface>
		</PluginUIProvider>
	);
}
