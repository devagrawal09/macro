import { expect, test } from "bun:test";
import path from "node:path";
import { loadPlugin } from "../src/config";
import { selectedSource } from "../src/select";
const fixture = (name: string) =>
	path.join(import.meta.dir, "fixtures", name, "plugin.tsx");
test("loads the full-stack descriptor and preserves inferred metadata", async () => {
	const plugin = await loadPlugin(fixture("full"));
	expect(plugin.entries.map((e) => [e.id, e.target, e.kind])).toEqual([
		["tasks", "client", "project.page"],
		["task-details", "client", "entity.side_panel"],
		["task-created", "server", "best_effort_event"],
	]);
	expect(plugin.entries[1].entityTypes).toEqual(["task"]);
	expect(plugin.entries[2].event).toBe("task.created");
});
test("small selector keeps only referenced imports and immutable constants", async () => {
	const plugin = await loadPlugin(fixture("full"));
	const client = selectedSource(plugin, plugin.entries[0]);
	expect(client).toContain("import { TaskPage }");
	expect(client).toContain('const heading = "Task Tools"');
	expect(client).not.toContain("processTask");
	expect(client).not.toContain("TaskPanel");
	const server = selectedSource(plugin, plugin.entries[2]);
	expect(server).toContain("processTask");
	expect(server).not.toContain("TaskPage");
	expect(server).not.toContain("TaskPanel");
});
test("selector rejects mutable captures with a source span", async () => {
	const plugin = await loadPlugin(fixture("mutable"));
	expect(() => selectedSource(plugin, plugin.entries[0])).toThrow(
		/MPC205 .*plugin\.tsx:\d+:\d+ selected callback captures mutable module binding value/,
	);
});
test("dynamic metadata and entry shapes fail", async () => {
	await expect(loadPlugin(fixture("dynamic"))).rejects.toThrow(/MPC106/);
});
