// Hangs until the platform deadline aborts its signal; records what it saw.
export function handle(event, context) {
	return new Promise((resolve) => {
		context.signal.addEventListener(
			"abort",
			() => {
				globalThis.__pluginRuntimeSawAbort = context.signal.aborted;
				resolve();
			},
			{ once: true },
		);
	});
}
