/**
 * Typed host intents crossing the plugin bridge.
 *
 * This closed union mirrors the v0 `ClientHostIntent` contract of the Macro
 * plugin platform. When the plugin SDK publishes browser types, this module
 * becomes a re-export of the SDK type. UI components never synthesize URLs,
 * never touch the host window, and never navigate: the host owns
 * routing and focus transfer after an intent is dispatched.
 */

/** A reference to an entity known to the host. */
export interface ClientEntityRef {
	type: string;
	id: string;
}

export type ClientHostIntent =
	| { type: "entity.open"; entity: ClientEntityRef }
	| { type: "project.open"; projectId: string };

/** Dispatches a typed intent through the plugin bridge. */
export type DispatchIntent = (intent: ClientHostIntent) => Promise<void>;
