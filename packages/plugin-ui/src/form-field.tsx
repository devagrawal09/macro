/**
 * Owns label/description/error association and supplies required, disabled,
 * and invalid state to the wrapped field through context.
 */
import type { ParentProps } from "solid-js";
import { Show } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { FieldAssociationContext, type FieldAssociationValue } from "./internal/context";
import { joinClass, nextId } from "./internal/utils";

export interface FormFieldProps extends ParentProps<{
	label: string;
	description?: JSX.Element;
	error?: JSX.Element;
	required?: boolean;
	disabled?: boolean;
	class?: string;
	style?: JSX.CSSProperties;
}> {}

export function FormField(props: FormFieldProps) {
	const controlId = nextId("field");
	const descriptionId = nextId("desc");
	const errorId = nextId("error");
	const hasError = () => props.error !== undefined;
	const hasDescription = () => props.description !== undefined;

	const value: FieldAssociationValue = {
		controlId: () => controlId,
		describedBy: () => {
			const ids: string[] = [];
			if (hasDescription()) ids.push(descriptionId);
			if (hasError()) ids.push(errorId);
			return ids.length > 0 ? ids.join(" ") : undefined;
		},
		errorId: () => (hasError() ? errorId : undefined),
		fieldInvalid: hasError,
		fieldRequired: () => props.required === true,
		fieldDisabled: () => props.disabled === true,
	};

	return (
		<FieldAssociationContext value={value}>
			<div class={joinClass("mui-field", props.class)} style={props.style} data-disabled={props.disabled ? "" : undefined}>
				<label class="mui-label" for={controlId}>
					{props.label}
					<Show when={props.required}>
						<span class="mui-field-required" aria-hidden="true">
							*
						</span>
					</Show>
				</label>
				{props.children}
				<Show when={props.description}>
					<p class="mui-field-desc" id={descriptionId}>
						{props.description}
					</p>
				</Show>
				<Show when={props.error}>
					<p class="mui-field-error" id={errorId}>
						{props.error}
					</p>
				</Show>
			</div>
		</FieldAssociationContext>
	);
}

