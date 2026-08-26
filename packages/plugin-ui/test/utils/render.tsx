/**
 * Minimal test renderer for the kit: mounts a factory inside a Solid root,
 * appends it to jsdom's body, and returns Testing Library queries.
 */
import { createRoot } from "solid-js";
import { insert, registerDelegatedRoot, unregisterDelegatedRoot } from "@solidjs/web";
import { within, queries, type BoundFunctions } from "@testing-library/dom";

const disposers: Array<() => void> = [];
const containers: Array<HTMLElement> = [];

type Queries = typeof queries;

export type RenderResult = BoundFunctions<Queries> & {
	container: HTMLElement;
	unmount(): void;
};

export function render(factory: () => unknown): RenderResult {
	const container = document.createElement("div");
	document.body.appendChild(container);
	registerDelegatedRoot(container);
	containers.push(container);
	const dispose = createRoot((dispose) => {
		insert(container, factory());
		return dispose;
	});
	disposers.push(dispose);
	const bound = within(container) as unknown as RenderResult;
	bound.container = container;
	bound.unmount = () => {
		dispose();
		unregisterDelegatedRoot(container);
		container.remove();
	};
	return bound;
}

/** afterEach helper disposing every mounted root. */
export function cleanup(): void {
	for (const dispose of disposers.splice(0)) dispose();
	for (const el of containers.splice(0)) {
		unregisterDelegatedRoot(el);
		el.remove();
	}
}

export { fireEvent } from "@testing-library/dom";
