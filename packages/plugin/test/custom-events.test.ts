import { describe, expect, test } from "bun:test";
import {
	createCustomEventMessage,
	emitCustomEvent,
	isCustomEventMessage,
	onCustomEvent,
	PLUGIN_CUSTOM_EVENT_TYPE,
	PLUGIN_CUSTOM_EVENT_VERSION,
} from "../src";

describe("custom event message contract", () => {
	test("createCustomEventMessage produces the exact port message shape", () => {
		const message = createCustomEventMessage("flag.toggled", { enabled: true });
		expect(Object.keys(message).sort()).toEqual([
			"name",
			"payload",
			"type",
			"version",
		]);
		expect(message).toEqual({
			version: PLUGIN_CUSTOM_EVENT_VERSION,
			type: PLUGIN_CUSTOM_EVENT_TYPE,
			name: "flag.toggled",
			payload: { enabled: true },
		});
	});

	test("rejects names outside the platform admission charset", () => {
		for (const name of [
			"",
			"Flag.Toggled",
			"-flag",
			"flag-",
			"fl ag",
			"flag!",
			"a".repeat(129),
		])
			expect(() => createCustomEventMessage(name, null)).toThrow(TypeError);
		expect(createCustomEventMessage("a", null).name).toBe("a");
		expect(
			createCustomEventMessage("flag.toggled_2-x", null).name,
		).toBe("flag.toggled_2-x");
	});
});

describe("emitCustomEvent", () => {
	test("posts exactly one built message on the sink", () => {
		const sent: unknown[] = [];
		emitCustomEvent({ postMessage: (m) => sent.push(m) }, "flag.toggled", {
			enabled: false,
		});
		expect(sent).toHaveLength(1);
		expect(isCustomEventMessage(sent[0])).toBe(true);
		expect((sent[0] as { payload: unknown }).payload).toEqual({
			enabled: false,
		});
	});
});

describe("isCustomEventMessage", () => {
	test("accepts only exact versioned custom-event messages", () => {
		const good = createCustomEventMessage("x.y", 1);
		expect(isCustomEventMessage(good)).toBe(true);
		expect(isCustomEventMessage({ ...good, version: 2 })).toBe(false);
		expect(isCustomEventMessage({ ...good, type: "other.v1" })).toBe(false);
		expect(isCustomEventMessage({ ...good, name: 3 })).toBe(false);
		const { payload, ...withoutPayload } = good;
		void payload;
		expect(isCustomEventMessage(withoutPayload)).toBe(false);
		expect(isCustomEventMessage(null)).toBe(false);
		expect(isCustomEventMessage("flag.toggled")).toBe(false);
	});
});

class FakePort {
	readonly listeners = new Map<string, Set<(event: unknown) => void>>();
	addEventListener(type: string, listener: (event: unknown) => void) {
		if (!this.listeners.has(type)) this.listeners.set(type, new Set());
		this.listeners.get(type)!.add(listener);
	}
	removeEventListener(type: string, listener: (event: unknown) => void) {
		this.listeners.get(type)?.delete(listener);
	}
	emit(data: unknown) {
		for (const listener of this.listeners.get(PLUGIN_CUSTOM_EVENT_TYPE) ?? [])
			listener({ data });
	}
}

describe("onCustomEvent", () => {
	test("delivers payloads only for the subscribed event name", () => {
		const port = new FakePort();
		const seen: unknown[] = [];
		onCustomEvent(port as never, "flag.toggled", (p) => seen.push(p));
		port.emit(createCustomEventMessage("other.event", 1));
		port.emit(createCustomEventMessage("flag.toggled", { on: true }));
		expect(seen).toEqual([{ on: true }]);
	});

	test("unsubscribe stops delivery and is idempotent", () => {
		const port = new FakePort();
		const seen: number[] = [];
		const stop = onCustomEvent<number>(port as never, "counter.bump", (p) =>
			seen.push(p),
		);
		port.emit(createCustomEventMessage("counter.bump", 1));
		stop();
		stop();
		port.emit(createCustomEventMessage("counter.bump", 2));
		expect(seen).toEqual([1]);
	});
});
