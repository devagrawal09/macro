import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, cleanup } from "./utils/render";
import { mount } from "./utils/fixtures";
import { IconButton } from "../src/icon-button";
import { MoreIcon } from "../src/icons";

afterEach(cleanup);

describe("IconButton", () => {
	it("requires a label that becomes aria-label; the icon is hidden", () => {
		const { getByRole } = render(() => mount(() => (
			<IconButton icon={MoreIcon} label="Task actions" />
		)));
		const button = getByRole("button", { name: "Task actions" });
		expect(button.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");
	});

	it("suppresses activation while loading", () => {
		const onClick = vi.fn();
		const { getByRole } = render(() => mount(() => (
			<IconButton icon={MoreIcon} label="Task actions" loading onClick={onClick} />
		)));
		fireEvent.click(getByRole("button", { name: "Task actions" }));
		expect(onClick).not.toHaveBeenCalled();
	});
});
