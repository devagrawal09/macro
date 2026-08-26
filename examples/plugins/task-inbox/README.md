# Task Inbox Macro Plugin

This is the real authored Task Inbox example for the local Macro Plugin compiler checkpoint. It has one default-exported inline `definePlugin({...})`, one `project.page`, and one best-effort `task.created` reaction. There is no Task side panel yet.

## What it does

The project page uses the Macro facade supplied by its host context. It:

- lists canonical tasks for the current project;
- shows loading, error, empty, and task-list states;
- creates tasks with `shareWithTeam: false`;
- listens for live created/updated task events; and
- reads and upserts canonical task data after creates and live events.

The best-effort server reaction uses the same runtime-neutral facade. It reads a created task and adds `[processed] ` once. It ignores other projects, other event types, incomplete events, and already processed names.

## Local checks

From the repository root:

```sh
bun install --frozen-lockfile
bun --cwd examples/plugins/task-inbox run check
bun --cwd examples/plugins/task-inbox test
bun run --cwd packages/plugin-cli src/cli.ts plugin check examples/plugins/task-inbox/plugin.tsx
bun run --cwd packages/plugin-cli src/cli.ts plugin build examples/plugins/task-inbox/plugin.tsx --out-dir examples/plugins/task-inbox/.macro-plugin/release
```

The example has no publish, deploy, install, or runtime command. Its generated output should stay under ignored `.macro-plugin/` directories.

## Difference from the historical artifact

`examples/artifacts/task-inbox` remains as verified historical evidence. It is a one-off HTML document artifact. It embeds build-time project/environment config, creates a browser SDK from a transferred bearer, implements its own `MessageChannel`, overwrites a fixed Macro document, and runs a separate Bun worker.

This Plugin does none of those things. The future host supplies project identity, cancellation, logging, and a transport-neutral Macro facade. The compiler splits the inline page and handler into standalone client and server bundles.

## Host and backend work still required

This checkpoint only authors and compiles the Plugin. A future host must still:

- publish immutable releases and install them into projects;
- enforce requested capabilities and provide scoped task operations;
- mount the project page with project, cancellation, and Macro context;
- connect best-effort task subscriptions and invoke the server handler;
- enforce runtime limits, delegated authorization, and release integrity; and
- add the separately planned Task side-panel contribution and shared `@macro/plugin-ui` components.
