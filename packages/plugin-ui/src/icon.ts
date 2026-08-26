/** Shared icon contract accepted by every icon slot in the kit. */
import type { Component } from "solid-js";

export interface PluginIconProps {
	size?: 12 | 16 | 20 | 24;
	class?: string;
	"aria-hidden"?: true;
}

/**
 * An icon component renders inline current-color SVG with a viewBox and no
 * external `<use href>` references.
 */
export type PluginIcon = Component<PluginIconProps>;
