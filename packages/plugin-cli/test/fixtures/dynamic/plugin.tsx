import { definePlugin, projectPage } from "@macro/plugin";
const entries = [projectPage({ id: "home", render: () => <div /> })];
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.dynamic",
	version: process.env.VERSION,
	contributions: [...entries],
});
