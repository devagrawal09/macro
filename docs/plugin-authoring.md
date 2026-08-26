# Local Macro Plugin authoring (compiler checkpoint)

This checkpoint proves only the local authoring and compiler kill gate. It does not publish, install, execute, grant access to, or load a Plugin. Immutable `PluginRelease` storage, object-store upload, publishing, installation, delegated authorization, and runtime hosting are next-checkpoint work.

## 15-minute workflow

1. Install the repository's pinned Bun dependencies with `bun install`.
2. Create `plugin.tsx` with one `export default definePlugin({...})`. Use `projectPage`, `entitySidePanel`, and `onBestEffortEvent` entries as needed.
3. Keep each `render` or `handle` callback inline. Move reused values, functions, and components into normal ESM helper modules and import them.
4. Run `bun run --cwd packages/plugin-cli src/cli.ts plugin check ./plugin.tsx`. The installed bin form is `macro plugin check ./plugin.tsx`.
5. Run `bun run --cwd packages/plugin-cli src/cli.ts plugin build ./plugin.tsx --out-dir .macro-plugin/release`. Inspect `manifest.json`, `integrity.json`, and `provenance.json`.
6. Re-run the build into a clean directory and compare hashes. The compiler owns and clears only the requested output directory.

## Honest restrictions

The local CLI imports the developer-owned plugin module as trusted build configuration to enumerate descriptors. It never invokes `render` or `handle`. This POC does not protect a developer from code they deliberately run during their own build, including environment reads or top-level effects. Use literal metadata and side-effect-free modules for repeatable output. Inline callbacks can use parameters, locals, module-local constants, standard globals, and normal imported helpers/components. Macro servers never evaluate plugin source.

Entry IDs use a conservative lowercase ASCII slug such as `task-details`. Separators, traversal, absolute paths, and prototype keys are rejected. The builder also resolves every output path below the requested output root before writing it. This filesystem guard is narrow and does not turn the compiler into a hostile-source sandbox.

A narrow TypeScript selector exists because the simpler untouched-definition Vite proof failed: a valid full-stack definition left `MPC_CLIENT_ONLY` bytes in its server output, and a server helper using `node:fs` could break a selected client build before tree shaking. The selector locates one validated descriptor callback. It copies only recursively referenced static ESM imports and immutable module constants into a virtual module. It rejects ambiguous or dynamic shapes and mutable dependencies with `MPC2xx` source spans. It does not compile JSX or otherwise rewrite callback semantics. Vite/Rollup remains responsible for Solid JSX compilation, selected graph traversal, tree shaking, bundling, minification, and one standalone output per entry. Final bundles are import-free and scanned for opposite-target markers and forbidden runtime values. This is not a security sandbox.

Client bundles use tracked Solid source from `/Users/devagr/solid` at `8a44c9eb0d8ae5d4a8193e2c88d891ee0ae7a82c`, never ignored `dist` files. Published `@dom-expressions/compiler` and `@dom-expressions/runtime` are pinned to `0.50.0-next.43`. The checkout commit, tracked source trees, aggregate source digest, package versions, path, CLI/SDK versions, and `bun.lock` digest are recorded in provenance. A mismatch fails before build. Each client entry contains its own Solid runtime and has no shared chunks, source maps, external runtime, or CDN imports.
