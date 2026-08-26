/**
 * Public provider context. Every kit component reads only this context plus
 * CSS custom properties; nothing reaches into the host document.
 */
import { createContext, useContext } from "solid-js";
import type { ResolvedPluginTheme, PluginSurfaceContextV1 } from "../theme";
import type { DispatchIntent } from "../intent";

export interface PluginUIContextValue {
	/** Reactive resolved theme snapshot. */
	theme(): ResolvedPluginTheme;
	/** Reactive surface context reported by the plugin runtime. */
	surface(): PluginSurfaceContextV1;
	/** Typed intent dispatch supplied by the host bridge. */
	dispatchIntent: DispatchIntent;
}

export const PluginUIContext = createContext<PluginUIContextValue>();

/** Reads the provider context. Components throw outside a provider. */
export function usePluginUI(): PluginUIContextValue {
	const ctx = useContext(PluginUIContext);
	if (!ctx) {
		throw new Error(
			"[macro-plugin-ui] this component must be rendered inside <PluginUIProvider>",
		);
	}
	return ctx;
}

/**
 * Field association context created by <FormField> so inputs inherit label,
 * description/error wiring, and disabled/invalid state.
 */
export interface FieldAssociationValue {
	controlId(): string | undefined;
	describedBy(): string | undefined;
	errorId(): string | undefined;
	fieldInvalid(): boolean;
	fieldDisabled(): boolean;
	fieldRequired(): boolean;
}

export const FieldAssociationContext = createContext<FieldAssociationValue>();

/** Reads the surrounding FormField association, if any. */
export function useOptionalFieldAssociation(): FieldAssociationValue | undefined {
	try {
		return useContext(FieldAssociationContext);
	} catch {
		// No surrounding <FormField>: Solid RC throws ContextNotFoundError.
		return undefined;
	}
}
