import type { ParentProps } from "solid-js";
import type { JSX } from "@solidjs/web";
import { joinClass } from "./internal/utils";

export type StackGap = "xs" | "sm" | "md" | "lg" | "xl";

export interface StackProps extends ParentProps<{
  direction?: "row" | "column";
  gap?: StackGap;
  align?: "start" | "center" | "end" | "stretch" | "baseline";
  justify?: "start" | "center" | "end" | "between";
  wrap?: boolean;
  class?: string;
  style?: JSX.CSSProperties;
  role?: JSX.HTMLAttributes<HTMLDivElement>["role"];
}> {}

export function Stack(props: StackProps) {
  return (
    <div
      class={joinClass("macro-ui-stack", props.class)}
      data-direction={props.direction ?? "column"}
      data-gap={props.gap ?? "md"}
      data-align={props.align}
      data-justify={props.justify}
      data-wrap={props.wrap ? "" : undefined}
      role={props.role}
      style={props.style}
    >
      {props.children}
    </div>
  );
}
