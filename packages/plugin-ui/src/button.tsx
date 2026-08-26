/** Primary interaction control with safe loading and disabled behavior. */
import type { ParentProps, Ref } from "solid-js";
import { Dynamic, Show } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { omit } from "solid-js";
import type { PluginIcon } from "./icon";
import { Spinner } from "./spinner";
import { joinClass, setRef } from "./internal/utils";

export type ButtonVariant = "ghost" | "secondary" | "primary" | "danger";
export type ControlSize = "sm" | "md" | "lg";

type NativeButtonProps = Omit<
	JSX.HTMLAttributes<HTMLButtonElement>,
	"class" | "children" | "disabled"
>;

export interface ButtonProps extends ParentProps<NativeButtonProps & {
	type?: "button" | "submit" | "reset";
	variant?: ButtonVariant;
	size?: ControlSize;
	disabled?: boolean;
	loading?: boolean;
	loadingLabel?: string;
	fullWidth?: boolean;
	startIcon?: PluginIcon;
	endIcon?: PluginIcon;
	class?: string;
	ref?: Ref<HTMLButtonElement>;
}> {}

export function Button(props: ButtonProps) {
	const rest = omit(props, "type", "variant", "size", "disabled", "loading", "loadingLabel", "fullWidth", "startIcon", "endIcon", "class", "ref", "children");
	const inactive = () => props.disabled === true || props.loading === true;
	return (
		<button
			{...(rest as JSX.HTMLAttributes<HTMLButtonElement>)}
			ref={(el) => setRef(props.ref, el)}
			type={props.type ?? "button"}
			class={joinClass("mui-button", props.class)}
			data-variant={props.variant ?? "secondary"}
			data-size={props.size ?? "md"}
			data-fullwidth={props.fullWidth ? "" : undefined}
			disabled={props.disabled}
			aria-busy={props.loading ? "true" : undefined}
			aria-disabled={!props.disabled && props.loading ? "true" : undefined}
			onClick={(event: MouseEvent) => {
				if (inactive()) return;
				(rest.onClick as ((e: MouseEvent) => void) | undefined)?.(event);
			}}
		>
			<Show when={props.loading}>
				<Spinner size={16} inline />
			</Show>
			<Show when={!props.loading && props.startIcon}>
				{(icon) => <Dynamic component={icon()} size={16} aria-hidden />}
			</Show>
			{props.children}
			<Show when={!props.loading && props.endIcon}>
				{(icon) => <Dynamic component={icon()} size={16} aria-hidden />}
			</Show>
			<Show when={props.loading && props.loadingLabel}>
				<span class="mui-visually-hidden">{props.loadingLabel}</span>
			</Show>
		</button>
	);
}
