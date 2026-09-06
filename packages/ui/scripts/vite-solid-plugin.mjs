import { transform } from "@dom-expressions/compiler";

export default function solidPlugin() {
  return {
    name: "macro-ui-solid-native-dom",
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
        requireImportSource: false
      });
      return { code: result.code, map: null };
    }
  };
}
