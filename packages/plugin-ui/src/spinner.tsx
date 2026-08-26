/** Quiet progress indicator; reduced motion renders a static partial ring. */
import { Show } from "solid-js";
import { joinClass } from "./internal/utils";

export interface SpinnerProps {
	size?: 12 | 16 | 20 | 24 | 32;
	label?: string;
	/** Renders inline without the status wrapper (e.g. inside a button). */
	inline?: boolean;
	class?: string;
}

export function Spinner(props: SpinnerProps) {
	const ring = (
		<span class={joinClass("mui-spinner", props.class)} data-size={props.size ?? 16} aria-hidden="true" />
	);
	return (
		<Show
			when={props.inline}
			fallback={
				<span class="mui-spinner-status" role="status">
					{ring}
					<span class="mui-visually-hidden">{props.label ?? "Loading"}</span>
				</span>
			}
		>
			{ring}
		</Show>
	);
}
