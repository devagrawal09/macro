// Blocks until the test releases it via the shared global gate.
export function handle(event, context) {
	globalThis.__pluginRuntimeGate.held.push(context);
	return new Promise((resolve) => {
		globalThis.__pluginRuntimeGate.releasers.push(resolve);
	});
}
