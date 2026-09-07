# Customizing Macro

Working notes on making Macro customizable from userspace: the SDK, typed
events over webhooks and SSE, and client extensions delivered as browser code.

- `macro-plugin-plan.md`: the research and decision record. Start here.
- `macro-customization-blog.md`: draft narrative for an external post.
- `customizing-macro.md` and `plugin.md`: earlier ideation, kept for context.
  The plan corrects several claims made in them.

Code that backs the plan:

- `packages/sdk` (`@macro/sdk/browser`, `observeMacroExtensionSlots`)
- `packages/ui` (`@macro/ui`)
- `examples/document-health-extension`
- `apps/web/src/components/app/client-extension` (local-only slots)

An earlier managed plugin platform was removed from the tree; its history is on
the `archive/plugin-platform-2026-08` branch.
