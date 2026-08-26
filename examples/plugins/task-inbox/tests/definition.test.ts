import { expect, test } from "bun:test";
import plugin from "../plugin";

test("declares one Task Inbox page and one best-effort created-task handler", () => {
	expect({
		id: plugin.id,
		name: plugin.name,
		version: plugin.version,
		contributions: plugin.contributions?.map(({ id, kind, capabilities }) => ({
			id,
			kind,
			capabilities,
		})),
		handlers: plugin.handlers?.map(({ id, kind, event, capabilities }) => ({
			id,
			kind,
			event,
			capabilities,
		})),
	}).toEqual({
		id: "com.macro.task-inbox",
		name: "Task Inbox",
		version: "0.1.0",
		contributions: [
			{
				id: "task-inbox",
				kind: "project.page",
				capabilities: ["tasks.list", "tasks.read", "tasks.create"],
			},
		],
		handlers: [
			{
				id: "process-created-task",
				kind: "best_effort_event",
				event: "task.created",
				capabilities: ["tasks.read", "tasks.rename"],
			},
		],
	});
});
