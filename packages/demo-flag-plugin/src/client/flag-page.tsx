import type { PluginClientContext } from "@macro/plugin";
import {
	createCustomEventMessage,
	emitCustomEvent,
	type CustomEventSink,
} from "@macro/plugin";
import {
	Badge,
	Button,
	Stack,
	Text,
	type BadgeProps,
	type ButtonProps,
	type StackProps,
	type TextProps,
} from "@macro/plugin-ui";
import { createSignal } from "solid-js";
import type { JSX } from "solid-js";

/**
 * `@macro/plugin-ui` types its components against the `@solidjs/web` JSX
 * namespace while this package authors JSX through `solid-js`. The runtime
 * contract is identical, so align the views locally instead of duplicating
 * the kit.
 */
type Ui<P> = (props: P) => JSX.Element;
const UiStack = Stack as unknown as Ui<StackProps>;
const UiButton = Button as unknown as Ui<ButtonProps>;
const UiBadge = Badge as unknown as Ui<BadgeProps>;
const UiText = Text as unknown as Ui<TextProps>;

export interface FlagPageProps {
	readonly context: PluginClientContext;
	/**
	 * MessagePort-style sink toward the host. The host does not transfer ports
	 * to client contributions yet, so this stays optional and the page reports
	 * delivery as pending until host wiring lands.
	 */
	readonly sink?: CustomEventSink;
}

/** Small demo surface that emits the declared `flag.toggled` custom event. */
export function FlagPage(props: FlagPageProps) {
	const [enabled, setEnabled] = createSignal(false);
	const [lastMessage, setLastMessage] = createSignal<string>();

	const toggle = () => {
		const next = !enabled();
		setEnabled(next);
		setLastMessage(
			JSON.stringify(
				createCustomEventMessage("flag.toggled", {
					enabled: next,
					actor: props.context.project.id,
				}),
			),
		);
		if (props.sink)
			emitCustomEvent(props.sink, "flag.toggled", {
				enabled: next,
				actor: props.context.project.id,
			});
	};

	return (
		<UiStack gap={4}>
			<UiText role="heading">Demo Flag</UiText>
			<UiStack direction="row" gap={2}>
				<UiButton
					variant={enabled() ? "danger" : "primary"}
					onClick={toggle}
				>
					{enabled() ? "Turn flag off" : "Turn flag on"}
				</UiButton>
				<UiBadge tone={enabled() ? "success" : "neutral"}>
					{enabled() ? "on" : "off"}
				</UiBadge>
			</UiStack>
			{lastMessage() && <UiText role="mono">{lastMessage()}</UiText>}
			{!props.sink && (
				<UiText role="caption" tone="muted">
					Port delivery waits on host wiring; message shape is final.
				</UiText>
			)}
		</UiStack>
	);
}
