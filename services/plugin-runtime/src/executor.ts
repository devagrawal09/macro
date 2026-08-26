import { randomUUID } from "node:crypto";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { createCappedLogger } from "./logger";
import type {
	ExecutorStats,
	FacadeFactory,
	InvocationError,
	InvocationOutcome,
	RunRecord,
	ServerHandler,
} from "./types";

/** Fixed platform invocation timeout. Plugin authors cannot override it. */
export const INVOCATION_TIMEOUT_MS = 30_000;
/** Fixed platform global concurrency limit. Excess admissions are dropped immediately. */
export const CONCURRENCY_LIMIT = 8;

const MAX_FAILURE_MESSAGE_CHARS = 512;

export interface ExecutorOptions<F = unknown> {
	/** Facade factory used to build the `macro` SDK view injected into handler contexts. */
	createMacro?: FacadeFactory<F>;
	/** Test-only override of the platform-fixed timeout. Defaults to {@link INVOCATION_TIMEOUT_MS}. */
	timeoutMsForTests?: number;
	/** Test-only override of the platform-fixed concurrency limit. Defaults to {@link CONCURRENCY_LIMIT}. */
	concurrencyLimitForTests?: number;
}

export interface InvokeRequest {
	/** Path to a compiled server entry bundle (`server/<entry>/index.js`). TypeScript source is never evaluated. */
	bundlePath: string;
	/** Named export to call. Defaults to `handle`, falling back to the module default export. */
	exportName?: string;
	readonly eventId: string;
	readonly eventType: string;
	/** Declared event the bundle handles. When set and mismatched with `eventType`, the invocation is a no-op. */
	expectedEventType?: string;
	readonly event: unknown;
	readonly projectId: string;
	readonly installationId: string;
	readonly capabilities?: readonly string[];
}

function boundedMessage(error: unknown): string {
	const raw = error instanceof Error ? error.message : String(error);
	return raw.length <= MAX_FAILURE_MESSAGE_CHARS
		? raw
		: `${raw.slice(0, MAX_FAILURE_MESSAGE_CHARS)}...`;
}

/**
 * Local in-process executor for compiled Macro Plugin server bundles.
 *
 * Trust model: this runs trusted local plugin code inside this Bun process.
 * Bun is NOT a security sandbox. There is no isolation claim beyond the
 * concurrency limit, the abort-signal deadline, and bounded logging.
 * Live-only: run records exist in memory for the current process only.
 */
export class ServerPluginExecutor<F = unknown> {
	readonly #limit: number;
	readonly #timeoutMs: number;
	readonly #createMacro?: FacadeFactory<F>;
	#active = 0;
	#dropped = 0;
	#completed = 0;
	#failed = 0;
	#timedOut = 0;
	readonly #runs: RunRecord[] = [];

	constructor(options: ExecutorOptions<F> = {}) {
		this.#createMacro = options.createMacro;
		this.#timeoutMs =
			options.timeoutMsForTests !== undefined &&
			options.timeoutMsForTests > 0
				? options.timeoutMsForTests
				: INVOCATION_TIMEOUT_MS;
		this.#limit =
			options.concurrencyLimitForTests !== undefined &&
			options.concurrencyLimitForTests > 0
				? options.concurrencyLimitForTests
				: CONCURRENCY_LIMIT;
	}

	/** In-memory records of admitted invocations only; drops never create records. */
	runs(): readonly RunRecord[] {
		return this.#runs;
	}

	counters(): ExecutorStats {
		return {
			active: this.#active,
			concurrencyLimit: this.#limit,
			droppedByConcurrency: this.#dropped,
			completed: this.#completed,
			failed: this.#failed,
			timedOut: this.#timedOut,
		};
	}

	async invoke(request: InvokeRequest): Promise<InvocationOutcome> {
		if (
			request.expectedEventType !== undefined &&
			request.expectedEventType !== request.eventType
		)
			return { status: "skipped", reason: "event_mismatch" };
		if (this.#active >= this.#limit) {
			this.#dropped += 1;
			return { status: "dropped", reason: "concurrency_limit" };
		}
		this.#active += 1;
		const startedAt = Date.now();
		const record: RunRecord = {
			runId: randomUUID(),
			eventId: request.eventId,
			eventType: request.eventType,
			startedAt,
			status: "running",
		};
		this.#runs.push(record);
		try {
			const outcome = await this.#run(request, record);
			return outcome;
		} finally {
			record.finishedAt = Date.now();
			this.#active -= 1;
		}
	}

