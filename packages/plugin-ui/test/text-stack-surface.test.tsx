import { afterEach, describe, expect, it } from "vitest";
import { render, cleanup } from "./utils/render";
import { mount } from "./utils/fixtures";
import { Text } from "../src/text";
import { Stack } from "../src/stack";
import { Surface } from "../src/surface";
import { Badge } from "../src/badge";

afterEach(cleanup);

describe("Text", () => {
	it("supports polymorphism, roles, tones, weights, and truncation", () => {
		const view = render(() => mount(() => (
			<Text as="h2" role="heading" tone="muted" weight="semibold" truncate>
				Title
			</Text>
		)));
		const el = view.getByRole("heading");
		expect(el.getAttribute("data-role")).toBe("heading");
		expect(el.getAttribute("data-tone")).toBe("muted");
		expect(el.style.whiteSpace).toBe("nowrap");
	});
});

describe("Stack", () => {
	it("emits data attributes for layout state", () => {
		const view = render(() => mount(() => (
			<Stack direction="row" gap={3} align="center" justify="between" wrap>
				<span>a</span>
			</Stack>
		)));
		const el = view.container.querySelector(".mui-stack") as HTMLElement;
		expect(el.getAttribute("data-direction")).toBe("row");
		expect(el.getAttribute("data-gap")).toBe("3");
		expect(el.hasAttribute("data-wrap")).toBe(true);
	});
});

describe("Surface", () => {
	it("variants, elevation, padding, and selection are data-driven", () => {
		const view = render(() => mount(() => (
			<Surface as="section" aria-label="Card" variant="card" elevation={2} padding={4} selected>
				<span>x</span>
			</Surface>
		)));
		const el = view.container.querySelector(".mui-surface") as HTMLElement;
		expect(el.tagName).toBe("SECTION");
		expect(el.getAttribute("aria-label")).toBe("Card");
		expect(el.getAttribute("data-variant")).toBe("card");
		expect(el.getAttribute("data-elevation")).toBe("2");
		expect(el.hasAttribute("data-selected")).toBe(true);
	});
});

describe("Badge", () => {
	it("renders informational pills with tones", () => {
		const view = render(() => mount(() => <Badge tone="warning">High priority</Badge>));
		const badge = view.container.querySelector(".mui-badge") as HTMLElement;
		expect(badge.textContent).toContain("High priority");
		expect(badge.getAttribute("data-tone")).toBe("warning");
		expect(badge.tagName).toBe("SPAN");
	});
});
