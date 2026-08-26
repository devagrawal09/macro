/** Typed authoring primitives for one Macro Plugin definition. */

/** A Macro capability requested by a plugin entrypoint.
 *
 * Capability names describe intended access. Runtime enforcement is future host work.
 */
export type PluginCapability = string;
/** An immutable list of requested Macro capabilities. */
export type PluginCapabilities = readonly PluginCapability[];

/** Preserve a literal capability tuple for callback inference. */
export function capabilities<const C extends readonly PluginCapability[]>(
	...values: C
): C {
	return values;
}

/** A project supplied by the Macro host. */
export interface PluginProjectContext {
	readonly id: string;
	readonly name?: string;
}

/** Canonical task data returned by the host facade. */
export interface PluginTask {
	readonly id: string;
	readonly projectId: string;
	readonly name: string;
}

/** A live, best-effort task event supplied by the host facade. */
export interface PluginTaskEvent {
	readonly type: string;
	readonly taskId?: string;
	readonly projectId?: string;
}

/** Runtime-neutral task operations supplied by the Macro host. */
export interface PluginTaskOperations {
	list(input: {
		readonly projectId: string;
		readonly signal?: AbortSignal;
	}): Promise<readonly PluginTask[]>;
	read(
		taskId: string,
		input?: { readonly signal?: AbortSignal },
	): Promise<PluginTask>;
	create(input: {
		readonly projectId: string;
		readonly name: string;
		readonly shareWithTeam: boolean;
		readonly signal?: AbortSignal;
	}): Promise<PluginTask>;
	rename(
		taskId: string,
		name: string,
		input?: { readonly signal?: AbortSignal },
	): Promise<PluginTask>;
	subscribe(input: {
		readonly projectId: string;
		readonly signal?: AbortSignal;
	}): AsyncIterable<PluginTaskEvent>;
}

/** Runtime-neutral Macro operations supplied to plugin entrypoints. */
export interface PluginMacroFacade {
	readonly tasks: PluginTaskOperations;
}

/** Structured logger supplied to server reactions. */
export interface PluginLogger {
	info(message: string, fields?: Readonly<Record<string, unknown>>): void;
	error(message: string, fields?: Readonly<Record<string, unknown>>): void;
}

/** Context supplied when Macro mounts a client contribution. */
export interface PluginClientContext<
	C extends PluginCapabilities = PluginCapabilities,
> {
	/** @deprecated Prefer `project.id`; kept during the host contract transition. */
	readonly projectId: string;
	readonly project: PluginProjectContext;
	readonly macro: PluginMacroFacade;
	readonly signal: AbortSignal;
	readonly entity?: { readonly type: string; readonly id: string };
	readonly capabilities: C;
}

/** Context supplied to a best-effort server reaction. */
export interface PluginServerContext<
	C extends PluginCapabilities = PluginCapabilities,
> {
	/** @deprecated Prefer `project.id`; kept during the host contract transition. */
	readonly projectId: string;
	readonly project: PluginProjectContext;
	readonly installationId: string;
	readonly macro: PluginMacroFacade;
	readonly signal: AbortSignal;
	readonly log: PluginLogger;
	readonly capabilities: C;
}

/** A project page contribution. */
export interface ProjectPage<
	C extends PluginCapabilities = PluginCapabilities,
> {
	readonly kind: "project.page";
	readonly id: string;
	readonly capabilities?: C;
	readonly render: (context: PluginClientContext<C>) => unknown;
}

/** Define one project page contribution. */
export function projectPage<const C extends PluginCapabilities>(
	value: Omit<ProjectPage<C>, "kind">,
): ProjectPage<C> {
	return { kind: "project.page", ...value };
}

/** A contextual entity side-panel contribution. */
export interface EntitySidePanel<
	C extends PluginCapabilities = PluginCapabilities,
> {
	readonly kind: "entity.side_panel";
	readonly id: string;
	readonly entityTypes: readonly string[];
	readonly capabilities?: C;
	readonly render: (context: PluginClientContext<C>) => unknown;
}

/** Define one entity side-panel contribution. */
export function entitySidePanel<const C extends PluginCapabilities>(
	value: Omit<EntitySidePanel<C>, "kind">,
): EntitySidePanel<C> {
	return { kind: "entity.side_panel", ...value };
}

/** A deliberately lossy, best-effort server event reaction. */
export interface BestEffortEvent<
	E = unknown,
	C extends PluginCapabilities = PluginCapabilities,
> {
	readonly kind: "best_effort_event";
	readonly id: string;
	readonly event: string;
	readonly capabilities?: C;
	readonly handle: (
		event: E,
		context: PluginServerContext<C>,
	) => void | Promise<void>;
}

/** Define a best-effort event reaction. Events may be missed and are not retried. */
export function onBestEffortEvent<
	E = unknown,
	const C extends PluginCapabilities = PluginCapabilities,
>(value: Omit<BestEffortEvent<E, C>, "kind">): BestEffortEvent<E, C> {
	return { kind: "best_effort_event", ...value };
}

