/**
 * Catalog entry: renders the four required variants at their evidence widths.
 * Frames are fixed to 1440px (project.page) and 390px (entity.side_panel) so
 * container queries behave like the real plugin surfaces.
 */
import { insert } from "@solidjs/web";
import {
	DemoTaskInboxPage,
	DemoTaskSidePanel,
	type DemoFrameProps,
} from "./demo";
import type { PluginThemeContractV1 } from "../src/theme";
import styles from "../src/styles.css?inline";

const LIGHT: PluginThemeContractV1 = {
	version: 1,
	mode: "light",
	contrast: "normal",
	density: "comfortable",
	motion: "full",
	tokens: {
		page: "#f5f4f0", surface: "#faf9f6", surfaceRaised: "#ffffff", surfaceInset: "#efeee9",
		ink: "#22211e", inkMuted: "#69665f", inkSubtle: "#949088", inkDisabled: "#aaa69e",
		edge: "#ddd9d0", edgeMuted: "#e8e4dc", accent: "#d98e17", accentInk: "#2c1b00",
		hover: "rgba(34,33,30,.045)", active: "rgba(34,33,30,.08)", selected: "rgba(217,142,23,.11)",
		success: "#168a50", warning: "#b66b00", failure: "#c93f3f", focusRing: "#a96300",
	},
};

const DARK: PluginThemeContractV1 = {
	version: 1,
	mode: "dark",
	contrast: "normal",
	density: "compact",
	motion: "full",
	tokens: {
		page: "#171714", surface: "#20201c", surfaceRaised: "#292923", surfaceInset: "#121210",
		ink: "#f2f0e9", inkMuted: "#c4c0b6", inkSubtle: "#8f8c84", inkDisabled: "#68665f",
		edge: "#46443d", edgeMuted: "#34332d", accent: "#f3b329", accentInk: "#261800",
		hover: "rgba(255,255,255,.045)", active: "rgba(255,255,255,.085)", selected: "rgba(243,179,41,.12)",
		success: "#68cf91", warning: "#f5bd55", failure: "#ff7d77", focusRing: "#ffd36d",
	},
};

const root = document.getElementById("catalog") as HTMLElement;

function frame(
	label: string,
	width: number,
	body: (props: DemoFrameProps) => unknown,
): void {
	const wrapper = document.createElement("section");
	wrapper.setAttribute("data-catalog-frame", label);
	wrapper.style.cssText = `margin:0 auto 40px;width:${width}px;max-width:100%`;
	const heading = document.createElement("h2");
	heading.textContent = `${label} · ${width}px`;
	heading.style.cssText = "font:600 13px system-ui;color:#eee;margin:0 0 6px";
	const note = document.createElement("p");
	note.setAttribute("data-intent-note", "");
	note.textContent = "Click a host action or link; the typed intent is announced here.";
	note.style.cssText = "font:11px ui-monospace,Menlo,monospace;color:#999;margin:0 0 8px";
	const shell = document.createElement("div");
	shell.style.cssText = `width:${width}px;max-width:100%`;
	wrapper.append(heading, note, shell);
	root.append(wrapper);

	const sidePanel = label.includes("side_panel");
	const props: DemoFrameProps = {
		theme: label.includes("DARK") ? DARK : LIGHT,
		surface: sidePanel
			? { version: 1, placement: "entity.side_panel", size: "narrow", widthPx: width, heightPx: 800 }
			: { version: 1, placement: "project.page", size: width >= 720 ? "wide" : "narrow", widthPx: width, heightPx: 900 },
		onIntent: (intent) => {
			note.textContent = `last intent: ${JSON.stringify(intent)}`;
		},
		dispatchIntent: async () => {},
	};
	insert(shell, body(props));
}

const styleEl = document.createElement("style");
styleEl.textContent = styles;
document.head.append(styleEl);
document.body.style.cssText = "margin:0;background:#141414;padding:24px;font-family:system-ui";

frame("LIGHT · project.page", 1440, (props) => <DemoTaskInboxPage {...props} />);
frame("DARK · project.page", 1440, (props) => <DemoTaskInboxPage {...props} />);
frame("LIGHT · entity.side_panel", 390, (props) => <DemoTaskSidePanel {...props} />);
frame("DARK · entity.side_panel", 390, (props) => <DemoTaskSidePanel {...props} />);
