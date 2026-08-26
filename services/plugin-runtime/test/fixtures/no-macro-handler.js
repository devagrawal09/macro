// Records whether a macro facade was injected.
export function handle(event, context) {
	globalThis.__pluginRuntimeHadMacro = "macro" in context;
	globalThis.__pluginRuntimeContextKeys = Object.keys(context).sort();
}
