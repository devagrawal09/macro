/** Flex layout primitive for rows, columns, gaps, and alignment. */
import type { ParentProps } from "solid-js";
import type { JSX } from "@solidjs/web";
import { joinClass } from "./internal/utils";

export interface StackProps extends ParentProps<{
	direction?: "row" | "column";
	gap?: 0 | 1 | 2 | 3 | 4 | 6 | 8;
	align?: "start" | "center" | "end" | "stretch" | "baseline";
	justify?: "start" | "center" | "end" | "between";
	wrap?: boolean;
	class?: string;
	style?: JSX.CSSProperties;
	role?: JSX.HTMLAttributes<HTMLDivElement>["role"];
}> {}

export function Stack(props: StackProps) {
	return (
		<div
			class={joinClass("mui-stack", props.class)}
			data-direction={props.direction ?? "column"}
			data-gap={props.gap ?? 0}
			data-align={props.align}
			data-justify={props.justify}
			data-wrap={props.wrap ? "" : undefined}
			role={props.role}
			style={props.style}
		>
			{props.children}
		</div>
	);
}
