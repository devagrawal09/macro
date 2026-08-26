/**
 * Plugin-side iframe bootstrap for the Macro plugin frame.
 *
 * This code runs INSIDE the sandboxed opaque-origin plugin iframe. It is
 * serialized into the srcdoc document via `serializePluginBootstrapSource`,
 * so `installPluginBootstrap` must stay fully self-contained: no imports,
 * no module-scope captures. It mirrors the exact-payload checks from
 * `../contract` locally for that reason.
 */

/** Minimal window surface the bootstrap touches; keeps tests honest. */
export interface BootstrapWindow {
	parent: {
		postMessage(
			message: unknown,
			targetOrigin: string,
			transfer?: Transferable[],
		): void;
	};
	addEventListener(
		type: 'message',
		listener: (event: {
			data?: unknown;
			ports?: MessagePort[];
		}) => void,
	): void;
	document: Document;
}

interface PluginMountContext {
	readonly projectId?: string;
}

/**
 * Install the nonce-bound handshake listener and mount the client entry on
 * the delivered port context. Accepts only exact init/connect payloads from
 * the parent window and reports sanitized fatal errors back over the port.
 */
export function installPluginBootstrap(win: BootstrapWindow): void {
	const PLUGIN_INIT_TYPE = 'macro.plugin.init.v1';
	const PLUGIN_READY_TYPE = 'macro.plugin.ready.v1';
	const PLUGIN_CONNECT_TYPE = 'macro.plugin.connect.v1';
	const PLUGIN_CONTEXT_TYPE = 'macro.plugin.context.v1';
	const PLUGIN_FATAL_TYPE = 'macro.plugin.fatal.v1';
	const PLUGIN_PROTOCOL_VERSION = 1;

	const isExactTypedNonce = (
		value: unknown,
		type: string,
	): value is { type: string; nonce: string } =>
		typeof value === 'object' &&
		value !== null &&
		!Array.isArray(value) &&
		Object.keys(value as Record<string, unknown>).sort().join(',') ===
			'nonce,type' &&
		(value as Record<string, unknown>).type === type &&
		typeof (value as Record<string, unknown>).nonce === 'string' &&
		((value as Record<string, unknown>).nonce as string).length > 0;

	const isContextPayload = (value: unknown): boolean => {
		if (
			typeof value !== 'object' ||
			value === null ||
			Array.isArray(value) ||
			Object.keys(value as Record<string, unknown>).sort().join(',') !==
				'context,grant,type,version'
		) {
			return false;
		}
		const record = value as Record<string, unknown>;
		return (
			record.version === PLUGIN_PROTOCOL_VERSION &&
			record.type === PLUGIN_CONTEXT_TYPE
		);
	};

	let nonce: string | null = null;
	let port: MessagePort | undefined;

	const reportFatal = (message: string) => {
		try {
			port?.postMessage({
				version: PLUGIN_PROTOCOL_VERSION,
				type: PLUGIN_FATAL_TYPE,
				message,
			});
		} catch {
			// The host is gone or the port closed; nothing else to do.
		}
	};

	win.addEventListener('message', (event) => {
		if (isExactTypedNonce(event.data, PLUGIN_INIT_TYPE)) {
			port?.close();
			port = undefined;
			nonce = event.data.nonce;
			win.parent.postMessage({ type: PLUGIN_READY_TYPE, nonce }, '*');
			return;
		}
		if (
			!isExactTypedNonce(event.data, PLUGIN_CONNECT_TYPE) ||
			event.data.nonce !== nonce ||
			event.ports?.length !== 1
		) {
			return;
		}
		const connectedNonce = nonce;
		port?.close();
		port = event.ports[0];
		port.onmessage = (portEvent) => {
			if (nonce !== connectedNonce || !isContextPayload(portEvent.data)) {
				return;
			}
			const context = (portEvent.data as Record<string, unknown>)
				.context as PluginMountContext;
			try {
				const root = win.document.getElementById('root');
				if (!root) throw new Error('plugin root element is missing');
				const entry = (
					globalThis as {
						__MACRO_PLUGIN_ENTRY__?: {
							mount?: (
								root: HTMLElement,
								context: PluginMountContext,
							) => unknown;
						};
					}
				).__MACRO_PLUGIN_ENTRY__;
				if (typeof entry?.mount !== 'function') {
					throw new Error('plugin entry is missing');
				}
				entry.mount(root, context);
			} catch (error) {
				reportFatal(
					error instanceof Error ? error.message : String(error),
				);
			}
		};
		port.start?.();
	});
}

/** Serialize the bootstrap so it can run inside a srcdoc document. */
export function serializePluginBootstrapSource(): string {
	return `(${installPluginBootstrap.toString()})(window);`;
}
