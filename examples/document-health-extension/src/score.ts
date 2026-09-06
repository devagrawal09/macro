export interface DocumentHealth {
	score: number;
	words: number;
	readingMinutes: number;
	longSentences: number;
	openTodos: string[];
}

export const DOCUMENT_HEALTH_PROPERTY = "Document Health";

/** Calculate small, deterministic readability and completeness signals. */
export function analyzeDocument(content: string): DocumentHealth {
	const words = content.match(/\b[\w'-]+\b/g)?.length ?? 0;
	const sentences = content
		.split(/[.!?]+(?:\s|$)/)
		.map((sentence) => sentence.trim())
		.filter(Boolean);
	const longSentences = sentences.filter(
		(sentence) => (sentence.match(/\b[\w'-]+\b/g)?.length ?? 0) > 24,
	).length;
	const openTodos = content
		.split("\n")
		.map((line) => line.match(/(?:TODO|FIXME|\[ \])[:\s-]*(.+)/i)?.[1]?.trim())
		.filter((todo): todo is string => Boolean(todo));

	return {
		score: Math.max(0, 100 - longSentences * 8 - openTodos.length * 6),
		words,
		readingMinutes: Math.max(1, Math.ceil(words / 220)),
		longSentences,
		openTodos,
	};
}

/** A short verdict for a score, shown beside the number. */
export function describeHealthScore(score: number): string {
	if (score >= 90) return "Healthy";
	if (score >= 70) return "Needs attention";
	return "At risk";
}

/** Parse a health snapshot stored in a Macro string property. */
export function parseDocumentHealth(
	value: unknown,
): DocumentHealth | undefined {
	if (typeof value !== "string") return undefined;

	try {
		const parsed = JSON.parse(value) as Partial<DocumentHealth>;
		if (
			typeof parsed.score !== "number" ||
			typeof parsed.words !== "number" ||
			typeof parsed.readingMinutes !== "number" ||
			typeof parsed.longSentences !== "number" ||
			!Array.isArray(parsed.openTodos) ||
			!parsed.openTodos.every((todo) => typeof todo === "string")
		) {
			return undefined;
		}
		return parsed as DocumentHealth;
	} catch {
		return undefined;
	}
}
