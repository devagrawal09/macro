export interface DocumentHealth {
	score: number;
	words: number;
	readingMinutes: number;
	longSentences: number;
	openTodos: string[];
}

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
