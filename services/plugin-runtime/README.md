# @macro/plugin-runtime

Local, in-process Bun executor for **compiled** Macro Plugin server bundles.
This is a lean POC for trusted local/dev plugin code.

## Trust model (honest scope)

- Runs trusted local plugin code **inside this Bun process**. Bun is NOT a
  security sandbox. There is no isolation claim beyond the concurrency limit,
  the 30 s abort deadline, and bounded in-memory logging.
- Only compiled bundles produced by `macro plugin build` (`server/<entry>/index.js`)
  are imported via ESM `import`. Plugin TypeScript source is never evaluated here.
- Live-only: run records live in memory for the current process only.
  Dropped admissions are counted and their payloads are never retained.

## API

```ts
import { createServerPluginExecutor } from "@macro/plugin-runtime";

const executor = createServerPluginExecutor({
	// Injectable Macro facade: supply a real SDK view or a fake in tests.
	createMacro: ({ projectId, installationId, capabilities }) => realSdk,
});

const outcome = await executor.invoke({
	bundlePath: ".macro-plugin/release/server/task-created/index.js",
	eventId, // unique per event
	eventType: "task.created",
	event: { taskId: "t1" },
	projectId,
	installationId,
	capabilities: ["tasks.read", "tasks.rename"],
	expectedEventType: "task.created", // mismatch => no-op skip
});
// outcome.status: "completed" | "failed" | "timeout" | "dropped" | "skipped"

executor.runs(); // started invocations only, current process
executor.counters(); // active/limit/dropped/completed/failed/timedOut
```

Per invocation the executor builds a runtime-neutral context:

- `projectId`, `installationId`, `capabilities` passthrough;
- `signal`: `AbortSignal.timeout(30s)`;
- `deadline`: epoch ms of the platform deadline;
- `log`: structured logger capped at 100 entries / ~8 KiB (excess dropped and counted);
- `macro`: result of the injected facade factory (absent when no factory is given).

Fixed platform policy: concurrency limit 8 (immediate drop of excess admissions),
timeout 30 s. On timeout the context signal aborts and the outcome is recorded as
`timeout`; in-process handler code that ignores the signal cannot be force-killed.

## CLI

```bash
bun run src/cli.ts run-once \
	server/task-created/index.js \
	--event-type task.created \
	--event '{"taskId":"t1","name":"New name"}' \
	--project-id p1 --installation-id i1 \
	--capabilities tasks.read,tasks.rename
```

Prints the outcome plus run records as JSON. Exit code 0 on `completed`.

## Tests

```bash
bun install
bun --cwd services/plugin-runtime test
bun --cwd services/plugin-runtime run check
```
