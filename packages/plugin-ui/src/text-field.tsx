/** Common single-line text/search input with controlled and uncontrolled use. */
import type { Ref } from "solid-js";
import { Dynamic, Show } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { createRenderEffect, omit } from "solid-js";
import type { PluginIcon } from "./icon";
import type { ControlSize } from "./button";
import { useOptionalFieldAssociation } from "./internal/context";
import { joinClass, setRef } from "./internal/utils";

type NativeInputProps = Omit<
	JSX.HTMLAttributes<HTMLInputElement>,
	"class" | "children" | "value" | "defaultValue" | "disabled" | "required"
>;

export interface TextFieldProps extends NativeInputProps {
	type?: string;
	value?: string;
	defaultValue?: string;
	onValueChange?(value: string): void;
	size?: ControlSize;
	placeholder?: string;
	invalid?: boolean;
	disabled?: boolean;
	required?: boolean;
	leadingIcon?: PluginIcon;
	trailing?: JSX.Element;
	class?: string;
	ref?: Ref<HTMLInputElement>;
}

export function TextField(props: TextFieldProps) {
	const rest = omit(props, "type", "value", "defaultValue", "onValueChange", "size", "invalid", "disabled", "required", "leadingIcon", "trailing", "class", "ref");
	const field = useOptionalFieldAssociation();
	let input!: HTMLInputElement;

	// Controlled updates flow from props into the DOM; uncontrolled inputs
	// simply never pass `value`, so the effect stays idle.
	createRenderEffect(
		() => props.value,
		(value) => {
			if (input && value !== undefined && input.value !== value) input.value = value;
		},
	);

	return (
		<span class="mui-input-wrap">
			<Show when={props.leadingIcon}>
				{(icon) => <Dynamic component={icon()} size={16} aria-hidden />}
			</Show>
			<input
				{...(rest as JSX.HTMLAttributes<HTMLInputElement>)}
				ref={(el) => {
					input = el;
					setRef(props.ref, el);
				}}
				id={field?.controlId() ?? props.id}
				type={props.type ?? "text"}
				class={joinClass("mui-input", props.class)}
				data-size={props.size}
				data-invalid={props.invalid || field?.fieldInvalid() ? "" : undefined}
				defaultValue={props.defaultValue}
				disabled={props.disabled || field?.fieldDisabled()}
				required={props.required || field?.fieldRequired()}
				aria-invalid={props.invalid || field?.fieldInvalid() ? "true" : undefined}
				aria-describedby={field?.describedBy()}
				aria-errormessage={field?.errorId()}
				onInput={(event: Event) => {
					props.onValueChange?.((event.target as HTMLInputElement).value);
					(rest.onInput as ((e: Event) => void) | undefined)?.(event);
				}}
			/>
			<Show when={props.trailing}>
				<span class="mui-input-trailing">{props.trailing}</span>
			</Show>
		</span>
	);
}
