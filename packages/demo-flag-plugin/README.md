# `@macro/demo-flag-plugin`

End-to-end demo Macro Plugin for the custom-events checkpoint.

It declares one custom event (`flag.toggled`, direction `both`), renders one
project page with `@macro/plugin-ui` components that emits the event through
the typed `@macro/plugin` port-message API (`emitCustomEvent`), and reacts on
the server with one best-effort handler (`handleFlagToggled`) executable
through the local `services/plugin-runtime` run-once CLI.

## Local loop

```sh
bun install --frozen-lockfile

# plugin-ui dist output is required for client compilation
bun run --cwd packages/plugin-ui build

# `ui-server-stub.ts` gives the compiler's trusted definition loader inert
# stand-ins for the precompiled Solid UI kit; Vite still bundles the real kit.
STUB="--preload ./packages/demo-flag-plugin/scripts/ui-server-stub.ts"

bun $STUB packages/plugin-cli/src/cli.ts plugin check \
	packages/demo-flag-plugin/src/plugin.tsx

bun $STUB packages/plugin-cli/src/cli.ts plugin build \
	packages/demo-flag-plugin/src/plugin.tsx \
	--out-dir /tmp/demo-flag-release

# manifest.json then contains: customEvents: [{ name: "flag.toggled", direction: "both" }]

bun services/plugin-runtime/src/cli.ts run-once \
	/tmp/demo-flag-release/server/flag-toggled/index.js \
	--event-type flag.toggled --event '{"enabled":true,"actor":"proj_1"}' \
	--project-id proj_1 --installation-id inst_1

bun run --cwd packages/demo-flag-plugin test
```

## What works today vs what waits on host wiring

Works end to end locally:

1. Declaration -> compiler: `customEvents` survives into `manifest.json`.
2. Admission shape: names/directions match the platform's validated form
   (`PluginEvent::parse`: 1-128 chars of `[a-z0-9._-]`, alphanumeric edges).
3. Server reaction: the compiled server bundle executes through the
   plugin-runtime executor and completes against a real payload.
4. Client emission contract: `createCustomEventMessage`, `emitCustomEvent`,
   and `onCustomEvent` in `@macro/plugin` pin the exact
   `macro.plugin.custom-event.v1` port-message shape, unit-tested.

Waiting on host work (deliberately not built here):

- The host does not transfer a MessagePort to client contributions yet, so
  `FlagPage` has no live sink; it renders the exact message it will emit.
- No host-side routing of `macro.plugin.custom-event.v1` messages between
  client surfaces and server reactions exists.
- Manifest admission at install/publish time (crates/plugin_platform) is not
  exercised by this local loop; only its documented rules are matched.
