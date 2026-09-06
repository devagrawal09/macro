import { joinClass } from "./internal/utils";

export interface ProgressProps {
  value: number;
  max?: number;
  tone?: "accent" | "success" | "warning" | "failure";
  size?: "sm" | "md";
  class?: string;
  label?: string;
}

export function Progress(props: ProgressProps) {
  const max = () => Math.max(props.max ?? 100, 1);
  const value = () => Math.min(Math.max(props.value, 0), max());
  const percent = () => String((value() / max()) * 100) + "%";

  return (
    <div
      class={joinClass("macro-ui-progress", props.class)}
      data-tone={props.tone ?? "accent"}
      data-size={props.size ?? "md"}
    >
      <progress value={value()} max={max()} aria-label={props.label ?? "Progress"} />
      <span class="macro-ui-progress__track" aria-hidden="true">
        <span class="macro-ui-progress__value" style={{ width: percent() }} />
      </span>
    </div>
  );
}
