import { expect, test } from "bun:test";
import type { PluginLogger } from "@macro/plugin";
import {
	handleFlagToggled,
	lastObservedFlag,
} from "../src/server/on-flag-toggled";

interface Recorded {
	level: string;
	message: string;
	fields?: unknown;
}

function fakeLogger(): { log: PluginLogger; entries: Recorded[] } {
	const entries: Recorded[] = [];
	const record =
		(level: string) =>
		(message: string, fields?: Record<string, unknown>) => {
			entries.push({ level, message, fields });
		};
	const log: PluginLogger = {
		info: record("info"),
		error: record("error"),
	};
	return { log, entries };
}

test("server reaction logs one bounded structured entry per event", async () => {
	const logger = fakeLogger();
	await handleFlagToggled({ enabled: true, actor: "proj_1" }, logger.log);
	expect(logger.entries).toEqual([
		{
			level: "info",
			message: "demo-flag-plugin handled flag.toggled",
			fields: { enabled: true, actor: "proj_1" },
		},
	]);
});

test("reaction records the observed payload on the demo global", async () => {
	const logger = fakeLogger();
	await handleFlagToggled({ enabled: true, actor: "proj_2" }, logger.log);
	expect(lastObservedFlag()).toEqual({ enabled: true, actor: "proj_2" });
});

test("missing actor falls back to unknown", async () => {
	const logger = fakeLogger();
	await handleFlagToggled({ enabled: false }, logger.log);
	const entry = logger.entries[0]!;
	expect((entry.fields as { actor: string }).actor).toBe("unknown");
});
