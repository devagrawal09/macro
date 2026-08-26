import { definePlugin, onBestEffortEvent } from "@macro/plugin";
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.dynamic-import",
	version: "1.0.0",
	handlers: [
		onBestEffortEvent({
			id: "event",
			event: "x",
			handle: async () => {
				await import("./helper");
			},
		}),
	],
});
