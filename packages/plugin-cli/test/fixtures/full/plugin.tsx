import {
	definePlugin,
	capabilities,
	projectPage,
	entitySidePanel,
	onBestEffortEvent,
} from "@macro/plugin";
import { TaskPage } from "./client/task-page";
import { TaskPanel } from "./client/task-panel";
import { processTask } from "./server/process-task";
const heading = "Task Tools";
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.task-tools",
	name: "Task Tools",
	version: "0.1.0",
	capabilities: capabilities("tasks.read"),
	contributions: [
		projectPage({
			id: "tasks",
			capabilities: capabilities("tasks.write"),
			render: (context) => (
				<TaskPage projectId={`${heading}:${context.projectId}`} />
			),
		}),
		entitySidePanel({
			id: "task-details",
			entityTypes: ["task"],
			render: (context) => <TaskPanel entityId={context.entity?.id ?? ""} />,
		}),
	],
	handlers: [
		onBestEffortEvent<{ id: string }>({
			id: "task-created",
			event: "task.created",
			capabilities: capabilities("tasks.write"),
			handle: async (event, context) => {
				await processTask(event.id, context.projectId);
			},
		}),
	],
});
