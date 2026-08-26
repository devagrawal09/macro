/** One polymorphic typography primitive covering the kit text hierarchy. */
import type { ParentProps } from "solid-js";
import { Dynamic } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { joinClass } from "./internal/utils";

export type TextRole = "display" | "title" | "heading" | "body" | "caption" | "mono";
export type TextTone = "default" | "muted" | "subtle" | "accent" | "success" | "warning" | "failure";
export type TextAs = "span" | "p" | "div" | "label" | "h1" | "h2" | "h3";

export interface TextProps extends ParentProps<{
	as?: TextAs;
	role?: TextRole;
	tone?: TextTone;
	weight?: "regular" | "medium" | "semibold";
	/** `true` clamps to one line; a number clamps to that many lines. */
	truncate?: boolean | 2 | 3;
	class?: string;
	style?: JSX.CSSProperties;
	id?: string;
}> {}

function truncateStyle(truncate: TextProps["truncate"]): JSX.CSSProperties {
	if (truncate === true) {
		return { overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" };
	}
	if (typeof truncate === "number") {
		return {
			display: "-webkit-box",
			"-webkit-box-orient": "vertical",
			"-webkit-line-clamp": String(truncate),
			overflow: "hidden",
		};
	}
	return {};
}

export function Text(props: TextProps) {
	return (
		<Dynamic
			component={props.as ?? "span"}
			class={joinClass("mui-text", props.class)}
			data-role={props.role ?? "body"}
			data-tone={props.tone}
			data-weight={props.weight}
			id={props.id}
			style={truncateStyle(props.truncate)}
		>
			{props.children}
		</Dynamic>
	);
}
