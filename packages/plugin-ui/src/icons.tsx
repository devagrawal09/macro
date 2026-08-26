/**
 * Tiny internal inline icon set (current-color SVG, no external assets).
 * These are not part of the public v0 export surface.
 */
import type { PluginIcon, PluginIconProps } from "./icon";

function svg(path: string): PluginIcon {
	return function Icon(props: PluginIconProps) {
		return (
			<svg
				viewBox="0 0 16 16"
				width={props.size ?? 16}
				height={props.size ?? 16}
				fill="none"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
				class={props.class}
			>
				<path d={path} />
			</svg>
		);
	};
}

export const PlusIcon = svg("M8 3v10M3 8h10");
export const SearchIcon = svg("M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10Zm9 4-4.5-4.5");
export const MoreIcon = svg("M4 8h.01M8 8h.01M12 8h.01");
export const WarningIcon = svg("M8 2 1.5 13.5h13L8 2Zm0 4.5v3m0 2h.01");
export const CheckIcon = svg("M2.5 8.5 6 12l7.5-8");
export const InboxIcon = svg("M2 9h3l1.5 2h3L11 9h3M2 9V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v5m0 0v3a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V9");
