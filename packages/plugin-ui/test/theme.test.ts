import { afterEach, describe, expect, it, vi } from "vitest";
import {
	MACRO_DARK_TOKENS,
	MACRO_LIGHT_TOKENS,
	resolveTheme,
	resetThemeWarningsForTest,
} from "../src/theme";
import type { PluginThemeContractV1 } from "../src/theme";

function contract(overrides: Partial<PluginThemeContractV1> = {}): PluginThemeContractV1 {
	return {
		version: 1,
		mode: "light",
		contrast: "normal",
		density: "comfortable",
		motion: "full",
		tokens: { ...MACRO_LIGHT_TOKENS },
		...overrides,
	};
}

afterEach(() => {
	resetThemeWarningsForTest();
	vi.restoreAllMocks();
});

describe("resolveTheme", () => {
	it("passes through a valid v1 contract", () => {
		const theme = resolveTheme(contract({ mode: "dark", density: "compact", tokens: { ...MACRO_DARK_TOKENS } }));
		expect(theme.fallbackUsed).toBe(false);
		expect(theme.scheme).toBe("dark");
		expect(theme.density).toBe("compact");
		expect(theme.tokens.page).toBe(MACRO_DARK_TOKENS.page);
	});

	it("falls back to readable tokens for an unsupported version and warns once", () => {
		const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
		const bad = contract() as unknown as { version: number };
		bad.version = 99;
		const first = resolveTheme(bad as unknown as PluginThemeContractV1);
		resolveTheme(bad as unknown as PluginThemeContractV1);
		expect(first.fallbackUsed).toBe(true);
		expect(first.tokens.ink).toBe(MACRO_LIGHT_TOKENS.ink);
		expect(warn).toHaveBeenCalledTimes(1);
	});

	it("rejects unsafe token values like url(...) per token", () => {
		const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
		const tokens = { ...MACRO_LIGHT_TOKENS, page: "url(https://evil.example/x)" };
		const theme = resolveTheme(contract({ tokens }));
		expect(theme.tokens.page).toBe(MACRO_LIGHT_TOKENS.page);
		expect(theme.tokens.surface).toBe(MACRO_LIGHT_TOKENS.surface);
		expect(theme.fallbackUsed).toBe(true);
		expect(warn).toHaveBeenCalled();
	});
});
