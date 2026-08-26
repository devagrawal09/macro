# `@macro/plugin`

Typed, declarative authoring helpers for Macro Plugins. A plugin module has exactly one default-exported direct `definePlugin({...})`. During local `check` and `build`, the trusted plugin definition and its config imports execute to enumerate descriptors. The CLI does not invoke `render` or `handle`; imported modules must be side-effect-free by contract. See `docs/plugin-authoring.md`.


## Current runtime-neutral context

Client and best-effort server callbacks receive project identity, an `AbortSignal`, and a host-supplied `macro` facade. The current facade has only task list, read, create, rename, and best-effort subscription operations. Server callbacks also receive a structured logger. Plugins do not construct a Macro SDK client or handle transport/auth configuration.

Capability strings describe requested access for generated metadata. This compiler checkpoint does not enforce capabilities at runtime. The future host must supply a scoped facade and enforce the installation grant.
