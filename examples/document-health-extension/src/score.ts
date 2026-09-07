export interface DocumentHealth {
	score: number;
	words: number;
	readingMinutes: number;
	longSentences: number;
	openTodos: string[];
	/** Stable hash of the analyzed content. Changes only when the content does. */
	contentHash: string;
	/** ISO timestamp of the analysis that produced this snapshot. */
	analyzedAt: string;
}

export const DOCUMENT_HEALTH_PROPERTY = "Document Health";

/** FNV-1a 32-bit hash, hex encoded. Small, dependency-free, deterministic. */
export function hashContent(content: string): string {
	let hash = 0x811c9dc5;
	for (let index = 0; index < content.length; index++) {
		hash ^= content.charCodeAt(index);
		hash = Math.imul(hash, 0x01000193) >>> 0;
	}
	return hash.toString(16).padStart(8, "0");
}

/** Calculate small, deterministic readability and completeness signals. */
export function analyzeDocument(
	content: string,
	analyzedAt: Date = new Date(),
): DocumentHealth {
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
		contentHash: hashContent(content),
		analyzedAt: analyzedAt.toISOString(),
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
			!parsed.openTodos.every((todo) => typeof todo === "string") ||
			typeof parsed.contentHash !== "string" ||
			typeof parsed.analyzedAt !== "string"
		) {
			return undefined;
		}
		return parsed as DocumentHealth;
	} catch {
		return undefined;
	}
}
