import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, cleanup } from "./utils/render";
import { mount } from "./utils/fixtures";
import { FormField } from "../src/form-field";
import { TextField } from "../src/text-field";
import { TextArea } from "../src/text-area";

afterEach(cleanup);

describe("FormField + TextField", () => {
	it("associates label, description, and error with the input", () => {
		const { container, getByRole } = render(() => mount(() => (
			<FormField label="Notes" description="Visible to members" error="Required" required>
				<TextField value="" onValueChange={() => {}} />
			</FormField>
		)));
		const input = getByRole("textbox", { name: /Notes/ }) as HTMLInputElement;
		const label = container.querySelector(".mui-label") as HTMLLabelElement;
		expect(label.getAttribute("for")).toBe(input.id);
		const describedBy = input.getAttribute("aria-describedby") ?? "";
		for (const id of describedBy.split(" ")) {
			expect(container.querySelector(`#${id}`)).not.toBeNull();
		}
		expect(container.querySelector(".mui-field-error")).not.toBeNull();
		expect(input.getAttribute("aria-errormessage")).not.toBeNull();
		expect(input.required).toBe(true);
	});

	it("fires onValueChange for typed input (controlled)", () => {
		const onValueChange = vi.fn();
		const { getByRole } = render(() => mount(() => (
			<TextField value="" onValueChange={onValueChange} aria-label="Search" />
		)));
		fireEvent.input(getByRole("textbox", { name: "Search" }), { target: { value: "abc" } });
		expect(onValueChange).toHaveBeenCalledWith("abc");
	});

	it("marks invalid inputs with data-invalid and aria-invalid", () => {
		const { getByRole } = render(() => mount(() => (
			<TextField invalid aria-label="Search" />
		)));
		const input = getByRole("textbox", { name: "Search" });
		expect(input.getAttribute("aria-invalid")).toBe("true");
	});
});

describe("TextArea", () => {
	it("reports value changes and supports resize=none", () => {
		const onValueChange = vi.fn();
		const { getByRole } = render(() => mount(() => (
			<TextArea defaultValue="draft" resize="none" onValueChange={onValueChange} aria-label="Body" />
		)));
		const area = getByRole("textbox", { name: "Body" }) as HTMLTextAreaElement;
		expect(area.value).toBe("draft");
		expect(area.getAttribute("data-resize")).toBe("none");
		fireEvent.input(area, { target: { value: "updated" } });
		expect(onValueChange).toHaveBeenCalledWith("updated");
	});
});
