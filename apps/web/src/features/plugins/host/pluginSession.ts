import {
	isPluginContextMessage,
	isPluginFatalMessage,
	isPluginReadyMessage,
	PLUGIN_CONNECT_TYPE,
	PLUGIN_CONTEXT_TYPE,
	PLUGIN_INIT_TYPE,
	PLUGIN_PROTOCOL_VERSION,
	type PluginClientContext,
	type PluginGrant,
} from '../contract';

/** Lifecycle phase of one plugin frame session. */
export type PluginSessionState =
	| { phase: 'loading' }
	| { phase: 'handshaking' }
	| { phase: 'ready' }
	| { phase: 'error'; reason: string };

export const DEFAULT_HANDSHAKE_TIMEOUT_MS = 10_000;

export interface PluginSessionOptions {
	/** Current iframe node, or undefined while unmounted. */
	getFrame: () => HTMLIFrameElement | undefined;
	/** Mount context delivered to the plugin once the handshake completes. */
	context: PluginClientContext;
	/**
	 * Grant seam. This checkpoint passes fake local/dev data; real grant
	 * minting plugs in here later without touching the session machinery.
	 */
	fetchGrant: (context: PluginClientContext) => Promise<PluginGrant>;
	onStateChange?: (state: PluginSessionState) => void;
	timeoutMs?: number;
}

interface SessionAttempt {
	generation: number;
	nonce: string | null;
	claimed: boolean;
	port1?: MessagePort;
	port2?: MessagePort;
}

function samePhase(a: PluginSessionState, b: PluginSessionState): boolean {
	if (a.phase !== b.phase) return false;
	return a.phase !== 'error' || b.phase !== 'error' || a.reason === b.reason;
}

/**
 * Host-side plugin frame session: opaque-origin handshake with timeout and
 * retry, generation invalidation, port lifecycle, and error surfacing.
 * Mirrors the proven HtmlPreview gates (sandbox allow-scripts srcdoc frames
 * have origin "null"): exact source window, exact null origin, exact payload
 * keys, one-time nonce, current generation.
 */
export function createPluginSession(options: PluginSessionOptions): {
	handleLoad: () => void;
	retry: () => void;
	dispose: () => void;
} {
	const timeoutMs = options.timeoutMs ?? DEFAULT_HANDSHAKE_TIMEOUT_MS;
	let active = true;
	let generation = 0;
	let attempt: SessionAttempt | undefined;
	let timeoutHandle: ReturnType<typeof setTimeout> | undefined;
	let state: PluginSessionState = { phase: 'loading' };
	let messageListenerInstalled = false;

	const setState = (next: PluginSessionState) => {
		if (!active || samePhase(state, next)) return;
		state = next;
		options.onStateChange?.(next);
	};

	const clearTimer = () => {
		if (timeoutHandle === undefined) return;
		clearTimeout(timeoutHandle);
		timeoutHandle = undefined;
	};

	const invalidate = () => {
		generation += 1;
		clearTimer();
		attempt?.port1?.close();
		attempt?.port2?.close();
		attempt = undefined;
	};

	const fail = (reason: string) => {
		invalidate();
		setState({ phase: 'error', reason });
	};

	const onMessage = async (event: MessageEvent) => {
		const node = options.getFrame();
		const current = attempt;
		if (
			!active ||
			!node ||
			event.source !== node.contentWindow ||
			event.origin !== 'null' ||
			!isPluginReadyMessage(event.data) ||
			!current ||
			current.claimed ||
			current.nonce !== event.data.nonce ||
			current.generation !== generation
		) {
			return;
		}

		// Claim and consume synchronously, before the first await.
		current.claimed = true;
		const captured = {
			generation: current.generation,
			nonce: current.nonce as string,
			frame: node,
			controller: current,
		} as const;
		current.nonce = null;

		let grant: PluginGrant;
		try {
			grant = await options.fetchGrant(options.context);
		} catch {
			fail('grant-failed');
			return;
		}

		if (
			!active ||
			options.getFrame() !== captured.frame ||
			!captured.frame.contentWindow ||
			generation !== captured.generation ||
			attempt !== captured.controller ||
			!captured.controller.claimed
		) {
			return;
		}

		const channel = new MessageChannel();
		captured.controller.port1 = channel.port1;
		captured.controller.port2 = channel.port2;
		channel.port1.onmessage = (portEvent) => {
			// The port stays open for future typed host intents; only fatal
			// plugin errors are surfaced in this checkpoint.
			if (
				active &&
				attempt === captured.controller &&
				captured.controller.generation === generation &&
				isPluginFatalMessage(portEvent.data)
			) {
				fail(`plugin-fatal:${portEvent.data.message}`);
			}
		};
		try {
			captured.frame.contentWindow.postMessage(
				{ type: PLUGIN_CONNECT_TYPE, nonce: captured.nonce },
				'*',
				[channel.port2],
			);
			captured.controller.port2 = undefined;
			channel.port1.postMessage({
				version: PLUGIN_PROTOCOL_VERSION,
				type: PLUGIN_CONTEXT_TYPE,
				context: options.context,
				grant,
			});
		} catch {
			channel.port1.close();
			channel.port2.close();
			fail('connect-failed');
			return;
		}
		clearTimer();
		setState({ phase: 'ready' });
	};

	const ensureListener = () => {
		if (messageListenerInstalled || typeof window === 'undefined') return;
		window.addEventListener('message', onMessage);
		messageListenerInstalled = true;
	};

	/** Start or restart the handshake for the current iframe load. */
	const handleLoad = () => {
		if (!active) return;
		const node = options.getFrame();
		if (!node?.contentWindow) return;
		invalidate();
		ensureListener();
		const nonce = crypto.randomUUID();
		attempt = { generation, nonce, claimed: false };
		setState({ phase: 'handshaking' });
		node.contentWindow.postMessage({ type: PLUGIN_INIT_TYPE, nonce }, '*');
		timeoutHandle = setTimeout(() => {
			if (
				!active ||
				!attempt ||
				attempt.claimed ||
				attempt.generation !== generation
			) {
				return;
			}
			fail('handshake-timeout');
		}, timeoutMs);
	};

	const retry = () => {
		if (!active) return;
		handleLoad();
	};

	const dispose = () => {
		if (!active) return;
		active = false;
		if (messageListenerInstalled && typeof window !== 'undefined') {
			window.removeEventListener('message', onMessage);
			messageListenerInstalled = false;
		}
		invalidate();
		setState({ phase: 'error', reason: 'disposed' });
	};

	return { handleLoad, retry, dispose };
}
