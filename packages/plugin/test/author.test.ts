import { expect, test } from "bun:test";
import {
	capabilities,
	definePlugin,
	entitySidePanel,
	onBestEffortEvent,
	projectPage,
} from "../src";

test("authoring helpers preserve plugin declarations", () => {
	const caps = capabilities("tasks.read", "tasks.create");
	const plugin = definePlugin({
		apiVersion: "1",
		id: "com.macro.test",
		version: "1.2.3",
		capabilities: caps,
		contributions: [
			projectPage({ id: "home", render: (context) => context.projectId }),
			entitySidePanel({
				id: "task",
				entityTypes: ["task"],
				render: (context) => context.entity?.id,
			}),
		],
		handlers: [
			onBestEffortEvent<{ id: string }>({
				id: "created",
				event: "task.created",
				handle: (event) => {
					expect(event.id).toBeString();
				},
			}),
		],
	});
	expect(plugin.id).toBe("com.macro.test");
	expect(plugin.contributions?.map((entry) => entry.kind)).toEqual([
		"project.page",
		"entity.side_panel",
	]);
	expect(plugin.handlers?.[0]?.kind).toBe("best_effort_event");
});

