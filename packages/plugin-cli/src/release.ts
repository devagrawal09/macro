import { createHash } from "node:crypto";
import path from "node:path";
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import type { LoadedPlugin } from "./config";
import type { SolidProvenance } from "./solid";
import { buildEntry } from "./build";
const CLI_VERSION = "0.0.1",
	SDK_VERSION = "0.0.1";
function canonical(value: unknown): string {
	return `${JSON.stringify(value, (_, item) => (item && typeof item === "object" && !Array.isArray(item) ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item), 2)}\n`;
}
function sha(bytes: Uint8Array): string {
	return createHash("sha256").update(bytes).digest("hex");
}
async function filesUnder(root: string, current = root): Promise<string[]> {
	const out: string[] = [];
	for (const item of await readdir(current, { withFileTypes: true })) {
		const full = path.join(current, item.name);
		if (item.isDirectory()) out.push(...(await filesUnder(root, full)));
		else out.push(path.relative(root, full).split(path.sep).join("/"));
	}
	return out.sort();
}
export async function buildRelease(
	plugin: LoadedPlugin,
	outDir: string,
	solidRoot: string,
	solid: SolidProvenance,
	repoRoot: string,
): Promise<void> {
	await rm(outDir, { recursive: true, force: true });
	await mkdir(outDir, { recursive: true });
	const entrypoints: Record<string, unknown> = {},
		slots: unknown[] = [],
		events: unknown[] = [];
	for (const entry of [...plugin.entries].sort((a, b) =>
		a.id.localeCompare(b.id),
	)) {
		const rel = `${entry.target}/${entry.id}/index.js`;
		await buildEntry(entry, plugin, path.join(outDir, rel), solidRoot);
		const bytes = await readFile(path.join(outDir, rel));
		entrypoints[entry.id] = {
			target: entry.target,
			file: rel,
			integrity: `sha256-${sha(bytes)}`,
			capabilities: entry.capabilities,
		};
		if (entry.target === "client")
			slots.push({
				entrypoint: entry.id,
				slot: entry.kind,
				entityTypes: entry.entityTypes,
			});
		else
			events.push({
				entrypoint: entry.id,
				event: entry.event,
				delivery: "best-effort",
			});
	}
	const manifest = {
		apiVersion: plugin.definition.apiVersion,
		plugin: {
			id: plugin.definition.id,
			name: plugin.definition.name ?? plugin.definition.id,
			version: plugin.definition.version,
		},
		capabilities: [
			...new Set(plugin.entries.flatMap((e) => e.capabilities)),
		].sort(),
		entrypoints,
		slots,
		events,
	};
	await writeFile(path.join(outDir, "manifest.json"), canonical(manifest));
	const lock = await readFile(path.join(repoRoot, "bun.lock"));
	const source = await readFile(plugin.path);
	const provenance = {
		schemaVersion: 1,
		pluginSource: { entry: path.basename(plugin.path), sha256: sha(source) },
		tool: { name: "@macro/plugin-cli", version: CLI_VERSION },
		pluginSdk: { name: "@macro/plugin", version: SDK_VERSION },
		solid,
		lock: { file: "bun.lock", sha256: sha(lock) },
		targets: {
			client: "es2022-browser-vite-standalone",
			server: "es2022-neutral-vite-standalone",
		},
		sbom: {
			scope:
				"compiler-known direct build inputs only; not a complete transitive SBOM",
			packages: [
				{ name: "@macro/plugin", version: SDK_VERSION },
				{ name: "@macro/plugin-cli", version: CLI_VERSION },
				{ name: "solid-js", version: solid.solidJs },
				{ name: "@solidjs/web", version: solid.web },
				{ name: "@solidjs/signals", version: solid.signals },
				{ name: "@dom-expressions/compiler", version: solid.compiler },
				{ name: "@dom-expressions/runtime", version: solid.runtime },
			],
		},
	};
	await writeFile(path.join(outDir, "provenance.json"), canonical(provenance));
	const files: Record<string, { bytes: number; digest: string }> = {};
	for (const rel of await filesUnder(outDir)) {
		const bytes = await readFile(path.join(outDir, rel));
		files[rel] = { bytes: bytes.byteLength, digest: `sha256-${sha(bytes)}` };
	}
	await writeFile(
		path.join(outDir, "integrity.json"),
		canonical({ schemaVersion: 1, algorithm: "sha256", files }),
	);
}
