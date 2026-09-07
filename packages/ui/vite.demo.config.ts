import path from "node:path";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  build: {
    outDir: "dist-demo",
    rollupOptions: {
      input: {
        host: path.resolve("demo/index.html"),
        plugin: path.resolve("demo/plugin.html"),
      },
    },
  },
});
