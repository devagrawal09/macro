/**
 * Semantic list with a compound List.Item covering the common task/inbox row
 * shape: leading slot, strong title, secondary description, meta, badges, and
 * an actions slot outside the activation target.
 */
import type { ParentProps } from "solid-js";
import { Dynamic, Show } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
import { createContext, useContext } from "solid-js";
import { joinClass, nextId } from "./internal/utils";

export interface ListProps extends ParentProps<{
	as?: "ul" | "ol" | "div";
	label?: string;
	divided?: boolean;
	selectionMode?: "none" | "single" | "multiple";
	class?: string;
}> {}

interface ListContextValue {
	selectionMode(): "none" | "single" | "multiple";
}

const ListContext = createContext<ListContextValue>();

function ListBase(props: ListProps) {
	const as = () => props.as ?? "ul";
	return (
		<ListContext value={{ selectionMode: () => props.selectionMode ?? "none" }}>
			<Dynamic
				component={as()}
				class={joinClass("mui-list", props.class)}
				role={as() === "div" ? "list" : undefined}
				aria-label={props.label}
				data-divided={props.divided ? "" : undefined}
			>
				{props.children}
			</Dynamic>
		</ListContext>
	);
}

export interface ListItemProps extends ParentProps<{
	id?: string;
	title: JSX.Element;
	description?: JSX.Element;
	leading?: JSX.Element;
	meta?: JSX.Element;
	badges?: JSX.Element;
	actions?: JSX.Element;
	unread?: boolean;
	selected?: boolean;
	disabled?: boolean;
	onActivate?(): void;
	onSelectedChange?(selected: boolean): void;
	/** Accessible name for the multiple-selection checkbox. */
	selectionLabel?: string;
	class?: string;
}> {}

function ListItem(props: ListItemProps) {
	const list = useContext(ListContext);
	const mode = () => list?.selectionMode() ?? "none";
	const selectable = () => mode() === "multiple";
	const activatable = () => props.onActivate !== undefined && props.disabled !== true;
	const checkId = nextId("item-check");
	return (
		<li
			id={props.id}
			class={joinClass("mui-list-item", props.class)}
			data-selected={props.selected ? "" : undefined}
			data-unread={props.unread ? "" : undefined}
			data-disabled={props.disabled ? "" : undefined}
			data-interactive={props.onActivate !== undefined || selectable() ? "" : undefined}
		>
			<Show when={selectable()}>
				<input
					type="checkbox"
					id={checkId}
					class="mui-check"
					aria-label={props.selectionLabel ?? "Select"}
					checked={props.selected}
					disabled={props.disabled}
					onChange={(event: Event) => {
						props.onSelectedChange?.((event.target as HTMLInputElement).checked);
					}}
				/>
			</Show>
			<div class="mui-item-main">
				<Show when={props.leading}>
					<span class="mui-item-leading">{props.leading}</span>
				</Show>
				<Show
					when={activatable()}
					fallback={
						<div class="mui-item-text">
							<span class="mui-item-title" data-unread={props.unread ? "" : undefined}>
								{props.title}
							</span>
							<Show when={props.description}>
								<span class="mui-item-desc">{props.description}</span>
							</Show>
						</div>
					}
				>
					<button
						type="button"
						class="mui-item-activate"
						aria-current={mode() === "single" && props.selected ? "true" : undefined}
						onClick={() => props.onActivate?.()}
					>
						<span class="mui-item-title" data-unread={props.unread ? "" : undefined}>
							{props.title}
						</span>
						<Show when={props.description}>
							<span class="mui-item-desc">{props.description}</span>
						</Show>
					</button>
				</Show>
				<Show when={props.badges}>
					<span class="mui-item-badges">{props.badges}</span>
				</Show>
			</div>
			<Show when={props.meta}>
				<span class="mui-item-meta">{props.meta}</span>
			</Show>
			<Show when={props.actions}>
				<span class="mui-item-actions">{props.actions}</span>
			</Show>
		</li>
	);
}

/** `List` renders semantic ul/ol by default; `List.Item` is the compound row. */
export const List = Object.assign(ListBase, { Item: ListItem }) as ((props: ListProps) => JSX.Element) & {
	Item(props: ListItemProps): JSX.Element;
};
