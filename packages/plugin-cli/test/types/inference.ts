import {
	capabilities,
	definePlugin,
	onBestEffortEvent,
	projectPage,
} from "@macro/plugin";
const caps = capabilities("tasks.read", "tasks.write");
const plugin = definePlugin({
	apiVersion: "1",
	id: "com.macro.types",
	version: "1.0.0",
	capabilities: caps,
	contributions: [
		projectPage({
			id: "page",
			capabilities: caps,
			render: (context) => {
				const exact: readonly ["tasks.read", "tasks.write"] =
					context.capabilities;
				return exact;
			},
		}),
	],
	handlers: [
		onBestEffortEvent<{ id: string }, typeof caps>({
			id: "handler",
			event: "task.created",
			capabilities: caps,
			handle: (event, context) => {
				const id: string = event.id;
				const exact: typeof caps = context.capabilities;
				void id;
				void exact;
			},
		}),
	],
});
void plugin;
