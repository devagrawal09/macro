/** Public API of the local Macro Plugin server executor (trusted local/dev only). */
export {
	CONCURRENCY_LIMIT,
	INVOCATION_TIMEOUT_MS,
	ServerPluginExecutor,
	createServerPluginExecutor,
} from "./executor";
export type {
	ExecutorOptions,
	InvokeRequest,
} from "./executor";
export { createCappedLogger } from "./logger";
export type {
	ExecutorStats,
	FacadeFactory,
	FacadeInfo,
	InvocationError,
	InvocationOutcome,
	LogEntry,
	PluginHandlerContext,
	PluginRuntimeLogger,
	RunRecord,
	ServerHandler,
} from "./types";
export { runOnceMain } from "./cli";
