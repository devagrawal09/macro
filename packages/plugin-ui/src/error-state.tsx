/**
 * Standard recovery surface for failures: role="alert", plain description,
 * and a normal secondary retry button. Raw exception text stays in logs.
 */
import type { JSX } from "@solidjs/web";
import { Dynamic, Show } from "@solidjs/web";
import type { PluginIcon } from "./icon";
import { WarningIcon } from "./icons";
import { Button } from "./button";
import { joinClass } from "./internal/utils";

export interface ErrorStateProps {
	title?: string;
	description: JSX.Element;
	retryLabel?: string;
	onRetry?(): void | Promise<void>;
	compact?: boolean;
	class?: string;
}

export function ErrorState(props: ErrorStateProps) {
	return (
		<div
			class={joinClass("mui-error", props.class)}
			data-compact={props.compact ? "" : undefined}
			role="alert"
		>
			<span class="mui-state-icon mui-error-icon">
				<Dynamic component={WarningIcon} size={16} aria-hidden />
			</span>
			<div class="mui-state-copy">
				<Show when={props.title}>
					<strong class="mui-state-title">{props.title}</strong>
				</Show>
				<p class="mui-state-desc">{props.description}</p>
			</div>
			<Show when={props.onRetry}>
				<Button variant="secondary" size="sm" onClick={() => void props.onRetry?.()}>
					{props.retryLabel ?? "Retry"}
				</Button>
			</Show>
		</div>
	);
}
