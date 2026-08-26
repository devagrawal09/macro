import { expect, test } from "bun:test";
import {
	createPrivateTask,
	followTaskEvents,
	loadProjectTasks,
	upsertTask,
} from "../client/task-logic";
import { FakeMacro } from "./fake-macro";

test("lists only the current project and creates a private canonical task", async () => {
	const macro = new FakeMacro();
	macro.tasksById.set("b", { id: "b", projectId: "project-1", name: "Beta" });
	macro.tasksById.set("x", { id: "x", projectId: "project-2", name: "Other" });
	expect(await loadProjectTasks(macro, "project-1")).toEqual([
		{ id: "b", projectId: "project-1", name: "Beta" },
	]);

	const task = await createPrivateTask(macro, "project-1", "Alpha");
	expect(macro.creates[0]).toMatchObject({
		projectId: "project-1",
		name: "Alpha",
		shareWithTeam: false,
	});
	expect(macro.reads).toContain(task.id);
});

test("live created and updated events refetch and upsert canonical task data", async () => {
	const macro = new FakeMacro();
	macro.tasksById.set("task-1", {
		id: "task-1",
		projectId: "project-1",
		name: "Canonical",
	});
	macro.events = [
		{ type: "task.created", taskId: "task-1", projectId: "other" },
		{ type: "document.created", taskId: "task-1", projectId: "project-1" },
		{ type: "task.updated", taskId: "task-1", projectId: "project-1" },
	];
	let tasks = [{ id: "task-1", projectId: "project-1", name: "Stale" }];
	await followTaskEvents(macro, "project-1", (task) => {
		tasks = upsertTask(tasks, task);
	});
	expect(tasks).toEqual([
		{ id: "task-1", projectId: "project-1", name: "Canonical" },
	]);
	expect(macro.reads).toEqual(["task-1"]);
});
