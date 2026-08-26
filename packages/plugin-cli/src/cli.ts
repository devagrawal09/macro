#!/usr/bin/env bun
import path from "node:path";
import { parseArgs } from "node:util";
import { loadPlugin } from "./config";
import { validateSolidCheckout } from "./solid";
import { buildRelease } from "./release";
import { buildEntry } from "./build";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";

export async function main(argv = process.argv.slice(2)): Promise<number> {
	try {
		if (argv[0] !== "plugin" || !["check", "build"].includes(argv[1] ?? ""))
			throw new Error(
				"MPC001 usage: macro plugin <check|build> <plugin.tsx> [--out-dir PATH]",
			);
		const command = argv[1];
		const parsed = parseArgs({
			args: argv.slice(2),
			options: { "out-dir": { type: "string" } },
			allowPositionals: true,
			strict: true,
		});
		const entry = parsed.positionals[0];
		if (!entry || parsed.positionals.length !== 1)
			throw new Error(
				"MPC001 exactly one plugin.ts or plugin.tsx path is required",
			);
		const pluginPath = path.resolve(entry);
		const plugin = await loadPlugin(pluginPath);
		const { root, provenance } = validateSolidCheckout();
		if (command === "check") {
			const temp = await mkdtemp(path.join(tmpdir(), "macro-plugin-check-"));
			try {
				for (const item of plugin.entries)
					await buildEntry(
						item,
						plugin,
						path.join(temp, item.target, item.id, "index.js"),
						root,
					);
			} finally {
				await rm(temp, { recursive: true, force: true });
			}
			console.log(
				JSON.stringify(
					{
						ok: true,
						plugin: `${plugin.definition.id}@${plugin.definition.version}`,
						entries: plugin.entries.map(({ id, target, kind }) => ({
							id,
							target,
							kind,
						})),
						solid: {
							commit: provenance.commit,
							sourceDigest: provenance.sourceDigest,
						},
					},
					null,
					2,
				),
			);
			return 0;
		}
		const outDir = path.resolve(
			parsed.values["out-dir"] ?? ".macro-plugin/release",
		);
		const repoRoot = path.resolve(import.meta.dir, "../../..");
		await buildRelease(plugin, outDir, root, provenance, repoRoot);
		console.log(
			JSON.stringify(
				{
					ok: true,
					plugin: `${plugin.definition.id}@${plugin.definition.version}`,
					outDir,
				},
				null,
				2,
			),
		);
		return 0;
	} catch (error) {
		const record = {
			code:
				(error instanceof Error
					? error.message.match(/^(MPC\d+)/)?.[1]
					: undefined) ?? "MPC999",
			message: error instanceof Error ? error.message : String(error),
		};
		console.error(JSON.stringify({ ok: false, diagnostic: record }, null, 2));
		return 1;
	}
}
if (import.meta.main) process.exit(await main());
