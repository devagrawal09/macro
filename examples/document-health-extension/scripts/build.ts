import { copyFile, readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { build, type InlineConfig } from "vite";
import solid from "vite-plugin-solid";

const root = resolve(import.meta.dirname, "..");
const outDir = join(root, "dist");
const target = "chrome120";

// Source aliases so neither workspace package needs a `dist` build first.
const shared: InlineConfig = {
	root,
	configFile: false,
	logLevel: "warn",
	plugins: [solid()],
	resolve: {
		alias: {
			"@macro/sdk/browser": resolve(root, "../../packages/sdk/src/macro.browser.ts"),
			"@macro/ui": resolve(root, "../../packages/ui/src/index.ts"),
		},
		dedupe: ["solid-js"],
	},
};

// Extension pages: module scripts referenced from panel.html and popup.html.
await build({
	...shared,
	build: {
		outDir,
		emptyOutDir: true,
		target,
		assetsInlineLimit: 0,
		rollupOptions: {
			input: {
				panel: join(root, "panel.html"),
				popup: join(root, "popup.html"),
			},
		},
	},
});

// Content script: one classic IIFE bundle, as Manifest V3 requires.
await build({
	...shared,
	build: {
		outDir,
		emptyOutDir: false,
		target,
		lib: {
			entry: join(root, "src/content.ts"),
			formats: ["iife"],
			name: "macroDocumentHealthContent",
			fileName: () => "content.js",
		},
	},
});

await copyFile(join(root, "manifest.json"), join(outDir, "manifest.json"));

async function* scripts(dir: string): AsyncGenerator<string> {
	for (const entry of await readdir(dir, { withFileTypes: true })) {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) yield* scripts(path);
		else if (entry.name.endsWith(".js")) yield path;
	}
}

for await (const file of scripts(outDir)) {
	const source = await readFile(file, "utf8");
	for (const forbidden of [
		"eval(",
		"new Function(",
		"importScripts(",
		"MACRO_API_KEY",
		"MACRO_BOT_TOKEN",
		"process.env",
		"node:",
	]) {
		if (source.includes(forbidden)) {
			throw new Error(`${file} contains forbidden executable-code loading`);
		}
	}
}
