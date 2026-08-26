import type { PluginLogger } from "@macro/plugin";

export interface FlagToggledPayload {
	readonly enabled: boolean;
	readonly actor?: string;
}

/** Last payload observed by this process; mirrors the compiler fixture convention for local demos. */
export function lastObservedFlag(): FlagToggledPayload | undefined {
	return (globalThis as { __demoFlagLast?: FlagToggledPayload })
		.__demoFlagLast;
}

/** React to one best-effort `flag.toggled` emission with bounded structured logging. */
export async function handleFlagToggled(
	event: FlagToggledPayload,
	log: PluginLogger,
): Promise<void> {
	(globalThis as { __demoFlagLast?: FlagToggledPayload }).__demoFlagLast =
		event;
	log.info("demo-flag-plugin handled flag.toggled", {
		enabled: event.enabled,
		actor: event.actor ?? "unknown",
	});
}
