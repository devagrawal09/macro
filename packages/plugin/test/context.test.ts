import { expect, test } from "bun:test";
import type {
	PluginClientContext,
	PluginMacroFacade,
	PluginServerContext,
} from "../src";

test("client and server contexts expose only host-supplied neutral primitives", () => {
	const operations = ["list", "read", "create", "rename", "subscribe"];
	const macro = {
		tasks: {
			list: async () => [],
			read: async (taskId: string) => ({
				id: taskId,
				projectId: "p",
				name: "Task",
			}),
			create: async ({
				projectId,
				name,
			}: {
				projectId: string;
				name: string;
				shareWithTeam: boolean;
			}) => ({ id: "task", projectId, name }),
			rename: async (taskId: string, name: string) => ({
				id: taskId,
				projectId: "p",
				name,
			}),
			subscribe: () => (async function* () {})(),
		},
	} satisfies PluginMacroFacade;
	const signal = new AbortController().signal;
	const client = {
		projectId: "p",
		project: { id: "p" },
		macro,
		signal,
		capabilities: ["tasks.list"],
	} satisfies PluginClientContext;
	const server = {
		projectId: "p",
		project: { id: "p" },
		installationId: "i",
		macro,
		signal,
		log: { info: () => {}, error: () => {} },
		capabilities: ["tasks.read"],
	} satisfies PluginServerContext;
	expect(Object.keys(client.macro.tasks)).toEqual(operations);
	expect(server.project.id).toBe("p");
});
