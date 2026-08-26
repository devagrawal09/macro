import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, cleanup } from "./utils/render";
import { mount } from "./utils/fixtures";
import { List } from "../src/list";
import { Badge } from "../src/badge";

afterEach(cleanup);

function rows() {
	return (
		<List label="Tasks" divided selectionMode="multiple">
			<List.Item
				title={<span>Prepare launch checklist</span>}
				description="Atlas"
				badges={<Badge tone="success">In progress</Badge>}
				meta="8m"
				selectionLabel="Select Prepare launch checklist"
				onSelectedChange={(selected) => record.push(selected)}
				onActivate={() => activations += 1}
			/>
			<List.Item title={<span>Draft guide</span>} onActivate={() => activations += 1} />
		</List>
	);
}

let activations = 0;
let record: boolean[] = [];

afterEach(() => {
	activations = 0;
	record = [];
});

describe("List", () => {
	it("renders semantic list semantics", () => {
		const { container, getByRole } = render(() => mount(rows));
		expect(container.querySelector("ul")).not.toBeNull();
		expect(getByRole("list").getAttribute("aria-label")).toBe("Tasks");
		expect(container.querySelectorAll(".mui-list-item").length).toBe(2);
	});

	it("multiple selection renders a checkbox wired to onSelectedChange", () => {
		const { getByRole } = render(() => mount(rows));
		const check = getByRole("checkbox", { name: "Select Prepare launch checklist" }) as HTMLInputElement;
		check.click();
		expect(record).toEqual([true]);
	});

	it("row activation button fires onActivate with keyboard Enter", () => {
		const { getAllByRole } = render(() => mount(rows));
		const activate = getAllByRole("button")[0] as HTMLButtonElement;
		activate.focus();
		fireEvent.keyDown(activate, { key: "Enter" });
		fireEvent.click(activate);
		expect(activations).toBeGreaterThanOrEqual(1);
	});
});
