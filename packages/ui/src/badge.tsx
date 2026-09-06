import type { ParentProps } from "solid-js";
import { joinClass } from "./internal/utils";

export type BadgeTone = "neutral" | "accent" | "success" | "warning" | "failure";

export interface BadgeProps extends ParentProps<{
  tone?: BadgeTone;
  class?: string;
  title?: string;
  "aria-label"?: string;
}> {}

export function Badge(props: BadgeProps) {
  return (
    <span
      class={joinClass("macro-ui-badge", props.class)}
      data-tone={props.tone ?? "neutral"}
      title={props.title}
      aria-label={props["aria-label"]}
    >
      {props.children}
    </span>
  );
}
