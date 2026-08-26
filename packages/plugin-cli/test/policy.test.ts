import { expect, test } from "bun:test";
import path from "node:path";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { loadPlugin } from "../src/config";
import { buildEntry } from "../src/build";
import { selectedSource } from "../src/select";
import { validateSolidCheckout } from "../src/solid";
const fixture = (name: string, ext = "tsx") =>
	path.join(import.meta.dir, "fixtures", name, `plugin.${ext}`);
test("imported helper succeeds and module constant remains selected", async () => {
	const plugin = await loadPlugin(fixture("full"));
	expect(selectedSource(plugin, plugin.entries[0])).toContain("heading");
});
for (const [name, ext, pattern] of [
	["forbidden-client", "tsx", /MPC303.*process is forbidden/],
	["forbidden-server", "ts", /node:fs|browser external|MPC/],
	["dynamic-import", "ts", /MPC207/],
] as const)
	test(`${name} is rejected`, async () => {
		const plugin = await loadPlugin(fixture(name, ext)),
			solid = validateSolidCheckout(),
			root = await mkdtemp(path.join(tmpdir(), "macro-plugin-policy-"));
		try {
			await expect(
				buildEntry(
					plugin.entries[0],
					plugin,
					path.join(root, "index.js"),
					solid.root,
				),
			).rejects.toThrow(pattern);
		} finally {
			await rm(root, { recursive: true, force: true });
		}
	});
