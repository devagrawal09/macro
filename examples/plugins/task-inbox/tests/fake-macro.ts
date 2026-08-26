import type {
	PluginMacroFacade,
	PluginTask,
	PluginTaskEvent,
	PluginTaskOperations,
} from "@macro/plugin";

export class FakeMacro implements PluginMacroFacade {
	readonly reads: string[] = [];
	readonly creates: Parameters<PluginTaskOperations["create"]>[0][] = [];
	readonly renames: Array<{ taskId: string; name: string }> = [];
	readonly tasksById = new Map<string, PluginTask>();
	events: PluginTaskEvent[] = [];

	readonly tasks: PluginTaskOperations = {
		list: async ({ projectId }) =>
			[...this.tasksById.values()].filter(
				(task) => task.projectId === projectId,
			),
		read: async (taskId) => {
			this.reads.push(taskId);
			const task = this.tasksById.get(taskId);
			if (!task) throw new Error(`missing task ${taskId}`);
			return task;
		},
		create: async (input) => {
			this.creates.push(input);
			const task = {
				id: `task-${this.creates.length}`,
				projectId: input.projectId,
				name: input.name,
			};
			this.tasksById.set(task.id, task);
			return task;
		},
		rename: async (taskId, name) => {
			this.renames.push({ taskId, name });
			const current = this.tasksById.get(taskId);
			if (!current) throw new Error(`missing task ${taskId}`);
			const task = { ...current, name };
			this.tasksById.set(taskId, task);
			return task;
		},
		subscribe: () => {
			const events = [...this.events];
			return (async function* () {
				for (const event of events) yield event;
			})();
		},
	};
}
