import { describe, expect, it } from "vitest";
import * as kit from "../src/index";

describe("public export surface", () => {
	it("exports exactly the 16 v0 components", () => {
		const runtimeExports = Object.keys(kit).sort();
		expect(runtimeExports).toEqual(
			[
			"Badge", "Button", "EmptyState", "ErrorState", "FormField", "HostAction",
			"HostLink", "IconButton", "List", "MACRO_DARK_TOKENS", "MACRO_LIGHT_TOKENS",
			"PluginUIProvider", "Spinner", "Stack", "Surface", "TextArea", "TextField",
			"Text", "TOKEN_CSS_PROPERTIES", "resolveTheme",
			].sort(),
		);
	});

	it("exposes List.Item as a compound member without extra top-level components", () => {
		expect(typeof (kit.List as { Item: unknown }).Item).toBe("function");
	});
});
