import { afterEach, expect, test } from "bun:test";
import path from "node:path";
import { createServerPluginExecutor } from "../src/executor";
import type { FacadeInfo } from "../src/types";

const bundle = path.join(import.meta.dir, "fixtures", "rename-handler.js");
const renameCalls: Array<Record<string, unknown>> = [];
const facadeArgs: FacadeInfo[] = [];

afterEach(() => {
	renameCalls.length = 0;
	facadeArgs.length = 0;
});

function makeExecutor() {
	return createServerPluginExecutor({
		createMacro: (info) => {
			facadeArgs.push({ ...info });
			return {
				rename: async (
					taskId: string,
					name: string,
					options: { signal: AbortSignal },
				) => {
					expect(options.signal.aborted).toBe(false);
					renameCalls.push({ taskId, name });
				},
			};
		},
	});
}

test("completed invocation runs the handler once with a runtime-neutral context", async () => {
	const executor = makeExecutor();
	const outcome = await executor.invoke({
		bundlePath: bundle,
		eventId: "evt-1",
		eventType: "task.created",
		expectedEventType: "task.created",
		event: { taskId: "t1", name: "New name" },
		projectId: "p1",
		installationId: "i1",
		capabilities: ["tasks.read", "tasks.rename"],
	});
	expect(outcome).toEqual({
		status: "completed",
		runId: outcome.status === "completed" ? outcome.runId : "",
		durationMs: expect.any(Number),
	});
	expect(renameCalls).toEqual([{ taskId: "t1", name: "New name" }]);

	const [record] = executor.runs();
	expect(record).toMatchObject({
		eventId: "evt-1",
		eventType: "task.created",
		status: "completed",
	});
	expect(record?.startedAt).toBeNumber();
	expect(record?.finishedAt).toBeNumber();
	expect(record.runId).toBe(
		outcome.status === "completed" ? outcome.runId : "",
	);
});

test("facade receives the expected coordinates and the logger captures entries", async () => {
	const executor = makeExecutor();
	await executor.invoke({
		bundlePath: bundle,
		eventId: "evt-2",
		eventType: "task.created",
		event: { taskId: "t2", name: "Second" },
		projectId: "proj-2",
		installationId: "inst-2",
		capabilities: ["tasks.rename"],
	});
	expect(facadeArgs).toEqual([
		{
			projectId: "proj-2",
			installationId: "inst-2",
			capabilities: ["tasks.rename"],
			eventId: "evt-2",
			eventType: "task.created",
		},
	]);
});
