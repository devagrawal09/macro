import type { ParentProps } from "solid-js";
import { Show } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { joinClass } from "./internal/utils";

export interface CardProps extends ParentProps<{
  title?: JSX.Element;
  description?: JSX.Element;
  actions?: JSX.Element;
  padding?: "sm" | "md" | "lg";
  class?: string;
  style?: JSX.CSSProperties;
}> {}

export function Card(props: CardProps) {
  return (
    <section
      class={joinClass("macro-ui-card", props.class)}
      data-padding={props.padding ?? "md"}
      style={props.style}
    >
      <Show when={props.title || props.description || props.actions}>
        <header class="macro-ui-card__header">
          <div class="macro-ui-card__heading">
            <Show when={props.title}>
              <h2 class="macro-ui-card__title">{props.title}</h2>
            </Show>
            <Show when={props.description}>
              <p class="macro-ui-card__description">{props.description}</p>
            </Show>
          </div>
          <Show when={props.actions}>
            <div class="macro-ui-card__actions">{props.actions}</div>
          </Show>
        </header>
      </Show>
      <div class="macro-ui-card__content">{props.children}</div>
    </section>
  );
}
