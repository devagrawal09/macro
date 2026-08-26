import { expect, test } from "bun:test";
import { createCappedLogger } from "../src/logger";

test("logger caps entries and counts dropped writes", () => {
	const log = createCappedLogger();
	for (let i = 0; i < 500; i += 1)
		log.info(`entry ${i}`, { index: i, blob: "x".repeat(64) });
	const entries = log.entries();
	expect(entries.length).toBeLessThanOrEqual(100);
	expect(log.droppedCount()).toBeGreaterThan(0);
	// Entries are structured and ordered.
	expect(entries[0]?.level).toBe("info");
	expect(entries[0]?.message).toBe("entry 0");
	for (const entry of entries) {
		expect(entry.time).toBeNumber();
		expect(JSON.stringify(entry).length).toBeLessThan(1024);
	}
});

test("logger bounds oversized fields", () => {
	const log = createCappedLogger();
	log.warn("big", { blob: "y".repeat(10_000) });
	const [entry] = log.entries();
	expect(entry?.fields?.truncated).toBeString();
	expect(JSON.stringify(entry).length).toBeLessThan(1024);
});
