import type { ParentProps, Ref } from "solid-js";
import type { JSX } from "solid-js";
import { joinClass } from "./internal/utils";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends ParentProps<{
  type?: "button" | "submit" | "reset";
  variant?: ButtonVariant;
  size?: ButtonSize;
  disabled?: boolean;
  loading?: boolean;
  fullWidth?: boolean;
  onClick?: JSX.EventHandlerUnion<HTMLButtonElement, MouseEvent>;
  class?: string;
  id?: string;
  title?: string;
  name?: string;
  value?: string;
  ref?: Ref<HTMLButtonElement>;
  "aria-label"?: string;
}> {}

export function Button(props: ButtonProps) {
  return (
    <button
      ref={props.ref}
      type={props.type ?? "button"}
      class={joinClass("macro-ui-button", props.class)}
      data-variant={props.variant ?? "primary"}
      data-size={props.size ?? "md"}
      data-loading={props.loading ? "" : undefined}
      data-full-width={props.fullWidth ? "" : undefined}
      disabled={props.disabled || props.loading}
      aria-busy={props.loading ? "true" : undefined}
      aria-label={props["aria-label"]}
      id={props.id}
      title={props.title}
      name={props.name}
      value={props.value}
      onClick={props.onClick}
    >
      <span class="macro-ui-button__label">{props.children}</span>
    </button>
  );
}
