// `@macro/plugin-ui` ships precompiled Solid client output whose top-level
// templates refuse server-side evaluation. The plugin compiler only executes
// the trusted definition to enumerate descriptors, so this preload gives the
// loader inert stand-ins. Vite still bundles the real kit into client entries.
import { mock } from "bun:test";

mock.module("@macro/plugin-ui", () => ({
	Badge: () => null,
	Button: () => null,
	Stack: () => null,
	Text: () => null,
}));
