/** Accessible icon-only control: the label is required and becomes aria-label. */
import type { Ref } from "solid-js";
import { Dynamic, Show } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { omit } from "solid-js";
import type { PluginIcon } from "./icon";
import { Spinner } from "./spinner";
import type { ButtonVariant, ControlSize } from "./button";
import { joinClass, setRef } from "./internal/utils";

type NativeButtonProps = Omit<
	JSX.HTMLAttributes<HTMLButtonElement>,
	"class" | "children" | "disabled" | "aria-label"
>;

export interface IconButtonProps extends NativeButtonProps {
	type?: "button" | "submit" | "reset";
	icon: PluginIcon;
	label: string;
	variant?: ButtonVariant;
	size?: ControlSize;
	disabled?: boolean;
	loading?: boolean;
	class?: string;
	ref?: Ref<HTMLButtonElement>;
}

export function IconButton(props: IconButtonProps) {
	const rest = omit(props, "type", "icon", "label", "variant", "size", "disabled", "loading", "class", "ref");
	const inactive = () => props.disabled === true || props.loading === true;
	return (
		<button
			{...(rest as JSX.HTMLAttributes<HTMLButtonElement>)}
			ref={(el) => setRef(props.ref, el)}
			type={props.type ?? "button"}
			class={joinClass("mui-button", "mui-icon-button", props.class)}
			data-variant={props.variant ?? "secondary"}
			data-size={props.size ?? "md"}
			aria-label={props.label}
			aria-busy={props.loading ? "true" : undefined}
			aria-disabled={!props.disabled && props.loading ? "true" : undefined}
			disabled={props.disabled}
			onClick={(event: MouseEvent) => {
				if (inactive()) return;
				(rest.onClick as ((e: MouseEvent) => void) | undefined)?.(event);
			}}
		>
			<Show when={props.loading} fallback={<Dynamic component={props.icon} size={16} aria-hidden />}>
				<Spinner size={16} inline />
			</Show>
		</button>
	);
}
