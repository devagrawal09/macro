import { definePlugin, projectPage } from "@macro/plugin";
let value = "x";
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.mutable",
	version: "1.0.0",
	contributions: [
		projectPage({ id: "page", render: () => <div>{value}</div> }),
	],
});
