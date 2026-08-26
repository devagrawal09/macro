// Builds the static runnable catalog page into dist-catalog/ (not committed).
import { defineConfig } from "vite";
import solidPlugin from "./scripts/vite-solid-plugin.mjs";

export default defineConfig({
	define: { __DEV__: "false", _SOLID_DEV_: "false", _DEV_: "false" },
	resolve: {
		alias: {
			"solid-js": "/Users/devagr/solid/packages/solid/src/index.ts",
			"@solidjs/web": "/Users/devagr/solid/packages/solid-web/src/index.ts",
			"@solidjs/signals": "/Users/devagr/solid/packages/solid-signals/src/index.ts",
			"@dom-expressions/runtime": "/Users/devagr/dom-expressions/packages/runtime",
			rxcore: "/Users/devagr/solid/packages/solid-web/src/core.ts",
		},
	},
	plugins: [solidPlugin()],
	build: {
		outDir: "dist-catalog",
		rollupOptions: { input: "catalog/catalog.html" },
	},
});
