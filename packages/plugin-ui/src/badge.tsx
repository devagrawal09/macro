/** Informational pill for status, priority, counts, and metadata. Never a button. */
import type { ParentProps } from "solid-js";
import { Dynamic, Show } from "@solidjs/web";
import type { PluginIcon } from "./icon";
import { joinClass } from "./internal/utils";

export interface BadgeProps extends ParentProps<{
	tone?: "neutral" | "accent" | "success" | "warning" | "failure";
	size?: "sm" | "md";
	shape?: "pill" | "rounded";
	icon?: PluginIcon;
	class?: string;
}> {}

export function Badge(props: BadgeProps) {
	return (
		<span
			class={joinClass("mui-badge", props.class)}
			data-tone={props.tone ?? "neutral"}
			data-size={props.size ?? "md"}
			data-shape={props.shape ?? "pill"}
		>
			<Show when={props.icon}>
				{(icon) => <Dynamic component={icon()} size={12} aria-hidden />}
			</Show>
			{props.children}
		</span>
	);
}
