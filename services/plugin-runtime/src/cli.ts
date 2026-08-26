#!/usr/bin/env bun
/** Tiny `run-once` CLI for manual testing against a built server fixture bundle. */
import { parseArgs } from "node:util";
import { readFile } from "node:fs/promises";
import { ServerPluginExecutor } from "./executor";

const USAGE =
	"MPC-R001 usage: macro-plugin-runtime run-once <bundle.js> --event-type task.created (--event '<json>' | --event-file PATH) --project-id ID --installation-id ID [--capabilities a,b] [--event-id ID] [--timeout-ms N]";

export async function runOnceMain(
	argv = process.argv.slice(2),
): Promise<number> {
	try {
		if (argv[0] !== "run-once") throw new Error(USAGE);
		const parsed = parseArgs({
			args: argv.slice(1),
			options: {
				"event-type": { type: "string" },
				event: { type: "string" },
				"event-file": { type: "string" },
				"project-id": { type: "string" },
				"installation-id": { type: "string" },
				capabilities: { type: "string" },
				"event-id": { type: "string" },
				"timeout-ms": { type: "string" },
			},
			allowPositionals: true,
			strict: true,
		});
		const bundlePath = parsed.positionals[0];
		if (!bundlePath || parsed.positionals.length !== 1)
			throw new Error(USAGE);
		const eventType = parsed.values["event-type"],
			projectId = parsed.values["project-id"],
			installationId = parsed.values["installation-id"];
		if (!eventType || !projectId || !installationId) throw new Error(USAGE);

		let eventSource = parsed.values.event;
		if (parsed.values["event-file"])
			eventSource = await readFile(parsed.values["event-file"], "utf8");
		const event = eventSource === undefined ? null : JSON.parse(eventSource);

		const timeoutRaw = parsed.values["timeout-ms"];
		const executor = new ServerPluginExecutor({
			timeoutMsForTests:
				timeoutRaw !== undefined ? Number.parseInt(timeoutRaw, 10) : undefined,
		});
		const outcome = await executor.invoke({
			bundlePath,
			eventId: parsed.values["event-id"] ?? crypto.randomUUID(),
			eventType,
			event,
			projectId,
			installationId,
			capabilities: parsed.values.capabilities?.split(",").filter(Boolean),
		});
		console.log(JSON.stringify({ outcome, runs: executor.runs() }, null, 2));
		return outcome.status === "completed" ? 0 : 1;
	} catch (error) {
		console.error(
			JSON.stringify(
				{ ok: false, message: error instanceof Error ? error.message : String(error) },
				null,
				2,
			),
		);
		return 2;
	}
}

if (import.meta.main) process.exit(await runOnceMain());
