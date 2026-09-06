import type { ParentProps } from "solid-js";
import { Dynamic } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { joinClass } from "./internal/utils";

export type TextAs = "span" | "p" | "div" | "label" | "h1" | "h2" | "h3";
export type TextRole = "display" | "title" | "heading" | "body" | "caption" | "mono";
export type TextTone = "default" | "muted" | "subtle" | "accent" | "success" | "warning" | "failure";

export interface TextProps extends ParentProps<{
  as?: TextAs;
  role?: TextRole;
  tone?: TextTone;
  weight?: "regular" | "medium" | "semibold";
  truncate?: boolean | 2 | 3;
  class?: string;
  style?: JSX.CSSProperties;
  id?: string;
}> {}

export function Text(props: TextProps) {
  return (
    <Dynamic
      component={props.as ?? "span"}
      class={joinClass("macro-ui-text", props.class)}
      data-role={props.role ?? "body"}
      data-tone={props.tone ?? "default"}
      data-weight={props.weight}
      data-truncate={props.truncate === true ? "one" : props.truncate}
      id={props.id}
      style={props.style}
    >
      {props.children}
    </Dynamic>
  );
}
