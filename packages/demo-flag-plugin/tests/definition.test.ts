import { expect, mock, test } from "bun:test";

// plugin-ui ships precompiled Solid client output that refuses server-side
// evaluation, so the definition test swaps inert stand-ins before loading
// the real plugin definition.
mock.module("@macro/plugin-ui", () => ({
	Badge: () => null,
	Button: () => null,
	Stack: () => null,
	Text: () => null,
}));
// Keep the Solid-rendering page module itself out of the server-side test
// process; the compiler still sees the real component.
mock.module("../src/client/flag-page", () => ({ FlagPage: () => null }));

test("declares one page, one handler, and one both-direction custom event", async () => {
	const { default: plugin } = await import("../src/plugin");
	expect({
		id: plugin.id,
		name: plugin.name,
		version: plugin.version,
		contributions: plugin.contributions?.map(({ id, kind }) => ({ id, kind })),
		handlers: plugin.handlers?.map(({ id, kind, event }) => ({
			id,
			kind,
			event,
		})),
		customEvents: plugin.customEvents,
	}).toEqual({
		id: "com.macro.demo-flag",
		name: "Demo Flag",
		version: "0.1.0",
		contributions: [{ id: "flag-demo", kind: "project.page" }],
		handlers: [
			{ id: "flag-toggled", kind: "best_effort_event", event: "flag.toggled" },
		],
		customEvents: [{ name: "flag.toggled", direction: "both" }],
	});
});
