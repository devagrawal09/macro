import { afterEach, describe, expect, it, vi } from "vitest";
import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
import { render, cleanup } from "./utils/render";
import { lightTheme, widePageSurface } from "./utils/fixtures";
import { PluginUIProvider } from "../src/provider";
import { Text } from "../src/text";
import { MACRO_DARK_TOKENS } from "../src/theme";

afterEach(cleanup);

describe("PluginUIProvider", () => {
	it("renders the scoped root with contract data attributes and token variables", () => {
		const { container } = render(() => (
			<PluginUIProvider theme={lightTheme()} surface={widePageSurface()} dispatchIntent={async () => {}}>
				<Text>hi</Text>
			</PluginUIProvider>
		));
		const root = container.querySelector("[data-macro-plugin-ui]") as HTMLElement;
		expect(root).not.toBeNull();
		expect(root.getAttribute("data-macro-theme-v")).toBe("1");
		expect(root.getAttribute("data-macro-scheme")).toBe("light");
		expect(root.getAttribute("data-macro-placement")).toBe("project.page");
		expect(root.getAttribute("data-macro-size")).toBe("wide");
		expect(root.style.getPropertyValue("--macro-color-page")).toBe("#ffffff");
	});

	it("updates reactively when the host sends a new theme without remounting children", async () => {
		const [theme, setTheme] = createSignal(lightTheme());
		const probe = (() => <Text id="probe">x</Text>) as unknown as JSX.Element;
		const view = render(() => (
			<PluginUIProvider theme={theme()} surface={widePageSurface()} dispatchIntent={async () => {}}>
				{probe}
			</PluginUIProvider>
		));
		const before = view.container.querySelector("#probe");
		setTheme({
			...lightTheme(),
			mode: "dark",
			tokens: { ...MACRO_DARK_TOKENS },
		});
		const after = view.container.querySelector("#probe");
		expect(after).toBe(before);
		const root = view.container.querySelector("[data-macro-plugin-ui]") as HTMLElement;
		await vi.waitFor(() => expect(root.getAttribute("data-macro-scheme")).toBe("dark"));
		expect(root.style.getPropertyValue("--macro-color-page")).toBe(MACRO_DARK_TOKENS.page);
	});
});
