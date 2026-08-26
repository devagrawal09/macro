import { afterEach, expect, test } from "bun:test";
import path from "node:path";
import { createServerPluginExecutor } from "../src/executor";

afterEach(() => {
	globalThis.__pluginRuntimeHadMacro = undefined;
});

test("wrong event is a no-op: no import, no facade, no run record", async () => {
	let factoryCalls = 0;
	const executor = createServerPluginExecutor({
		createMacro: () => {
			factoryCalls += 1;
			return {};
		},
	});
	// The bundle path points at a nonexistent file to prove it is never imported.
	const outcome = await executor.invoke({
		bundlePath: path.join(import.meta.dir, "fixtures", "does-not-exist.js"),
		eventId: "evt-wrong",
		eventType: "task.updated",
		expectedEventType: "task.created",
		event: { anything: true },
		projectId: "p1",
		installationId: "i1",
	});
	expect(outcome).toEqual({ status: "skipped", reason: "event_mismatch" });
	expect(factoryCalls).toBe(0);
	expect(executor.runs()).toEqual([]);
	expect(executor.counters().droppedByConcurrency).toBe(0);
	expect(globalThis.__pluginRuntimeHadMacro).toBeUndefined();
});
