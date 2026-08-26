import { expect, test } from "bun:test";
import { processCreatedTask } from "../server/process-created-task";
import { FakeMacro } from "./fake-macro";

function macroWith(name: string) {
	const macro = new FakeMacro();
	macro.tasksById.set("task-1", {
		id: "task-1",
		projectId: "project-1",
		name,
	});
	return macro;
}

test("handler ignores wrong, non-created, and incomplete events", async () => {
	const macro = macroWith("Task");
	for (const event of [
		{ type: "task.updated", taskId: "task-1", projectId: "project-1" },
		{ type: "task.created", taskId: "task-1", projectId: "other" },
		{ type: "task.created", projectId: "project-1" },
	])
		expect(await processCreatedTask(macro, "project-1", event)).toBe(false);
	expect(macro.reads).toEqual([]);
	expect(macro.renames).toEqual([]);
});

test("handler prefixes a created task once", async () => {
	const macro = macroWith("Task");
	const event = {
		type: "task.created",
		taskId: "task-1",
		projectId: "project-1",
	};
	expect(await processCreatedTask(macro, "project-1", event)).toBe(true);
	expect(macro.renames).toEqual([
		{ taskId: "task-1", name: "[processed] Task" },
	]);
	expect(await processCreatedTask(macro, "project-1", event)).toBe(false);
	expect(macro.renames).toHaveLength(1);
});

test("handler does not duplicate an existing prefix", async () => {
	const macro = macroWith("[processed] Task");
	expect(
		await processCreatedTask(macro, "project-1", {
			type: "task.created",
			taskId: "task-1",
			projectId: "project-1",
		}),
	).toBe(false);
	expect(macro.renames).toEqual([]);
});
