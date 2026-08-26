import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { main } from "../../../../packages/plugin-cli/src/cli";
import { FakeMacro } from "./fake-macro";

const entry = path.resolve(import.meta.dir, "../plugin.tsx");

async function tree(root: string): Promise<Record<string, string>> {
	const result: Record<string, string> = {};
	async function walk(dir: string) {
		for (const item of await readdir(dir, { withFileTypes: true })) {
			const full = path.join(dir, item.name);
			if (item.isDirectory()) await walk(full);
			else
				result[path.relative(root, full)] = createHash("sha256")
					.update(await readFile(full))
					.digest("hex");
		}
	}
	await walk(root);
	return result;
}

test("real CLI checks and deterministically builds this exact plugin", async () => {
	const first = await mkdtemp(path.join(tmpdir(), "task-inbox-plugin-a-"));
	const second = await mkdtemp(path.join(tmpdir(), "task-inbox-plugin-b-"));
	try {
		expect(await main(["plugin", "check", entry])).toBe(0);
		expect(await main(["plugin", "build", entry, "--out-dir", first])).toBe(0);
		expect(await main(["plugin", "build", entry, "--out-dir", second])).toBe(0);
		expect(await tree(first)).toEqual(await tree(second));

		const manifest = JSON.parse(
			await readFile(path.join(first, "manifest.json"), "utf8"),
		);
		expect(manifest.capabilities).toEqual([
			"tasks.create",
			"tasks.list",
			"tasks.read",
			"tasks.rename",
		]);
		expect(manifest.slots).toEqual([
			{ entrypoint: "task-inbox", slot: "project.page", entityTypes: [] },
		]);
		expect(manifest.events).toEqual([
			{
				entrypoint: "process-created-task",
				event: "task.created",
				delivery: "best-effort",
			},
		]);

		const clientPath = path.join(first, "client/task-inbox/index.js");
		const serverPath = path.join(first, "server/process-created-task/index.js");
		const client = await readFile(clientPath, "utf8");
		const server = await readFile(serverPath, "utf8");
		expect(client).not.toContain("[processed] ");
		expect(server).not.toContain("Task Inbox");
		expect(client).not.toMatch(/\b(?:import|export)\s+(?:[^"']*?from\s*)?["']/);
		expect(server).not.toMatch(/\b(?:import|export)\s+(?:[^"']*?from\s*)?["']/);

		for (const file of Object.keys(await tree(first))) {
			expect(file.endsWith(".map")).toBe(false);
			const text = await readFile(path.join(first, file), "utf8");
			for (const forbidden of [
				"MACRO_API_KEY",
				"MACRO_BOT_TOKEN",
				"Authorization: Bearer",
				"process.env",
				"node:",
				"bun:",
				"sourceMappingURL",
			])
				expect(text).not.toContain(forbidden);
		}

		const macro = new FakeMacro();
		macro.tasksById.set("task-1", {
			id: "task-1",
			projectId: "project-1",
			name: "Task",
		});
		const logs: unknown[] = [];
		const serverModule = await import(
			`${pathToFileURL(serverPath).href}?test=${Date.now()}`
		);
		await serverModule.default(
			{ type: "task.created", taskId: "task-1", projectId: "project-1" },
			{
				project: { id: "project-1" },
				macro,
				signal: new AbortController().signal,
				log: { info: (...args: unknown[]) => logs.push(args), error: () => {} },
			},
		);
		expect(macro.renames).toEqual([
			{ taskId: "task-1", name: "[processed] Task" },
		]);
		expect(logs).toHaveLength(1);
	} finally {
		await rm(first, { recursive: true, force: true });
		await rm(second, { recursive: true, force: true });
	}
}, 120_000);
