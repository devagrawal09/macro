import { createHash } from "node:crypto";
import path from "node:path";
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import type { LoadedPlugin } from "./config";
import type { SolidProvenance } from "./solid";
import { buildEntry } from "./build";
import { resolveOutputPath } from "./paths";
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
	const outputRoot = path.resolve(outDir);
	if (outputRoot === path.parse(outputRoot).root)
		throw new Error(
			"MPC600 refusing to use the filesystem root as an output directory",
		);
	await rm(outputRoot, { recursive: true, force: true });
	await mkdir(outputRoot, { recursive: true });
	const entrypoints: Record<string, unknown> = Object.create(null),
		slots: unknown[] = [],
		events: unknown[] = [];
	for (const entry of [...plugin.entries].sort((a, b) =>
		a.id.localeCompare(b.id),
	)) {
		const rel = `${entry.target}/${entry.id}/index.js`;
		const entryPath = resolveOutputPath(outputRoot, rel);
		await buildEntry(entry, plugin, entryPath, solidRoot, outputRoot);
		const bytes = await readFile(entryPath);
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
	await writeFile(
		resolveOutputPath(outputRoot, "manifest.json"),
		canonical(manifest),
	);
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
	await writeFile(
		resolveOutputPath(outputRoot, "provenance.json"),
		canonical(provenance),
	);
	const files: Record<string, { bytes: number; digest: string }> =
		Object.create(null);
	for (const rel of await filesUnder(outputRoot)) {
		const bytes = await readFile(resolveOutputPath(outputRoot, rel));
		files[rel] = { bytes: bytes.byteLength, digest: `sha256-${sha(bytes)}` };
	}
	await writeFile(
		resolveOutputPath(outputRoot, "integrity.json"),
		canonical({ schemaVersion: 1, algorithm: "sha256", files }),
	);
}