	async #run(
		request: InvokeRequest,
		record: RunRecord,
	): Promise<InvocationOutcome> {
		const finish = (
			status: "completed" | "failed" | "timeout",
			error?: InvocationError,
		): InvocationOutcome => {
			const durationMs = Date.now() - record.startedAt;
			record.status = status;
			if (status === "completed") {
				this.#completed += 1;
				return { status, runId: record.runId, durationMs };
			}
			if (status === "failed") this.#failed += 1;
			else this.#timedOut += 1;
			const detail: InvocationError =
				error ?? { code: "handler_error", message: "unknown failure" };
			record.failureCode = detail.code;
			record.failureMessage = detail.message;
			return { status, runId: record.runId, durationMs, error: detail };
		};

		const bundlePath = resolve(request.bundlePath);
		if (!/[.][cm]?js$/.test(bundlePath))
			return finish("failed", {
				code: "bundle_load_error",
				message:
					"only compiled .js/.cjs/.mjs bundles may be executed; TypeScript source is never evaluated",
			});

		let handler: unknown;
		try {
			const module = await import(pathToFileURL(bundlePath).href);
			const named =
				request.exportName !== undefined
					? (module as Record<string, unknown>)[request.exportName]
					: (module as Record<string, unknown>).handle;
			handler = named ?? (module as Record<string, unknown>).default;
		} catch (error) {
			return finish("failed", {
				code: "bundle_load_error",
				message: boundedMessage(error),
			});
		}
		if (typeof handler !== "function")
			return finish("failed", {
				code: "handler_not_found",
				message: `no callable ${
					request.exportName ?? "handle"
				} or default export in ${bundlePath}`,
			});

		const signal = AbortSignal.timeout(this.#timeoutMs);
		let deadlineFired = false;
		if (signal.aborted) deadlineFired = true;
		else
			signal.addEventListener("abort", () => {
				deadlineFired = true;
			}, { once: true });
		const log = createCappedLogger();
		const context: Record<string, unknown> = {
			projectId: request.projectId,
			installationId: request.installationId,
			capabilities: request.capabilities ?? [],
			signal,
			deadline: record.startedAt + this.#timeoutMs,
			log,
		};
		if (this.#createMacro)
			context.macro = this.#createMacro({
				projectId: request.projectId,
				installationId: request.installationId,
				capabilities: request.capabilities ?? [],
				eventId: request.eventId,
				eventType: request.eventType,
			});

		const TIMED_OUT = Symbol("macro-plugin-runtime-timeout");
		let timer: ReturnType<typeof setTimeout> | undefined;
		const deadline = new Promise<typeof TIMED_OUT>((settled) => {
			timer = setTimeout(() => settled(TIMED_OUT), this.#timeoutMs);
			timer.unref?.();
		});
		try {
			const settled = await Promise.race([
				Promise.resolve((handler as ServerHandler)(
					request.event,
					context as unknown as Parameters<ServerHandler>[1],
				)).then(() => null),
				deadline,
			]);
			return settled === TIMED_OUT || deadlineFired
				? finish("timeout", {
						code: "timeout",
						message: `invocation exceeded the ${this.#timeoutMs}ms deadline`,
					})
				: finish("completed");
		} catch (error) {
			return deadlineFired
				? finish("timeout", {
						code: "timeout",
						message: `invocation exceeded the ${this.#timeoutMs}ms deadline`,
					})
				: finish("failed", {
						code: "handler_error",
						message: boundedMessage(error),
					});
		} finally {
			clearTimeout(timer);
		}
	}
}

export function createServerPluginExecutor<F = unknown>(
	options: ExecutorOptions<F> = {},
): ServerPluginExecutor<F> {
	return new ServerPluginExecutor<F>(options);
}
