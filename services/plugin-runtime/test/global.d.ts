declare var __pluginRuntimeHadMacro: boolean | undefined;
declare var __pluginRuntimeContextKeys: string[] | undefined;
declare var __pluginRuntimeSawAbort: boolean | undefined;
declare var __pluginRuntimeGate: {
	held: Array<{ signal: AbortSignal }>;
	releasers: Array<() => void>;
} | undefined;
