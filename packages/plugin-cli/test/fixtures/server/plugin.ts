import { definePlugin, onBestEffortEvent } from "@macro/plugin";
import { save } from "./save";
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.server",
	version: "1.0.0",
	handlers: [
		onBestEffortEvent<{ id: string }>({
			id: "received",
			event: "document.created",
			handle: async (event, context) => {
				await save(event.id, context.projectId);
			},
		}),
	],
});
