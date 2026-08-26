/**
 * Versioned host theme/surface contracts (v1).
 *
 * The host sends fully resolved colors across the plugin bridge. The provider
 * maps a validated contract onto prefixed CSS custom properties. Unknown
 * versions fall back to readable built-in Macro Light/Dark tokens.
 */

export type PluginPlacement = "project.page" | "entity.side_panel";
export type PluginContainerSize = "narrow" | "compact" | "wide";
export type PluginDensity = "comfortable" | "compact";
export type PluginContrast = "normal" | "high";
export type PluginMotion = "full" | "reduced";

/** Semantic color tokens, version 1. Values are resolved CSS colors. */
export interface PluginThemeTokensV1 {
	page: string;
	surface: string;
	surfaceRaised: string;
	surfaceInset: string;
	ink: string;
	inkMuted: string;
	inkSubtle: string;
	inkDisabled: string;
	edge: string;
	edgeMuted: string;
	accent: string;
	accentInk: string;
	hover: string;
	active: string;
	selected: string;
	success: string;
	warning: string;
	failure: string;
	focusRing: string;
}

export interface PluginThemeContractV1 {
	version: 1;
	mode: "light" | "dark";
	contrast: PluginContrast;
	density: PluginDensity;
	motion: PluginMotion;
	tokens: PluginThemeTokensV1;
}

export interface PluginSurfaceContextV1 {
	version: 1;
	placement: PluginPlacement;
	size: PluginContainerSize;
	widthPx: number;
	heightPx: number;
}

/** Maps token names to public CSS custom properties. */
export const TOKEN_CSS_PROPERTIES: Record<keyof PluginThemeTokensV1, string> = {
	page: "--macro-color-page",
	surface: "--macro-color-surface",
	surfaceRaised: "--macro-color-surface-raised",
	surfaceInset: "--macro-color-surface-inset",
	ink: "--macro-color-ink",
	inkMuted: "--macro-color-ink-muted",
	inkSubtle: "--macro-color-ink-subtle",
	inkDisabled: "--macro-color-ink-disabled",
	edge: "--macro-color-edge",
	edgeMuted: "--macro-color-edge-muted",
	accent: "--macro-color-accent",
	accentInk: "--macro-color-accent-ink",
	hover: "--macro-color-hover",
	active: "--macro-color-active",
	selected: "--macro-color-selected",
	success: "--macro-color-success",
	warning: "--macro-color-warning",
	failure: "--macro-color-failure",
	focusRing: "--macro-color-focus-ring",
};

/**
 * Built-in readable fallback tokens mirroring Macro Light (warm neutral
 * surfaces, warm accent). Used when the host contract is missing or invalid.
 */
export const MACRO_LIGHT_TOKENS: PluginThemeTokensV1 = {
	page: "#f5f4f0",
	surface: "#faf9f6",
	surfaceRaised: "#ffffff",
	surfaceInset: "#efeee9",
	ink: "#22211e",
	inkMuted: "#69665f",
	inkSubtle: "#949088",
	inkDisabled: "#aaa69e",
	edge: "#ddd9d0",
	edgeMuted: "#e8e4dc",
	accent: "#d98e17",
	accentInk: "#2c1b00",
	hover: "rgba(34, 33, 30, 0.045)",
	active: "rgba(34, 33, 30, 0.08)",
	selected: "rgba(217, 142, 23, 0.11)",
	success: "#168a50",
	warning: "#b66b00",
	failure: "#c93f3f",
	focusRing: "#a96300",
};

/** Built-in readable fallback tokens mirroring Macro Dark. */
export const MACRO_DARK_TOKENS: PluginThemeTokensV1 = {
	page: "#171714",
	surface: "#20201c",
	surfaceRaised: "#292923",
	surfaceInset: "#121210",
	ink: "#f2f0e9",
	inkMuted: "#c4c0b6",
	inkSubtle: "#8f8c84",
	inkDisabled: "#68665f",
	edge: "#46443d",
	edgeMuted: "#34332d",
	accent: "#f3b329",
	accentInk: "#261800",
	hover: "rgba(255, 255, 255, 0.045)",
	active: "rgba(255, 255, 255, 0.085)",
	selected: "rgba(243, 179, 41, 0.12)",
	success: "#68cf91",
	warning: "#f5bd55",
	failure: "#ff7d77",
	focusRing: "#ffd36d",
};

export interface ResolvedPluginTheme {
	scheme: "light" | "dark";
	contrast: PluginContrast;
	density: PluginDensity;
	motion: PluginMotion;
	tokens: PluginThemeTokensV1;
	fallbackUsed: boolean;
}

let warnedAbout = new Set<string>();

function warnOnce(key: string, message: string): void {
	if (warnedAbout.has(key)) return;
	warnedAbout.add(key);
	console.warn(`[macro-plugin-ui] ${message}`);
}

/** Test hook: clears accumulated development warnings. */
export function resetThemeWarningsForTest(): void {
	warnedAbout = new Set<string>();
}

function isUnsafeColor(value: unknown): boolean {
	return (
		typeof value !== "string" ||
		value.trim().length === 0 ||
		/url\s*\(/i.test(value) ||
		/expression\s*\(/i.test(value) ||
		/javascript:/i.test(value)
	);
}

/**
 * Validates a host theme contract and resolves concrete tokens. Unknown
 * versions, wrong shapes, or unsafe token values fall back to the built-in
 * Macro Light/Dark tokens for the requested scheme (readable, never blank).
 */
export function resolveTheme(contract: PluginThemeContractV1): ResolvedPluginTheme {
	const scheme = contract.mode === "dark" ? "dark" : "light";
	if (
		contract.version !== 1 ||
		typeof contract !== "object" ||
		!contract.tokens ||
		typeof contract.tokens !== "object"
	) {
		warnOnce(
			"version",
			`unsupported theme contract version (${String((contract as { version?: unknown }).version)}); using built-in ${scheme} fallback tokens`,
		);
		return {
			scheme,
			contrast: "normal",
			density: "comfortable",
			motion: "full",
			tokens: scheme === "dark" ? MACRO_DARK_TOKENS : MACRO_LIGHT_TOKENS,
			fallbackUsed: true,
		};
	}
	const incoming = contract.tokens as unknown as Record<string, unknown>;
	const fallback = scheme === "dark" ? MACRO_DARK_TOKENS : MACRO_LIGHT_TOKENS;
	const tokens = {} as PluginThemeTokensV1;
	let fallbackUsed = false;
	for (const key of Object.keys(TOKEN_CSS_PROPERTIES) as (keyof PluginThemeTokensV1)[]) {
		const value = incoming[key];
		if (isUnsafeColor(value)) {
			warnOnce(`token:${key}`, `invalid theme token "${key}"; using built-in ${scheme} fallback`);
			(tokens[key] as string) = fallback[key];
			fallbackUsed = true;
		} else {
			(tokens[key] as string) = value as string;
		}
	}
	return {
		scheme,
		contrast: contract.contrast === "high" ? "high" : "normal",
		density: contract.density === "compact" ? "compact" : "comfortable",
		motion: contract.motion === "reduced" ? "reduced" : "full",
		tokens,
		fallbackUsed,
	};
}
