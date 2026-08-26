import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, cleanup } from "./utils/render";
import { mount } from "./utils/fixtures";
import { Button } from "../src/button";
import { PlusIcon } from "../src/icons";

afterEach(cleanup);

describe("Button", () => {
	it("defaults to type=button and forwards clicks", () => {
		const onClick = vi.fn();
		const { getByRole } = render(() => mount(() => (
			<Button onClick={onClick}>Save</Button>
		)));
		const button = getByRole("button", { name: "Save" });
		expect(button.getAttribute("type")).toBe("button");
		fireEvent.click(button);
		expect(onClick).toHaveBeenCalledOnce();
	});

	it("uses native disabled and suppresses activation", () => {
		const onClick = vi.fn();
		const { getByRole } = render(() => mount(() => (
			<Button disabled onClick={onClick}>Save</Button>
		)));
		const button = getByRole("button", { name: "Save" }) as HTMLButtonElement;
		expect(button.disabled).toBe(true);
		fireEvent.click(button);
		expect(onClick).not.toHaveBeenCalled();
	});

	it("loading keeps the button focusable but announces busy state and suppresses activation", () => {
		const onClick = vi.fn();
		const { getByRole } = render(() => mount(() => (
			<Button loading loadingLabel="Saving changes" onClick={onClick}>
				Save
			</Button>
		)));
		const button = getByRole("button", { name: /Save/ }) as HTMLButtonElement;
		expect(button.disabled).toBe(false);
		expect(button.getAttribute("aria-busy")).toBe("true");
		expect(button.getAttribute("aria-disabled")).toBe("true");
		expect(button.querySelector(".mui-spinner")).not.toBeNull();
		fireEvent.click(button);
		expect(onClick).not.toHaveBeenCalled();
	});

	it("renders start icons decoratively", () => {
		const { getByRole } = render(() => mount(() => (
			<Button variant="primary" startIcon={PlusIcon}>New task</Button>
		)));
		const svg = getByRole("button", { name: "New task" }).querySelector("svg");
		expect(svg?.getAttribute("aria-hidden")).toBe("true");
	});
});
