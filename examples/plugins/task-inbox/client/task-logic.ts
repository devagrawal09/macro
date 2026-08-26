import type {
	PluginMacroFacade,
	PluginTask,
	PluginTaskEvent,
} from "@macro/plugin";

/** Sort canonical tasks for stable display. */
export function sortTasks(tasks: readonly PluginTask[]): PluginTask[] {
	return [...tasks].sort(
		(a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id),
	);
}

/** Replace one task with its latest canonical value. */
export function upsertTask(
	tasks: readonly PluginTask[],
	task: PluginTask,
): PluginTask[] {
	return sortTasks([...tasks.filter((item) => item.id !== task.id), task]);
}

/** Load the current project's canonical tasks. */
export function loadProjectTasks(
	macro: PluginMacroFacade,
	projectId: string,
	signal?: AbortSignal,
): Promise<readonly PluginTask[]> {
	return macro.tasks.list({ projectId, signal });
}

/** Create a private task, then read its canonical value. */
export async function createPrivateTask(
	macro: PluginMacroFacade,
	projectId: string,
	name: string,
	signal?: AbortSignal,
): Promise<PluginTask> {
	const created = await macro.tasks.create({
		projectId,
		name,
		shareWithTeam: false,
		signal,
	});
	return macro.tasks.read(created.id, { signal });
}

/** Refetch canonical task data for relevant live events. */
export async function followTaskEvents(
	macro: PluginMacroFacade,
	projectId: string,
	onTask: (task: PluginTask) => void,
	signal?: AbortSignal,
): Promise<void> {
	for await (const event of macro.tasks.subscribe({ projectId, signal })) {
		if (signal?.aborted) return;
		if (!isRefreshEvent(event, projectId)) continue;
		onTask(await macro.tasks.read(event.taskId, { signal }));
	}
}

function isRefreshEvent(
	event: PluginTaskEvent,
	projectId: string,
): event is PluginTaskEvent & { readonly taskId: string } {
	return (
		event.projectId === projectId &&
		typeof event.taskId === "string" &&
		(event.type === "task.created" || event.type === "task.updated")
	);
}
