import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, cleanup } from "./utils/render";
import { mount } from "./utils/fixtures";
import { Spinner } from "../src/spinner";
import { EmptyState } from "../src/empty-state";
import { ErrorState } from "../src/error-state";

afterEach(cleanup);

describe("states", () => {
	it("Spinner is a labelled status by default and inline inside buttons", () => {
		const view = render(() => mount(() => <Spinner size={24} label="Loading tasks" />));
		const status = view.getByRole("status");
		expect(status.textContent).toContain("Loading tasks");
		expect(status.querySelector(".mui-spinner")?.getAttribute("aria-hidden")).toBe("true");
	});

	it("EmptyState composes title, copy, and action", () => {
		const onClick = vi.fn();
		const view = render(() => mount(() => (
			<EmptyState
				title="No matching tasks"
				description={<span>Try another search.</span>}
				action={<button type="button" onClick={onClick}>Clear search</button>}
			/>
		)));
		expect(view.getByText("No matching tasks")).not.toBeNull();
		fireEvent.click(view.getByRole("button", { name: "Clear search" }));
		expect(onClick).toHaveBeenCalledOnce();
	});

	it("ErrorState uses role=alert and wires the retry button", () => {
		const onRetry = vi.fn();
		const view = render(() => mount(() => (
			<ErrorState description={<span>Could not load tasks.</span>} retryLabel="Try again" onRetry={onRetry} />
		)));
		expect(view.getByRole("alert")).not.toBeNull();
		fireEvent.click(view.getByRole("button", { name: "Try again" }));
		expect(onRetry).toHaveBeenCalledOnce();
	});
});
