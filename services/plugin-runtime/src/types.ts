/** Shared runtime-neutral types for the local server-plugin executor. */
import type { PluginServerContext } from "@macro/plugin";

/** One bounded structured log entry produced by a handler invocation. */
export interface LogEntry {
	readonly level: "debug" | "info" | "warn" | "error";
	readonly time: number;
	readonly message: string;
	readonly fields?: Record<string, unknown>;
}

/** A small structured logger with a hard size cap. Excess writes are dropped and counted. */
export interface PluginRuntimeLogger {
	debug(message: string, fields?: Record<string, unknown>): void;
	info(message: string, fields?: Record<string, unknown>): void;
	warn(message: string, fields?: Record<string, unknown>): void;
	error(message: string, fields?: Record<string, unknown>): void;
	/** Captured entries in write order. */
	entries(): readonly LogEntry[];
	/** Number of entries dropped because the size cap was reached. */
	droppedCount(): number;
}

/**
 * Runtime-neutral context passed to one server handler invocation.
 * Extends the authoring `PluginServerContext` with execution controls.
 * `macro` is only present when the executor was constructed with a facade factory.
 */
export interface PluginHandlerContext extends PluginServerContext {
	/** Aborts when the platform invocation deadline fires. */
	readonly signal: AbortSignal;
	/** Epoch milliseconds at which the invocation times out. */
	readonly deadline: number;
	/** Bounded structured logger scoped to this invocation. */
	readonly log: PluginRuntimeLogger;
}

/** A compiled server handler as emitted by `macro plugin build`. */
export type ServerHandler = (
	event: unknown,
	context: PluginHandlerContext,
) => void | Promise<void>;

/** Coordinates handed to the facade factory so it can build a scoped SDK view. */
export interface FacadeInfo {
	readonly projectId: string;
	readonly installationId: string;
	readonly capabilities: readonly string[];
	readonly eventId: string;
	readonly eventType: string;
}

/** Creates the Macro facade injected into handler contexts. Callers supply real or fake SDKs. */
export type FacadeFactory<F = unknown> = (info: FacadeInfo) => F;

/** Terminal outcome of one admission attempt. */
export type InvocationOutcome =
	| { readonly status: "completed"; readonly runId: string; readonly durationMs: number }
	| {
			readonly status: "failed";
			readonly runId: string;
			readonly durationMs: number;
			readonly error: InvocationError;
	  }
	| {
			readonly status: "timeout";
			readonly runId: string;
			readonly durationMs: number;
			readonly error: InvocationError;
	  }
	| { readonly status: "dropped"; readonly reason: "concurrency_limit" }
	| { readonly status: "skipped"; readonly reason: "event_mismatch" };

/** Bounded failure detail. Never carries event payloads or log output. */
export interface InvocationError {
	readonly code:
		| "handler_error"
		| "timeout"
		| "bundle_load_error"
		| "handler_not_found";
	readonly message: string;
}

/** In-memory record of one admitted (started) invocation for the current process. */
export interface RunRecord {
	readonly runId: string;
	readonly eventId: string;
	readonly eventType: string;
	readonly startedAt: number;
	finishedAt?: number;
	status: "running" | "completed" | "failed" | "timeout";
	failureCode?: string;
	failureMessage?: string;
}

/** Executor counters for the current process. */
export interface ExecutorStats {
	readonly active: number;
	readonly concurrencyLimit: number;
	readonly droppedByConcurrency: number;
	readonly completed: number;
	readonly failed: number;
	readonly timedOut: number;
}
