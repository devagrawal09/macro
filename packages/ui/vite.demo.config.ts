import path from "node:path";
import { defineConfig } from "vite";
import solidPlugin from "./scripts/vite-solid-plugin.mjs";

const SOLID_ROOT = process.env.MACRO_SOLID_CHECKOUT ?? "/Users/devagr/solid";
const DOM_EXPRESSIONS_ROOT = process.env.MACRO_DOM_EXPRESSIONS_CHECKOUT ?? "/Users/devagr/dom-expressions";

export default defineConfig({
  define: { __DEV__: "false", _SOLID_DEV_: "false", _DEV_: "false" },
  resolve: {
    alias: {
      "solid-js": path.join(SOLID_ROOT, "packages/solid/src/index.ts"),
      "@solidjs/web": path.join(SOLID_ROOT, "packages/solid-web/src/index.ts"),
      "@solidjs/signals": path.join(SOLID_ROOT, "packages/solid-signals/src/index.ts"),
      "@dom-expressions/runtime": path.join(DOM_EXPRESSIONS_ROOT, "packages/runtime"),
      rxcore: path.join(SOLID_ROOT, "packages/solid-web/src/core.ts")
    }
  },
  plugins: [solidPlugin()],
  build: {
    outDir: "dist-demo",
    rollupOptions: {
      input: {
        host: path.resolve("demo/index.html"),
        plugin: path.resolve("demo/plugin.html")
      }
    }
  }
});
