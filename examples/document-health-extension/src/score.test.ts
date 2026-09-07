import { describe, expect, test } from "bun:test";
import {
	analyzeDocument,
	describeHealthScore,
	hashContent,
	parseDocumentHealth,
} from "./score";

describe("analyzeDocument", () => {
	test("penalizes long sentences and open work", () => {
		const health = analyzeDocument(
			"Overview\n\n" +
				"This sentence contains far more than twenty four separate words so the readability signal can identify prose that should probably be broken into smaller ideas for teammates.\n\n" +
				"TODO: confirm the launch date\n- [ ] Add customer evidence",
		);

		expect(health.score).toBe(80);
		expect(health.longSentences).toBe(1);
		expect(health.openTodos).toEqual([
			"confirm the launch date",
			"Add customer evidence",
		]);
		expect(health.readingMinutes).toBe(1);
	});

	test("returns a perfect score for concise completed prose", () => {
		expect(analyzeDocument("A short, finished document.").score).toBe(100);
	});

	test("stamps a stable content hash and the analysis time", () => {
		const at = new Date("2026-09-07T00:00:00.000Z");
		const first = analyzeDocument("same words", at);
		const second = analyzeDocument("same words", new Date());
		expect(first.contentHash).toBe(hashContent("same words"));
		expect(first.contentHash).toBe(second.contentHash);
		expect(first.analyzedAt).toBe("2026-09-07T00:00:00.000Z");
		expect(analyzeDocument("other words").contentHash).not.toBe(
			first.contentHash,
		);
	});

	test("describes score bands", () => {
		expect(describeHealthScore(100)).toBe("Healthy");
		expect(describeHealthScore(90)).toBe("Healthy");
		expect(describeHealthScore(89)).toBe("Needs attention");
		expect(describeHealthScore(70)).toBe("Needs attention");
		expect(describeHealthScore(69)).toBe("At risk");
	});

	test("parses a stored health snapshot", () => {
		const health = analyzeDocument("TODO: ship it");
		expect(parseDocumentHealth(JSON.stringify(health))).toEqual(health);
		expect(parseDocumentHealth("not json")).toBeUndefined();
		const { contentHash: _, ...legacy } = health;
		expect(parseDocumentHealth(JSON.stringify(legacy))).toBeUndefined();
	});
});
