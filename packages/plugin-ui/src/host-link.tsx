/**
 * Inline link presentation backed by a typed host intent. Renders a button
 * because the operation is a bridge command, not same-document navigation.
 */
import type { ParentProps } from "solid-js";
import type { ClientHostIntent } from "./intent";
import { usePluginUI } from "./internal/context";
import { joinClass } from "./internal/utils";

export interface HostLinkProps extends ParentProps<{
	intent: ClientHostIntent;
	tone?: "default" | "muted";
	disabled?: boolean;
	onIntentError?(error: unknown): void;
	class?: string;
	"aria-label"?: string;
}> {}

export function HostLink(props: HostLinkProps) {
	const ui = usePluginUI();
	return (
		<button
			type="button"
			class={joinClass("mui-host-link", props.class)}
			data-tone={props.tone}
			disabled={props.disabled}
			aria-label={props["aria-label"]}
			onClick={() => {
				if (props.disabled) return;
				ui.dispatchIntent(props.intent).catch((error: unknown) => props.onIntentError?.(error));
			}}
		>
			{props.children}
		</button>
	);
}
