/** Typed authoring primitives for one Macro Plugin definition. */

/** A Macro capability requested by a plugin entrypoint. */
export type PluginCapability = string;
/** An immutable list of requested Macro capabilities. */
export type PluginCapabilities = readonly PluginCapability[];

/** Preserve a literal capability tuple for callback inference. */
export function capabilities<const C extends readonly PluginCapability[]>(
	...values: C
): C {
	return values;
}

/** Context supplied when Macro mounts a client contribution. */
export interface PluginClientContext<
	C extends PluginCapabilities = PluginCapabilities,
> {
	readonly projectId: string;
	readonly entity?: { readonly type: string; readonly id: string };
	readonly capabilities: C;
}

/** Context supplied to a best-effort server reaction. */
export interface PluginServerContext<
	C extends PluginCapabilities = PluginCapabilities,
> {
	readonly projectId: string;
	readonly installationId: string;
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

/** Static definition owned by one default-exported definePlugin call. */
export interface PluginDefinition {
	readonly apiVersion: "1";
	readonly id: string;
	readonly name?: string;
	readonly version: `${number}.${number}.${number}`;
	readonly capabilities?: PluginCapabilities;
	readonly contributions?: readonly (ProjectPage<any> | EntitySidePanel<any>)[];
	readonly handlers?: readonly BestEffortEvent<any, any>[];
}

/** Define a Macro Plugin. The CLI parses this call statically and never executes it. */
export function definePlugin<const P extends PluginDefinition>(
	definition: P,
): P {
	return definition;
}
