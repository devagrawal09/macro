# Plugin frame demo (dev-only)

Temporary demo location for the generic `PluginFrame` host. It mounts the
built Task Tools client bundle inside a sandboxed opaque-origin iframe and
runs the hardened handshake with FAKE grant data. There are no backend calls.

- Route: `/app/dev/plugin-frame` (registered in `src/routes/Root.tsx`).
- The page fails closed unless `LOCAL_ONLY || DEV_MODE_ENV`.

## Rebuilding the committed bundle

The bundle is built by the existing plugin compiler CLI from the Task Tools
fixture and committed as `task-tools-client.bundle.js`:

```sh
bun install --frozen-lockfile
MACRO_PLUGIN_SOLID_SOURCE=/path/to/solid \
  bun run --cwd packages/plugin-cli src/cli.ts plugin build \
  /abs/path/to/packages/plugin-cli/test/fixtures/full/plugin.tsx \
  --out-dir /tmp/task-tools-release
cp /tmp/task-tools-release/client/tasks/index.js task-tools-client.bundle.js
```

This checkpoint does not sideload or fetch releases; a real release pipeline
replaces this committed artifact later.
