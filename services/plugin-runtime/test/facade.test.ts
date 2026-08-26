import { afterEach, expect, test } from "bun:test";
import path from "node:path";
import { createServerPluginExecutor } from "../src/executor";

afterEach(() => {
	globalThis.__pluginRuntimeHadMacro = undefined;
	globalThis.__pluginRuntimeContextKeys = undefined;
});

const noMacroBundle = path.join(import.meta.dir, "fixtures", "no-macro-handler.js");

test("facade factory result is injected as context.macro", async () => {
	let created = 0;
	const facade = { tasks: { list: async () => [] } };
	const executor = createServerPluginExecutor({
		createMacro: () => {
			created += 1;
			return facade;
		},
	});
	await executor.invoke({
		bundlePath: noMacroBundle,
		eventId: "evt-f1",
		eventType: "task.created",
		event: null,
		projectId: "p",
		installationId: "i",
		capabilities: ["tasks.rename"],
	});
	expect(created).toBe(1);
	expect(globalThis.__pluginRuntimeHadMacro).toBe(true);
	expect(globalThis.__pluginRuntimeContextKeys).toEqual([
		"capabilities",
		"deadline",
		"installationId",
		"log",
		"macro",
		"projectId",
		"signal",
	]);
});

test("without a factory the context has no macro property", async () => {
	const executor = createServerPluginExecutor();
	await executor.invoke({
		bundlePath: noMacroBundle,
		eventId: "evt-f2",
		eventType: "task.created",
		event: null,
		projectId: "p",
		installationId: "i",
		capabilities: [],
	});
	expect(globalThis.__pluginRuntimeHadMacro).toBe(false);
	expect(globalThis.__pluginRuntimeContextKeys).toEqual([
		"capabilities",
		"deadline",
		"installationId",
		"log",
		"projectId",
		"signal",
	]);
});
