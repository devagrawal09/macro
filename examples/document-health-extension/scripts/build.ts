import { copyFile, mkdir, readdir, rm } from "node:fs/promises";
import { join } from "node:path";
import { build } from "esbuild";

const outdir = "dist";
const sdkBrowserEntry = join(
	import.meta.dirname,
	"../../../packages/sdk/src/macro.browser.ts",
);
await rm(outdir, { recursive: true, force: true });
await mkdir(outdir, { recursive: true });

await build({
	alias: { "@macro/sdk/browser": sdkBrowserEntry },
	entryPoints: {
		content: "src/content.ts",
		panel: "src/panel.ts",
		popup: "src/popup.ts",
	},
	bundle: true,
	format: "iife",
	outdir,
	platform: "browser",
	target: "chrome120",
});

await Promise.all(
	["manifest.json", "panel.html", "popup.html"].map((file) =>
		copyFile(file, join(outdir, file)),
	),
);

for (const file of await readdir(outdir)) {
	if (!file.endsWith(".js")) continue;
	const source = await Bun.file(join(outdir, file)).text();
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
