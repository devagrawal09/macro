import path from "node:path";
import { mkdir, writeFile } from "node:fs/promises";
import { build as viteBuild, type Plugin, type Rollup } from "vite";
import { transform } from "@dom-expressions/compiler";
import type { LoadedEntry, LoadedPlugin } from "./config";
import { selectedSource } from "./select";
import { assertBundle, projectPolicy } from "./policy";
function virtual(source: string, resolveDir: string): Plugin {
	const id = "\0macro-plugin-selected.tsx";
	return {
		name: "macro-plugin-selected",
		async resolveId(s, importer) {
			if (s === "virtual:macro-plugin-selected") return id;
			if (importer === id && s.startsWith("."))
				return (
					await this.resolve(s, path.join(resolveDir, "plugin.tsx"), {
						skipSelf: true,
					})
				)?.id;
		},
		load(x) {
			if (x === id) return source;
		},
	};
}
function solidTransform(): Plugin {
	return {
		name: "macro-solid-native-dom",
		enforce: "pre",
		transform(code, id) {
			if (!/\.[jt]sx$/.test(id)) return null;
			const result = transform(code, {
				filename: id,
				moduleName: "@solidjs/web",
				generate: "dom",
				hydratable: false,
				dev: false,
				sourceMap: false,
				requireImportSource: false,
			});
			return { code: result.code, map: null };
		},
	};
}
function sourceAlias(root: string): Plugin {
	const runtime = path.resolve(
		import.meta.dir,
		"../../../node_modules/@dom-expressions/runtime",
	);
	const aliases: Record<string, string> = {
		"solid-js": path.join(root, "packages/solid/src/index.ts"),
		"@solidjs/web": path.join(root, "packages/solid-web/src/index.ts"),
		"@solidjs/signals": path.join(root, "packages/solid-signals/src/index.ts"),
		rxcore: path.join(root, "packages/solid-web/src/core.ts"),
	};
	return {
		name: "macro-solid-source",
		enforce: "pre",
		resolveId(source) {
			if (source.startsWith("@dom-expressions/runtime/"))
				return path.join(
					runtime,
					source.slice("@dom-expressions/runtime/".length),
				);
			return aliases[source] ?? null;
		},
	};
}
export async function buildEntry(
	entry: LoadedEntry,
	plugin: LoadedPlugin,
	outputPath: string,
	solidRoot: string,
): Promise<void> {
	await mkdir(path.dirname(outputPath), { recursive: true });
	const output = (await viteBuild({
		configFile: false,
		root: path.dirname(plugin.path),
		logLevel: "silent",
		define: { __DEV__: "false" },
		plugins: [
			virtual(selectedSource(plugin, entry), path.dirname(plugin.path)),
			projectPolicy(entry.target, path.dirname(plugin.path)),
			sourceAlias(solidRoot),
			solidTransform(),
		],
		build: {
			target: "es2022",
			minify: "esbuild",
			sourcemap: false,
			write: false,
			cssCodeSplit: false,
			rollupOptions: {
				preserveEntrySignatures: "strict",
				input: "virtual:macro-plugin-selected",
				output: {
					format: "es",
					inlineDynamicImports: true,
					entryFileNames: "index.js",
					assetFileNames: "[name][extname]",
				},
			},
		},
	})) as Rollup.RollupOutput;
	const chunk = output.output.find(
		(item): item is Rollup.OutputChunk => item.type === "chunk",
	);
	if (!chunk) throw new Error(`MPC501 Vite emitted no ${entry.target} chunk`);
	assertBundle(entry.target, chunk.code, outputPath);
	if (
		entry.target === "client" &&
		!chunk.code.includes("createRoot") &&
		!chunk.code.includes("Solid")
	)
		throw new Error(
			"MPC502 client bundle does not contain the pinned Solid runtime",
		);
	await writeFile(outputPath, chunk.code);
	for (const asset of output.output.filter(
		(item): item is Rollup.OutputAsset => item.type === "asset",
	))
		await writeFile(
			path.join(path.dirname(outputPath), asset.fileName),
			asset.source,
		);
}
