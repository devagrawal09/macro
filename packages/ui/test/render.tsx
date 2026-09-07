import { cleanup, render as renderSolid } from "@solidjs/testing-library";
import { type BoundFunctions, type queries, within } from "@testing-library/dom";
import type { JSX } from "solid-js";

export type RenderResult = BoundFunctions<typeof queries> & {
  container: HTMLElement;
  unmount(): void;
};

export function render(factory: () => JSX.Element): RenderResult {
  const result = renderSolid(factory);
  const bound = within(result.container) as unknown as RenderResult;
  bound.container = result.container;
  bound.unmount = result.unmount;
  return bound;
}

export { cleanup };
