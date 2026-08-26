import {
	definePlugin,
	onBestEffortEvent,
	projectPage,
} from "@macro/plugin";
import { FlagPage } from "./client/flag-page";
import { handleFlagToggled } from "./server/on-flag-toggled";

/** Demo plugin: one project page, one custom event, one server reaction. */
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.demo-flag",
	name: "Demo Flag",
	version: "0.1.0",
	contributions: [
		projectPage({
			id: "flag-demo",
			render: (context) => <FlagPage context={context} />,
		}),
	],
	handlers: [
		onBestEffortEvent<{ enabled: boolean; actor?: string }>({
			id: "flag-toggled",
			event: "flag.toggled",
			handle: async (event, context) => {
				await handleFlagToggled(event, context.log);
			},
		}),
	],
	customEvents: [{ name: "flag.toggled", direction: "both" }],
});
