/**
 * Renders the four required catalog variants (project.page / entity.side_panel
 * x light / dark at 1440px / 390px) and writes DOM snapshots with the scoped
 * stylesheet inlined as committed evidence under catalog/evidence/.
 */
import { afterEach, describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { render, cleanup } from "./utils/render";
import { DemoTaskInboxPage, DemoTaskSidePanel } from "../catalog/demo";
import styles from "../src/styles.css?inline";
import type { PluginThemeContractV1 } from "../src/theme";

const LIGHT: PluginThemeContractV1 = {
	version: 1, mode: "light", contrast: "normal", density: "comfortable", motion: "full",
	tokens: {
		page: "#f5f4f0", surface: "#faf9f6", surfaceRaised: "#ffffff", surfaceInset: "#efeee9",
		ink: "#22211e", inkMuted: "#69665f", inkSubtle: "#949088", inkDisabled: "#aaa69e",
		edge: "#ddd9d0", edgeMuted: "#e8e4dc", accent: "#d98e17", accentInk: "#2c1b00",
		hover: "rgba(34,33,30,.045)", active: "rgba(34,33,30,.08)", selected: "rgba(217,142,23,.11)",
		success: "#168a50", warning: "#b66b00", failure: "#c93f3f", focusRing: "#a96300",
	},
};

const DARK: PluginThemeContractV1 = {
	version: 1, mode: "dark", contrast: "normal", density: "compact", motion: "full",
	tokens: {
		page: "#171714", surface: "#20201c", surfaceRaised: "#292923", surfaceInset: "#121210",
		ink: "#f2f0e9", inkMuted: "#c4c0b6", inkSubtle: "#8f8c84", inkDisabled: "#68665f",
		edge: "#46443d", edgeMuted: "#34332d", accent: "#f3b329", accentInk: "#261800",
		hover: "rgba(255,255,255,.045)", active: "rgba(255,255,255,.085)", selected: "rgba(243,179,41,.12)",
		success: "#68cf91", warning: "#f5bd55", failure: "#ff7d77", focusRing: "#ffd36d",
	},
};

const VARIANTS = [
	{ name: "project-page-light-1440", theme: LIGHT, placement: "project.page", size: "wide", width: 1440 },
	{ name: "project-page-dark-1440", theme: DARK, placement: "project.page", size: "wide", width: 1440 },
	{ name: "side-panel-light-390", theme: LIGHT, placement: "entity.side_panel", size: "narrow", width: 390 },
	{ name: "side-panel-dark-390", theme: DARK, placement: "entity.side_panel", size: "narrow", width: 390 },
] as const;

describe("catalog evidence", () => {
	it("renders all four variants with real content", () => {
		for (const variant of VARIANTS) {
			const props = {
				theme: variant.theme,
				surface: {
					version: 1 as const,
					placement: variant.placement,
					size: variant.size,
					widthPx: variant.width,
					heightPx: variant.placement === "entity.side_panel" ? 800 : 900,
				},
				dispatchIntent: async () => {},
			};
			const view = render(() =>
				variant.placement === "entity.side_panel"
					? <DemoTaskSidePanel {...props} />
					: <DemoTaskInboxPage {...props} />,
			);
			const root = view.container.querySelector("[data-macro-plugin-ui]") as HTMLElement;
			expect(root.getAttribute("data-macro-scheme")).toBe(variant.theme.mode);
			expect(root.textContent?.toLowerCase()).toContain("task");
			view.unmount();
		}
	});

	it("writes DOM evidence snapshots", () => {
		const outDir = path.resolve(import.meta.dirname ?? ".", "../catalog/evidence");
		fs.mkdirSync(outDir, { recursive: true });
		for (const variant of VARIANTS) {
			const props = {
				theme: variant.theme,
				surface: {
					version: 1 as const,
					placement: variant.placement,
					size: variant.size,
					widthPx: variant.width,
					heightPx: variant.placement === "entity.side_panel" ? 800 : 900,
				},
				dispatchIntent: async () => {},
			};
			const view = render(() =>
				variant.placement === "entity.side_panel"
					? <DemoTaskSidePanel {...props} />
					: <DemoTaskInboxPage {...props} />,
			);
			const doc = [
				"<!doctype html>",
				'<html lang="en"><head><meta charset="utf-8">',
				`<title>${variant.name}</title>`,
				"<style>body{margin:0;padding:16px;background:#141414}",
				`.frame{width:${variant.width}px}</style>`,
				`<style>${styles}</style>`,
				"</head><body>",
				`<div class="frame">${view.container.innerHTML}</div>`,
				"</body></html>",
			].join("\n");
			fs.writeFileSync(path.join(outDir, `${variant.name}.html`), doc);
			view.unmount();
		}
		expect(fs.readdirSync(outDir).sort()).toContain("side-panel-dark-390.html");
	});
});

afterEach(cleanup);
