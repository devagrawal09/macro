import { definePlugin, projectPage } from "@macro/plugin";
import { bad } from "./bad";
export default definePlugin({
	apiVersion: "1",
	id: "com.macro.bad-client",
	version: "1.0.0",
	contributions: [
		projectPage({ id: "home", render: () => <div>{bad()}</div> }),
	],
});
