/**
 * Installs the host theme/surface context inside an opaque iframe and renders
 * the scoped root every kit style rule is nested under. It applies only known
 * contract properties as prefixed CSS custom properties and never sets
 * document-global styles.
 */
import type { ParentProps, Ref } from "solid-js";
import type { JSX } from "@solidjs/web";
import type { PluginThemeContractV1, PluginSurfaceContextV1 } from "./theme";
import { resolveTheme, TOKEN_CSS_PROPERTIES } from "./theme";
import type { DispatchIntent } from "./intent";
import { PluginUIContext, type PluginUIContextValue } from "./internal/context";
import { joinClass, setRef } from "./internal/utils";

export interface PluginUIProviderProps extends ParentProps<{
	theme: PluginThemeContractV1;
	surface: PluginSurfaceContextV1;
	dispatchIntent: DispatchIntent;
	class?: string;
	style?: JSX.CSSProperties;
	ref?: Ref<HTMLDivElement>;
}> {}

function kebab(key: string): string {
	return key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
}

/** Serializes validated tokens plus author styles into an inline style string. */
function rootStyle(theme: PluginThemeContractV1, style: JSX.CSSProperties | undefined): string {
	const resolved = resolveTheme(theme);
	const declarations: string[] = [];
	for (const key of Object.keys(TOKEN_CSS_PROPERTIES) as (keyof typeof TOKEN_CSS_PROPERTIES)[]) {
		declarations.push(`${TOKEN_CSS_PROPERTIES[key]}:${resolved.tokens[key]}`);
	}
	if (style) {
		for (const [key, value] of Object.entries(style)) {
			if (value === undefined || value === null) continue;
			declarations.push(`${kebab(key)}:${String(value)}`);
		}
	}
	return declarations.join(";");
}

export function PluginUIProvider(props: PluginUIProviderProps) {
	const scheme = () => resolveTheme(props.theme);
	const contextValue: PluginUIContextValue = {
		theme: scheme,
		surface: () => props.surface,
		dispatchIntent: (intent) => props.dispatchIntent(intent),
	};
	return (
		<PluginUIContext value={contextValue}>
			<div
				ref={(el) => setRef(props.ref, el)}
				class={joinClass("mui-root", props.class)}
				data-macro-plugin-ui=""
				data-macro-theme-v="1"
				data-macro-scheme={scheme().scheme}
				data-macro-contrast={scheme().contrast}
				data-macro-density={scheme().density}
				data-macro-motion={scheme().motion}
				data-macro-placement={props.surface.placement}
				data-macro-size={props.surface.size}
				style={rootStyle(props.theme, props.style)}
			>
				{props.children}
			</div>
		</PluginUIContext>
	);
}
