import { afterEach, expect, test } from "bun:test";
import path from "node:path";
import { createServerPluginExecutor } from "../src/executor";

const bundle = path.join(import.meta.dir, "fixtures", "gate-handler.js");

function gate() {
	return { held: [] as Array<{ signal: AbortSignal }>, releasers: [] as Array<() => void> };
}

afterEach(() => {
	globalThis.__pluginRuntimeGate = undefined;
});

test("excess admissions are dropped immediately and counted without retention", async () => {
	globalThis.__pluginRuntimeGate = gate();
	const executor = createServerPluginExecutor({ concurrencyLimitForTests: 2 });

	const first = executor.invoke({
		bundlePath: bundle, eventId: "e1", eventType: "task.created",
		event: null, projectId: "p", installationId: "i",
	});
	const second = executor.invoke({
		bundlePath: bundle, eventId: "e2", eventType: "task.created",
		event: null, projectId: "p", installationId: "i",
	});
	// Wait until two handlers actually hold the slots.
	while (globalThis.__pluginRuntimeGate.held.length < 2)
		await Bun.sleep(1);

	const third = await executor.invoke({
		bundlePath: bundle, eventId: "e3", eventType: "task.created",
		event: { payload: "never-retained" }, projectId: "p", installationId: "i",
	});
	expect(third).toEqual({ status: "dropped", reason: "concurrency_limit" });
	expect(executor.counters()).toMatchObject({
		active: 2,
		concurrencyLimit: 2,
		droppedByConcurrency: 1,
	});
	// Drops never create run records.
	expect(executor.runs().length).toBe(2);

	for (const release of globalThis.__pluginRuntimeGate.releasers) release();
	expect((await first).status).toBe("completed");
	expect((await second).status).toBe("completed");
	expect(executor.counters().active).toBe(0);
	expect(executor.counters().droppedByConcurrency).toBe(1);
});
