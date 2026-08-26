import type { PluginMacroFacade, PluginTaskEvent } from "@macro/plugin";

const PREFIX = "[processed] ";

/** Idempotently prefix one created task in the current project. */
export async function processCreatedTask(
	macro: PluginMacroFacade,
	projectId: string,
	event: PluginTaskEvent,
	signal?: AbortSignal,
): Promise<boolean> {
	if (
		event.type !== "task.created" ||
		event.projectId !== projectId ||
		typeof event.taskId !== "string"
	)
		return false;
	const task = await macro.tasks.read(event.taskId, { signal });
	if (task.name.startsWith(PREFIX)) return false;
	await macro.tasks.rename(event.taskId, `${PREFIX}${task.name}`, { signal });
	return true;
}
