# marketplace

## What it is

Part of the Agents workspace: HTML artifacts a person publishes for their
team to install. A listing's versions are immutable snapshots of the page's
source; an install follows the newest minor version of the major it is on.

## Start here

- `src/domain/service.rs`: every rule — who publishes, sees, installs, runs
  and unlists — over the ports in `src/domain/ports.rs`.
- `src/domain/models.rs`: `Listing`, `Version`, `MarketplaceError`.
- `src/outbound/pg_listing_repo.rs`: the `marketplace_listing`,
  `marketplace_listing_version` and `marketplace_install` tables.
- `src/outbound/document_pages.rs`: the `Pages` port over `agent_code_mode`'s
  `DocumentPrograms`, so a published page builds and runs like a stored HTML
  document, under the same limits.
- `src/outbound/conn_gateway_notifier.rs`: the `VersionNotifier` port, which
  sends installers' open clients `marketplace_listing_version` through the
  connection gateway.
- `src/inbound/axum_router.rs`: `/marketplace/listings...`, in the harness
  spec under the `marketplace` tag.

## Scope

- Owns: the three `marketplace_*` tables and the marketplace routes.
- Not here: building and running pages → `crates/agent_code_mode`; the
  rollout adapter and wiring → `services/agent_harness_service`.

## Boundaries

- Depends on: `agent_code_mode` (page builds and runs), `entity_access`
  (receipts), `macro_authorization`, `connection_gateway_client` (version
  notifications).
- Used by: `services/agent_harness_service`, its only host.

## Checks

- `cargo test -p marketplace` (`SQLX_OFFLINE` unset).

## Gotchas

- Authorization arrives as receipts: the caller's team membership
  (`MemberTeamRole`, absent without a team) and, to publish, an owner receipt
  for the document, minted in the handler. The service checks both name the
  caller.
- A listing with no `team_id` is seen by everyone: one Macro provides. No
  route publishes one yet.
- A minor version is live as soon as it is stored: an install's build reads
  the newest minor of its major. Publishing then sends
  `marketplace_listing_version` (`{ listingId, version }`, no content) to the
  installers on that major, and the web app refetches an open page's build;
  a run with an older build's `sourceHash` answers `409`, and a return to the
  window refetches, as fallbacks. A major version notifies every installer,
  so the Update button appears, and moves an install only through
  `PUT .../install`.
- Version notifications are best effort: a failure is logged and never fails
  the publish. They skip installers no longer in the listing's team, and
  reach at most `MAX_NOTIFIED_INSTALLS` (1000) installers, most recently
  installed first, 100 per gateway request; the rest rely on the fallbacks.
  The `notify_installers` span records `recipient_count` and `truncated`,
  never user ids.
- Publishing refuses a page that doesn't build, so an install never shows
  one.
- Every route answers `404` outside the rollout: `enable-marketplace` and
  `enable-html-code-mode` together, since listings are pages with programs.
  Runs refuse an agent's or a program's token, like a document's.
- An unlisted listing (`removed_at`) leaves browsing and keeps serving those
  who installed it. Its document can then be published again.
- Deleting a user cascades: their installs go, and so do the listings they
  published, with everyone's installs of them.

## Related

- Rules: `.agents/rules/rust/architecture.md`, `.agents/rules/rust/db.md`
