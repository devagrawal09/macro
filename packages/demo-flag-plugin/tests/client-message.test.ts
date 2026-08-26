import { expect, test } from "bun:test";
import { createCustomEventMessage, emitCustomEvent } from "@macro/plugin";

/**
 * The host does not transfer ports to client contributions yet, so this test
 * pins the exact wire shape FlagPage will send once host wiring lands.
 */
test("FlagPage emission uses the pinned custom-event port message shape", () => {
	const sent: unknown[] = [];
	emitCustomEvent({ postMessage: (m) => sent.push(m) }, "flag.toggled", {
		enabled: true,
		actor: "proj_1",
	});
	expect(sent).toEqual([
		{
			version: 1,
			type: "macro.plugin.custom-event.v1",
			name: "flag.toggled",
			payload: { enabled: true, actor: "proj_1" },
		},
	]);
});

test("message builder matches the emitted shape byte for byte", () => {
	expect(createCustomEventMessage("flag.toggled", { enabled: false })).toEqual({
		version: 1,
		type: "macro.plugin.custom-event.v1",
		name: "flag.toggled",
		payload: { enabled: false },
	});
});
