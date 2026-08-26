import { definePlugin, onBestEffortEvent } from "@macro/plugin";
import { bad } from "./bad";
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.bad-server",
	version: "1.0.0",
	handlers: [
		onBestEffortEvent({ id: "event", event: "x", handle: async () => bad() }),
	],
});
