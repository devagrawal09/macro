/** Calm composition for empty data: title, optional copy, one clear action. */
import type { JSX } from "@solidjs/web";
import { Dynamic, Show } from "@solidjs/web";
import type { PluginIcon } from "./icon";
import { InboxIcon } from "./icons";
import { joinClass } from "./internal/utils";

export interface EmptyStateProps {
	title: string;
	description?: JSX.Element;
	icon?: PluginIcon;
	action?: JSX.Element;
	compact?: boolean;
	class?: string;
}

export function EmptyState(props: EmptyStateProps) {
	return (
		<div class={joinClass("mui-empty", props.class)} data-compact={props.compact ? "" : undefined}>
			<span class="mui-state-icon">
				<Dynamic component={props.icon ?? InboxIcon} size={16} aria-hidden />
			</span>
			<div class="mui-state-copy">
				<strong class="mui-state-title">{props.title}</strong>
				<Show when={props.description}>
					<p class="mui-state-desc">{props.description}</p>
				</Show>
			</div>
			<Show when={props.action}>
				<div class="mui-state-action">{props.action}</div>
			</Show>
		</div>
	);
}
