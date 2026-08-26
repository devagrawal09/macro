import { definePlugin, projectPage } from "@macro/plugin";
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.tiny",
	version: "1.0.0",
	contributions: [
		projectPage({
			id: "home",
			render: (context) => <main data-project={context.projectId}>Tiny</main>,
		}),
	],
});
