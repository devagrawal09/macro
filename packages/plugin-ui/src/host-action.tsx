/**
 * Dispatches a typed host intent with button styling. Never touches a router,
 * the parent window, or the host DOM; the host owns routing after dispatch.
 */
import { omit } from "solid-js";
import type { ClientHostIntent } from "./intent";
import { Button, type ButtonProps } from "./button";
import { usePluginUI } from "./internal/context";

export interface HostActionProps extends Omit<ButtonProps, "onClick"> {
	intent: ClientHostIntent;
	onIntentError?(error: unknown): void;
}

export function HostAction(props: HostActionProps) {
	const ui = usePluginUI();
	const rest = omit(props, "intent", "onIntentError");
	return (
		<Button
			{...(rest as ButtonProps)}
			onClick={() => {
				ui.dispatchIntent(props.intent).catch((error: unknown) => props.onIntentError?.(error));
			}}
		/>
	);
}
