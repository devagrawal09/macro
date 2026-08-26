import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, cleanup } from "./utils/render";
import { lightTheme, makeDispatch, widePageSurface } from "./utils/fixtures";
import type { JSX } from "@solidjs/web";
import { PluginUIProvider } from "../src/provider";
import { HostAction } from "../src/host-action";
import { HostLink } from "../src/host-link";

afterEach(cleanup);

function mountWith(dispatch: ReturnType<typeof makeDispatch>, children: () => unknown) {
	return render(() => (
		<PluginUIProvider theme={lightTheme()} surface={widePageSurface()} dispatchIntent={dispatch}>
			{children as unknown as JSX.Element}
		</PluginUIProvider>
	));
}

describe("typed host intents", () => {
	it("HostAction dispatches the typed intent and never navigates", async () => {
		const sent: unknown[] = [];
		const { getByRole } = mountWith(makeDispatch(sent as never), () => (
			<HostAction variant="primary" intent={{ type: "project.open", projectId: "p1" }}>
				Open project
			</HostAction>
		));
		fireEvent.click(getByRole("button", { name: "Open project" }));
		await vi.waitFor(() => expect(sent).toEqual([{ type: "project.open", projectId: "p1" }]));
	});

	it("HostLink renders a button (bridge command) and reports dispatch errors", async () => {
		const onIntentError = vi.fn();
		const failing = async () => {
			throw new Error("bridge down");
		};
		const { getByRole } = render(() => (
			<PluginUIProvider
				theme={lightTheme()}
				surface={widePageSurface()}
				dispatchIntent={failing as never}
			>
				<HostLink
					intent={{ type: "entity.open", entity: { type: "task", id: "t1" } }}
					onIntentError={onIntentError}
				>
					Prepare launch checklist
				</HostLink>
			</PluginUIProvider>
		));
		const link = getByRole("button", { name: "Prepare launch checklist" });
		expect(link.tagName).toBe("BUTTON");
		fireEvent.click(link);
		await vi.waitFor(() => expect(onIntentError).toHaveBeenCalledOnce());
	});

	it("kit source never touches window.top/parent/open or the router", async () => {
		const fs = await import("node:fs");
		const path = await import("node:path");
		const srcDir = path.resolve(import.meta.dirname ?? ".", "../src");
		const files = fs.readdirSync(srcDir, { recursive: true }) as unknown as string[];
		for (const file of files) {
			if (!String(file).match(/\.tsx?$/)) continue;
			const text = fs.readFileSync(path.join(srcDir, String(file)), "utf8");
			expect(text.includes("window.top"), file).toBe(false);
			expect(text.includes("window.parent"), file).toBe(false);
			expect(text.includes("window.open"), file).toBe(false);
			expect(text.includes("@solidjs/router"), file).toBe(false);
		}
	});
});
