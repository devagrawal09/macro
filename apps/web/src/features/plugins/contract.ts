/** Typed host<->plugin iframe message contract for the Macro plugin frame. */

/**
 * Protocol version carried by every port-phase message. Window-phase handshake
 * messages are versioned through their `type` suffix instead.
 */
export const PLUGIN_PROTOCOL_VERSION = 1;

/** Host -> plugin window message that starts one handshake attempt. */
export const PLUGIN_INIT_TYPE = 'macro.plugin.init.v1';
/** Plugin -> host window message acknowledging an init nonce. */
export const PLUGIN_READY_TYPE = 'macro.plugin.ready.v1';
/** Host -> plugin window message transferring the MessagePort. */
export const PLUGIN_CONNECT_TYPE = 'macro.plugin.connect.v1';
/** Host -> plugin port message delivering mount context and the grant. */
export const PLUGIN_CONTEXT_TYPE = 'macro.plugin.context.v1';
/** Plugin -> host port message reporting a sanitized fatal error. */
export const PLUGIN_FATAL_TYPE = 'macro.plugin.fatal.v1';

/** Mount context delivered to a client contribution over the port. */
export interface PluginClientContext {
	readonly projectId: string;
	readonly entity?: { readonly type: string; readonly id: string };
	readonly capabilities: readonly string[];
}

/**
 * Delegated grant delivered to a client contribution over the port.
 * This checkpoint only passes fake local/dev data through props; the real
 * grant minting plugs in behind this shape later.
 */
export interface PluginGrant {
	readonly token: string;
	/** ISO timestamp after which the grant must be refreshed or dropped. */
	readonly expiresAt: string;
	readonly capabilities: readonly string[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function exactKeys(record: Record<string, unknown>, keys: string[]): boolean {
	const actual = Object.keys(record).sort();
	const expected = [...keys].sort();
	return (
		actual.length === expected.length &&
		actual.every((key, index) => key === expected[index])
	);
}

function isNonce(value: unknown): value is string {
	return typeof value === 'string' && value.length > 0;
}

function isClientContext(value: unknown): value is PluginClientContext {
	if (!isRecord(value) || !exactKeys(value, ['projectId', 'capabilities'])) {
		return false;
	}
	return (
		typeof value.projectId === 'string' &&
		Array.isArray(value.capabilities) &&
		value.capabilities.every((item) => typeof item === 'string')
	);
}

function isGrant(value: unknown): value is PluginGrant {
	if (!isRecord(value) || !exactKeys(value, ['token', 'expiresAt', 'capabilities'])) {
		return false;
	}
	return (
		isNonce(value.token) &&
		isNonce(value.expiresAt) &&
		Array.isArray(value.capabilities) &&
		value.capabilities.every((item) => typeof item === 'string')
	);
}

function isTypedNonceMessage(
	value: unknown,
	type: string,
): value is { type: string; nonce: string } {
	return isRecord(value) && exactKeys(value, ['type', 'nonce']) &&
		value.type === type && isNonce(value.nonce);
}

/** Exact `{ type, nonce }` init message sent by the host on iframe load. */
export function isPluginInitMessage(
	value: unknown,
): value is { type: typeof PLUGIN_INIT_TYPE; nonce: string } {
	return isTypedNonceMessage(value, PLUGIN_INIT_TYPE);
}

/** Exact `{ type, nonce }` ready message returned by the plugin bootstrap. */
export function isPluginReadyMessage(
	value: unknown,
): value is { type: typeof PLUGIN_READY_TYPE; nonce: string } {
	return isTypedNonceMessage(value, PLUGIN_READY_TYPE);
}

/** Exact `{ type, nonce }` connect message carrying the transferred port. */
export function isPluginConnectMessage(
	value: unknown,
): value is { type: typeof PLUGIN_CONNECT_TYPE; nonce: string } {
	return isTypedNonceMessage(value, PLUGIN_CONNECT_TYPE);
}

/** Exact context message the host posts on the claimed port. */
export function isPluginContextMessage(
	value: unknown,
): value is {
	version: typeof PLUGIN_PROTOCOL_VERSION;
	type: typeof PLUGIN_CONTEXT_TYPE;
	context: PluginClientContext;
	grant: PluginGrant;
} {
	if (
		!isRecord(value) ||
		!exactKeys(value, ['version', 'type', 'context', 'grant']) ||
		value.version !== PLUGIN_PROTOCOL_VERSION ||
		value.type !== PLUGIN_CONTEXT_TYPE
	) {
		return false;
	}
	return isClientContext(value.context) && isGrant(value.grant);
}

/** Exact sanitized fatal-error message a plugin posts back on the port. */
export function isPluginFatalMessage(
	value: unknown,
): value is {
	version: typeof PLUGIN_PROTOCOL_VERSION;
	type: typeof PLUGIN_FATAL_TYPE;
	message: string;
} {
	return (
		isRecord(value) &&
		exactKeys(value, ['version', 'type', 'message']) &&
		value.version === PLUGIN_PROTOCOL_VERSION &&
		value.type === PLUGIN_FATAL_TYPE &&
		typeof value.message === 'string' &&
		value.message.length > 0
	);
}