/** Counterparts allowed to emit and observe a plugin-declared custom event. */
export type PluginCustomEventDirection = "client" | "server" | "both";

/** One plugin-declared custom event admitted through the compiler manifest. */
export interface PluginCustomEvent {
	readonly name: string;
	/**
	 * Counterparts allowed to emit and observe this event.
	 *
	 * Defaults to `"both"` when omitted, matching the platform admission default.
	 */
	readonly direction?: PluginCustomEventDirection;
}

/** Protocol version carried by plugin custom-event port messages. */
export const PLUGIN_CUSTOM_EVENT_VERSION = 1;

/** Plugin <-> host port message type delivering one declared custom event emission. */
export const PLUGIN_CUSTOM_EVENT_TYPE = "macro.plugin.custom-event.v1";

/** One plugin-declared custom event routed over the standard MessagePort channel. */
export interface PluginCustomEventMessage<P = unknown> {
	readonly version: typeof PLUGIN_CUSTOM_EVENT_VERSION;
	readonly type: typeof PLUGIN_CUSTOM_EVENT_TYPE;
	readonly name: string;
	readonly payload: P;
}

const CUSTOM_EVENT_NAME = /^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$/;

function assertCustomEventName(name: string): string {
	if (
		typeof name !== "string" ||
		name.length > 128 ||
		!CUSTOM_EVENT_NAME.test(name)
	)
		throw new TypeError(
			`invalid custom event name ${String(name)}: expected 1-128 lowercase letters, digits, '.', '_', or '-' with alphanumeric edges`,
		);
	return name;
}

/** Build one host-routed custom-event port message without sending it. */
export function createCustomEventMessage<P>(
	name: string,
	payload: P,
): PluginCustomEventMessage<P> {
	return {
		version: PLUGIN_CUSTOM_EVENT_VERSION,
		type: PLUGIN_CUSTOM_EVENT_TYPE,
		name: assertCustomEventName(name),
		payload,
	};
}

/** Narrow an unknown port message to a declared custom-event emission. */
export function isCustomEventMessage<P = unknown>(
	value: unknown,
): value is PluginCustomEventMessage<P> {
	if (typeof value !== "object" || value === null) return false;
	const record = value as Record<string, unknown>;
	return (
		record.version === PLUGIN_CUSTOM_EVENT_VERSION &&
		record.type === PLUGIN_CUSTOM_EVENT_TYPE &&
		typeof record.name === "string" &&
		"payload" in record
	);
}

/** Minimal send surface used by {@link emitCustomEvent}; satisfied by MessagePort and test doubles. */
export interface CustomEventSink {
	readonly postMessage: (message: unknown) => void;
}

/** Emit one declared custom event toward the host over the standard port channel. */
export function emitCustomEvent<P>(
	sink: CustomEventSink,
	name: string,
	payload: P,
): void {
	sink.postMessage(createCustomEventMessage(name, payload));
}

/** Listener invoked with one observed custom event payload. */
export type CustomEventListener<P> = (payload: P) => void;

/** Minimal receive surface used by {@link onCustomEvent}; satisfied by MessagePort and test doubles. */
export interface CustomEventSource {
	addEventListener: (
		type: string,
		listener: (event: { readonly data?: unknown }) => void,
	) => void;
	removeEventListener: (
		type: string,
		listener: (event: { readonly data?: unknown }) => void,
	) => void;
}

/**
 * Observe one declared custom event by name on a port-style source.
 * Host-side routing is future wiring; this only defines the message contract.
 * Returns an idempotent unsubscribe function.
 */
export function onCustomEvent<P>(
	source: CustomEventSource,
	name: string,
	listener: CustomEventListener<P>,
): () => void {
	assertCustomEventName(name);
	const handler = (event: { readonly data?: unknown }) => {
		const message = event.data;
		if (isCustomEventMessage<P>(message) && message.name === name)
			listener(message.payload);
	};
	source.addEventListener(PLUGIN_CUSTOM_EVENT_TYPE, handler);
	let unsubscribed = false;
	return () => {
		if (unsubscribed) return;
		unsubscribed = true;
		source.removeEventListener(PLUGIN_CUSTOM_EVENT_TYPE, handler);
	};
}

/** Static definition owned by one default-exported definePlugin call. */
export interface PluginDefinition {
	readonly apiVersion: "1";
	readonly id: string;
	readonly name?: string;
	readonly version: `${number}.${number}.${number}`;
	readonly capabilities?: PluginCapabilities;
	readonly contributions?: readonly (ProjectPage<any> | EntitySidePanel<any>)[];
	readonly handlers?: readonly BestEffortEvent<any, any>[];
	readonly customEvents?: readonly PluginCustomEvent[];
}

/** Define a Macro Plugin. Local check/build executes the trusted definition to enumerate descriptors, but does not invoke entry callbacks. */
export function definePlugin<const P extends PluginDefinition>(
	definition: P,
): P {
	return definition;
}
