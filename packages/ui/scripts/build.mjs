import fs from "node:fs/promises";
import path from "node:path";
import { build } from "vite";
import solidPlugin from "./vite-solid-plugin.mjs";

await build({
  configFile: false,
  logLevel: "info",
  define: { __DEV__: "false", _SOLID_DEV_: "false", _DEV_: "false" },
  plugins: [solidPlugin()],
  build: {
    lib: {
      entry: path.resolve("src/index.ts"),
      formats: ["es"],
      fileName: "index",
      cssFileName: "styles"
    },
    rollupOptions: {
      external: ["solid-js", "@solidjs/web", "@solidjs/signals"]
    },
    target: "es2022",
    minify: false,
    sourcemap: false,
    outDir: "dist",
    emptyOutDir: true
  }
});

const entryPath = path.resolve("dist/index.js");
const entry = await fs.readFile(entryPath, "utf8");
await fs.writeFile(entryPath, 'import "./styles.css";\n' + entry);
