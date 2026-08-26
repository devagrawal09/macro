/**
 * Deep region primitive: `plain` regions, rounded `card` surfaces, and
 * full-height `panel` surfaces. `interactive` changes visuals only; authors
 * still supply a real nested button or link.
 */
import type { ParentProps } from "solid-js";
import { Dynamic } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { joinClass } from "./internal/utils";

export interface SurfaceProps extends ParentProps<{
	as?: "div" | "section" | "article" | "main" | "aside";
	variant?: "plain" | "card" | "panel";
	elevation?: 0 | 1 | 2;
	padding?: 0 | 1 | 2 | 3 | 4 | 6;
	border?: boolean;
	interactive?: boolean;
	selected?: boolean;
	class?: string;
	style?: JSX.CSSProperties;
	role?: JSX.HTMLAttributes<HTMLElement>["role"];
	"aria-label"?: string;
}> {}

export function Surface(props: SurfaceProps) {
	return (
		<Dynamic
			component={props.as ?? "div"}
			class={joinClass("mui-surface", props.class)}
			data-variant={props.variant ?? "plain"}
			data-elevation={props.elevation ?? 0}
			data-padding={props.padding}
			data-border={props.border ? "" : undefined}
			data-interactive={props.interactive ? "" : undefined}
			data-selected={props.selected ? "" : undefined}
			role={props.role}
			aria-label={props["aria-label"]}
			style={props.style}
		>
			{props.children}
		</Dynamic>
	);
}
