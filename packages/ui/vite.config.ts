import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { defineConfig, type Plugin } from "vite";
import solid from "vite-plugin-solid";

/**
 * Library mode drops the `import "./styles.css"` side effect from the entry.
 * Put it back so consumers get the stylesheet by importing the package.
 */
function keepStylesImport(): Plugin {
  return {
    name: "macro-ui-keep-styles-import",
    async closeBundle() {
      const entry = path.resolve("dist/index.js");
      const code = await readFile(entry, "utf8");
      if (!code.startsWith('import "./styles.css";')) {
        await writeFile(entry, `import "./styles.css";\n${code}`);
      }
    },
  };
}

export default defineConfig({
  plugins: [solid(), keepStylesImport()],
  build: {
    lib: {
      entry: path.resolve("src/index.ts"),
      formats: ["es"],
      fileName: "index",
      cssFileName: "styles",
    },
    rollupOptions: {
      external: ["solid-js", "solid-js/web", "solid-js/store"],
    },
    target: "es2022",
    minify: false,
    outDir: "dist",
    emptyOutDir: true,
  },
});
