import {
	capabilities,
	definePlugin,
	onBestEffortEvent,
	type PluginTaskEvent,
	projectPage,
} from "@macro/plugin";
import { TaskInboxPage } from "./client/task-inbox";
import { processCreatedTask } from "./server/process-created-task";

export default definePlugin({
	apiVersion: "1",
	id: "com.macro.task-inbox",
	name: "Task Inbox",
	version: "0.1.0",
	contributions: [
		projectPage({
			id: "task-inbox",
			capabilities: capabilities("tasks.list", "tasks.read", "tasks.create"),
			render: (context) => <TaskInboxPage context={context} />,
		}),
	],
	handlers: [
		onBestEffortEvent<PluginTaskEvent>({
			id: "process-created-task",
			event: "task.created",
			capabilities: capabilities("tasks.read", "tasks.rename"),
			handle: async (event, context) => {
				const renamed = await processCreatedTask(
					context.macro,
					context.project.id,
					event,
					context.signal,
				);
				if (renamed)
					context.log.info("processed created task", { taskId: event.taskId });
			},
		}),
	],
});
