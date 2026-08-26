import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { loadPlugin } from "../src/config";
import { buildRelease } from "../src/release";
import { validateSolidCheckout } from "../src/solid";
const fixture = (name: string, ext = "tsx") =>
		path.join(import.meta.dir, "fixtures", name, `plugin.${ext}`),
	repo = path.resolve(import.meta.dir, "../../..");
async function tree(root: string) {
	const out: Record<string, string> = {};
	async function walk(dir: string) {
		for (const item of await readdir(dir, { withFileTypes: true })) {
			const full = path.join(dir, item.name);
			if (item.isDirectory()) await walk(full);
			else
				out[path.relative(root, full)] = createHash("sha256")
					.update(await readFile(full))
					.digest("hex");
		}
	}
	await walk(root);
	return out;
}
test("builds exact full-stack manifest with standalone target isolation", async () => {
	const root = await mkdtemp(path.join(tmpdir(), "macro-plugin-test-"));
	try {
		const plugin = await loadPlugin(fixture("full")),
			solid = validateSolidCheckout();
		await buildRelease(plugin, root, solid.root, solid.provenance, repo);
		const manifest = JSON.parse(
			await readFile(path.join(root, "manifest.json"), "utf8"),
		);
		const integrity = JSON.parse(
			await readFile(path.join(root, "integrity.json"), "utf8"),
		);
		expect(manifest).toEqual({
			apiVersion: "1",
			plugin: {
				id: "com.macro.task-tools",
				name: "Task Tools",
				version: "0.1.0",
			},
			capabilities: ["tasks.create", "tasks.read", "tasks.rename"],
			entrypoints: {
				"task-created": {
					target: "server",
					file: "server/task-created/index.js",
					integrity: expect.stringMatching(/^sha256-[a-f0-9]{64}$/),
					capabilities: ["tasks.read", "tasks.rename"],
				},
				"task-details": {
					target: "client",
					file: "client/task-details/index.js",
					integrity: expect.stringMatching(/^sha256-[a-f0-9]{64}$/),
					capabilities: ["tasks.read"],
				},
				tasks: {
					target: "client",
					file: "client/tasks/index.js",
					integrity: expect.stringMatching(/^sha256-[a-f0-9]{64}$/),
					capabilities: ["tasks.create", "tasks.read"],
				},
			},
			slots: [
				{
					entrypoint: "task-details",
					slot: "entity.side_panel",
					entityTypes: ["task"],
				},
				{ entrypoint: "tasks", slot: "project.page", entityTypes: [] },
			],
			events: [
				{
					entrypoint: "task-created",
					event: "task.created",
					delivery: "best-effort",
				},
			],
			customEvents: [{ name: "task.flagged", direction: "client" }],
		});
		const releaseFiles = [
			...Object.values(manifest.entrypoints).map(
				(entry) => (entry as { file: string }).file,
			),
			"manifest.json",
			"provenance.json",
		].sort();
		expect(Object.keys(integrity.files).sort()).toEqual(releaseFiles);
		for (const entry of Object.values(manifest.entrypoints) as Array<{
			file: string;
			integrity: string;
		}>) {
			const bytes = await readFile(path.join(root, entry.file));
			expect(`sha256-${createHash("sha256").update(bytes).digest("hex")}`).toBe(
				entry.integrity,
			);
		}
		const client = await readFile(
				path.join(root, "client/tasks/index.js"),
				"utf8",
			),
			server = await readFile(
				path.join(root, "server/task-created/index.js"),
				"utf8",
			);
		expect(client.length).toBeGreaterThan(20_000);
		expect(client).toContain("MPC_CLIENT_ONLY");
		expect(client).not.toContain("MPC_SERVER_ONLY");
		expect(client).not.toMatch(/\bimport\s/);
		expect(server).toContain("MPC_SERVER_ONLY");
		expect(server).not.toContain("MPC_CLIENT_ONLY");
		expect(server).not.toContain("@solidjs/web");
		expect(server).not.toMatch(/\bimport\s/);
		const serverModule = await import(
			`${pathToFileURL(path.join(root, "server/task-created/index.js")).href}?test=${Date.now()}`
		);
		await serverModule.default({ id: "task-1" }, { projectId: "project-1" });
		expect((globalThis as { __taskResult?: string }).__taskResult).toBe(
			"MPC_SERVER_ONLY:project-1:task-1",
		);
	} finally {
		await rm(root, { recursive: true, force: true });
	}
});
test("clean rebuilds are byte deterministic", async () => {
	const a = await mkdtemp(path.join(tmpdir(), "macro-plugin-a-")),
		b = await mkdtemp(path.join(tmpdir(), "macro-plugin-b-"));
	try {
		const plugin = await loadPlugin(fixture("tiny")),
			solid = validateSolidCheckout();
		await buildRelease(plugin, a, solid.root, solid.provenance, repo);
		await buildRelease(plugin, b, solid.root, solid.provenance, repo);
		expect(await tree(a)).toEqual(await tree(b));
		expect(
			JSON.parse(await readFile(path.join(a, "manifest.json"), "utf8"))
				.customEvents,
		).toEqual([]);
	} finally {
		await rm(a, { recursive: true, force: true });
		await rm(b, { recursive: true, force: true });
	}
});
test("server-only plugin builds without a client runtime", async () => {
	const root = await mkdtemp(path.join(tmpdir(), "macro-plugin-server-"));
	try {
		const plugin = await loadPlugin(fixture("server", "ts")),
			solid = validateSolidCheckout();
		await buildRelease(plugin, root, solid.root, solid.provenance, repo);
		expect(
			await readFile(path.join(root, "server/received/index.js"), "utf8"),
		).not.toContain("solid-js");
		expect(await readdir(root)).not.toContain("client");
	} finally {
		await rm(root, { recursive: true, force: true });
	}
});
