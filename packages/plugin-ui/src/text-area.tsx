/** Common multi-line notes/details input. */
import type { Ref } from "solid-js";
import type { JSX } from "@solidjs/web";
import { omit, createRenderEffect } from "solid-js";
import { useOptionalFieldAssociation } from "./internal/context";
import { joinClass, setRef } from "./internal/utils";

type NativeTextAreaProps = Omit<
	JSX.HTMLAttributes<HTMLTextAreaElement>,
	"class" | "children" | "value" | "defaultValue" | "disabled" | "required"
>;

export interface TextAreaProps extends NativeTextAreaProps {
	value?: string;
	defaultValue?: string;
	onValueChange?(value: string): void;
	placeholder?: string;
	rows?: number | string;
	resize?: "none" | "vertical";
	invalid?: boolean;
	disabled?: boolean;
	required?: boolean;
	class?: string;
	ref?: Ref<HTMLTextAreaElement>;
}

export function TextArea(props: TextAreaProps) {
	const rest = omit(props, "value", "defaultValue", "onValueChange", "resize", "invalid", "disabled", "required", "class", "ref");
	const field = useOptionalFieldAssociation();
	let area!: HTMLTextAreaElement;

	createRenderEffect(
		() => props.value,
		(value) => {
			if (area && value !== undefined && area.value !== value) area.value = value;
		},
	);

	return (
		<textarea
			{...(rest as JSX.HTMLAttributes<HTMLTextAreaElement>)}
			ref={(el) => {
				area = el;
				setRef(props.ref, el);
			}}
			id={field?.controlId() ?? props.id}
			class={joinClass("mui-textarea", props.class)}
			data-resize={props.resize ?? "vertical"}
			data-invalid={props.invalid || field?.fieldInvalid() ? "" : undefined}
			defaultValue={props.defaultValue}
			disabled={props.disabled || field?.fieldDisabled()}
			required={props.required || field?.fieldRequired()}
			aria-invalid={props.invalid || field?.fieldInvalid() ? "true" : undefined}
			aria-describedby={field?.describedBy()}
			aria-errormessage={field?.errorId()}
			onInput={(event: Event) => {
				props.onValueChange?.((event.target as HTMLTextAreaElement).value);
				(rest.onInput as ((e: Event) => void) | undefined)?.(event);
			}}
		/>
	);
}
