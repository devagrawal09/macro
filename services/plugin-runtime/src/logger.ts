import type { LogEntry, PluginRuntimeLogger } from "./types";

const MAX_ENTRIES = 100;
const MAX_TOTAL_CHARS = 8192;
const MAX_FIELD_CHARS = 512;

function boundedFields(
	fields?: Record<string, unknown>,
): Record<string, unknown> | undefined {
	if (!fields) return undefined;
	const serialized = JSON.stringify(fields) ?? "";
	if (serialized.length <= MAX_FIELD_CHARS) return fields;
	return {
		truncated: `${serialized.slice(0, MAX_FIELD_CHARS)}...`,
	};
}

/**
 * Create one invocation-scoped structured logger.
 * Entries are kept in memory up to a fixed entry-count and total-size cap;
 * further writes are dropped and counted. No persistence, no transport.
 */
export function createCappedLogger(): PluginRuntimeLogger {
	const entries: LogEntry[] = [];
	let totalChars = 0;
	let dropped = 0;
	function write(
		level: LogEntry["level"],
		message: string,
		fields?: Record<string, unknown>,
	): void {
		if (entries.length >= MAX_ENTRIES) {
			dropped += 1;
			return;
		}
		const entry: LogEntry = {
			level,
			time: Date.now(),
			message,
			fields: boundedFields(fields),
		};
		const size = JSON.stringify(entry).length;
		if (totalChars + size > MAX_TOTAL_CHARS) {
			dropped += 1;
			return;
		}
		totalChars += size;
		entries.push(entry);
	}
	return {
		debug: (message, fields) => write("debug", message, fields),
		info: (message, fields) => write("info", message, fields),
		warn: (message, fields) => write("warn", message, fields),
		error: (message, fields) => write("error", message, fields),
		entries: () => entries,
		droppedCount: () => dropped,
	};
}
