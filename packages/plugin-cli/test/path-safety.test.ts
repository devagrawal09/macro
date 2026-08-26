import { expect, test } from "bun:test";
import path from "node:path";
import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { loadPlugin } from "../src/config";
import { buildRelease } from "../src/release";
import { validateSolidCheckout } from "../src/solid";

async function definition(
	entryIds: string[],
): Promise<{ directory: string; file: string }> {
	const directory = await mkdtemp(path.join(import.meta.dir, ".path-safety-"));
	const entries = entryIds
		.map(
			(id) => `projectPage({ id: ${JSON.stringify(id)}, render: () => null })`,
		)
		.join(",");
	const file = path.join(directory, "plugin.ts");
	await writeFile(
		file,
		`import { definePlugin, projectPage } from "@macro/plugin"; export default definePlugin({ apiVersion: "1", id: "com.macro.path-test", version: "1.0.0", contributions: [${entries}] });`,
	);
	return { directory, file };
}

for (const id of [
	"../../escaped",
	"/tmp/absolute",
	"__proto__",
	"prototype",
	"constructor",
]) {
	test(`rejects unsafe entry id ${id}`, async () => {
		const fixture = await definition([id]);
		try {
			await expect(loadPlugin(fixture.file)).rejects.toThrow(/MPC112/);
		} finally {
			await rm(fixture.directory, { recursive: true, force: true });
		}
	});
}

test("rejects canonical entry collisions", async () => {
	const fixture = await definition(["same-entry", "same-entry"]);
	try {
		await expect(loadPlugin(fixture.file)).rejects.toThrow(
			/MPC108 canonical entry id collision/,
		);
	} finally {
		await rm(fixture.directory, { recursive: true, force: true });
	}
});

test("release path ownership is enforced again before writes", async () => {
	const plugin = await loadPlugin(
		path.join(import.meta.dir, "fixtures/tiny/plugin.tsx"),
	);
	plugin.entries[0].id = "../../escaped";
	const parent = await mkdtemp(path.join(tmpdir(), "macro-plugin-owned-"));
	const out = path.join(parent, "out");
	const escaped = path.join(parent, "escaped", "index.js");
	const solid = validateSolidCheckout();
	try {
		await expect(
			buildRelease(
				plugin,
				out,
				solid.root,
				solid.provenance,
				path.resolve(import.meta.dir, "../../.."),
			),
		).rejects.toThrow(/MPC601 output path escapes owned root/);
		expect(existsSync(escaped)).toBe(false);
	} finally {
		await rm(parent, { recursive: true, force: true });
	}
});
