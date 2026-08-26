// Builds dist/index.js (ESM, externalizes the pinned Solid peers).
// Types are emitted by tsc in scripts below; styles.css is copied by npm script.
import path from "node:path";
import { build as viteBuild } from "vite";
import solidPlugin from "./vite-solid-plugin.mjs";

const solidTransform = solidPlugin;

await viteBuild({
	configFile: false,
	logLevel: "info",
	define: { __DEV__: "false", _SOLID_DEV_: "false", _DEV_: "false" },
	plugins: [solidTransform()],
	build: {
		lib: {
			entry: path.resolve("src/index.ts"),
			formats: ["es"],
			fileName: "index",
		},
		rollupOptions: {
			external: ["solid-js", "@solidjs/web", "@solidjs/signals"],
		},
		target: "es2022",
		minify: false,
		sourcemap: false,
		outDir: "dist",
		emptyOutDir: true,
	},
});
