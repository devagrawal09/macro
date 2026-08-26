import type { JSX } from "@solidjs/web";
/** Shared provider fixture used across component tests. */
import type { PluginSurfaceContextV1, PluginThemeContractV1 } from "../../src/theme";
import type { ClientHostIntent, DispatchIntent } from "../../src/intent";
import { PluginUIProvider } from "../../src/provider";

export function lightTheme(): PluginThemeContractV1 {
	return {
		version: 1,
		mode: "light",
		contrast: "normal",
		density: "comfortable",
		motion: "full",
		tokens: {
			page: "#ffffff", surface: "#fafafa", surfaceRaised: "#ffffff", surfaceInset: "#f4f4f4",
			ink: "#111111", inkMuted: "#555555", inkSubtle: "#888888", inkDisabled: "#bbbbbb",
			edge: "#dddddd", edgeMuted: "#eeeeee", accent: "#d98e17", accentInk: "#2c1b00",
			hover: "rgba(0,0,0,.05)", active: "rgba(0,0,0,.08)", selected: "rgba(217,142,23,.11)",
			success: "#168a50", warning: "#b66b00", failure: "#c93f3f", focusRing: "#a96300",
		},
	};
}

export function widePageSurface(): PluginSurfaceContextV1 {
	return { version: 1, placement: "project.page", size: "wide", widthPx: 1440, heightPx: 900 };
}

export function narrowSidePanelSurface(): PluginSurfaceContextV1 {
	return { version: 1, placement: "entity.side_panel", size: "narrow", widthPx: 390, heightPx: 800 };
}

export function makeDispatch(recording: ClientHostIntent[]): DispatchIntent {
	return async (intent) => {
		recording.push(intent);
	};
}

/** Standard provider wrapper used by component tests (light theme, wide page). */
export function mount(factory: () => JSX.Element): () => JSX.Element {
	return () => (
		<PluginUIProvider theme={lightTheme()} surface={widePageSurface()} dispatchIntent={async () => {}}>
			{factory()}
		</PluginUIProvider>
	);
}
