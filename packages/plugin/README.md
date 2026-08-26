# `@macro/plugin`

Typed, declarative authoring helpers for Macro Plugins. A plugin module has exactly one default-exported direct `definePlugin({...})`. During local `check` and `build`, the trusted plugin definition and its config imports execute to enumerate descriptors. The CLI does not invoke `render` or `handle`; imported modules must be side-effect-free by contract. See `docs/plugin-authoring.md`.
