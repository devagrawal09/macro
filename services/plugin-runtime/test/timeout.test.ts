import { afterEach, expect, test } from "bun:test";
import path from "node:path";
import { createServerPluginExecutor } from "../src/executor";

const bundle = path.join(import.meta.dir, "fixtures", "slow-handler.js");

afterEach(() => {
	globalThis.__pluginRuntimeSawAbort = undefined;
});

test("deadline aborts the context signal and records a timeout", async () => {
	const executor = createServerPluginExecutor({ timeoutMsForTests: 50 });
	const startedAt = Date.now();
	const outcome = await executor.invoke({
		bundlePath: bundle,
		eventId: "evt-slow",
		eventType: "task.created",
		event: null,
		projectId: "p1",
		installationId: "i1",
		capabilities: [],
	});
	const elapsed = Date.now() - startedAt;
	expect(outcome.status).toBe("timeout");
	if (outcome.status === "timeout") {
		expect(outcome.error.code).toBe("timeout");
		expect(outcome.durationMs).toBeGreaterThanOrEqual(45);
	}
	expect(elapsed).toBeLessThan(5_000);
	// The handler observed the aborted platform deadline signal.
	expect(globalThis.__pluginRuntimeSawAbort).toBe(true);

	const [record] = executor.runs();
	expect(record?.status).toBe("timeout");
	expect(record?.failureCode).toBe("timeout");
	expect(executor.counters().active).toBe(0);
});
