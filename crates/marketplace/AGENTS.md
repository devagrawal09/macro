# marketplace

## What it is

Part of the Agents workspace: HTML artifacts and routines a person publishes
for their team to install. A listing's versions are immutable snapshots of a
page's source or a routine; an install follows the newest minor version of
the major it is on. A routine install is the installer's own routine, which
minor versions update until they edit it.

## Start here

- `src/domain/service.rs`: every rule — who publishes, sees, installs, runs
  and unlists — over the ports in `src/domain/ports.rs`.
- `src/domain/models.rs`: `Listing`, `Version`, `ListingContent`,
  `RoutineInstall`, `MarketplaceError`.
- `src/outbound/pg_listing_repo.rs`: the `marketplace_listing`,
  `marketplace_listing_version` and `marketplace_install` tables.
- `src/outbound/scheduled_routines.rs`: the `Routines` port over
  `scheduled_action`'s service: reading a routine to publish, and creating
  and updating installs' copies as their person, under its checks.
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
- Not here: building and running pages → `crates/agent_code_mode`; a
  routine's snapshot, filling it in and its interface →
  `scheduled_action`'s `domain/templates` (`RoutineSnapshot`,
  `templates/fill.rs`, `templates/interface.rs`); the rollout adapter and
  wiring → `services/agent_harness_service`.

## Boundaries

- Depends on: `agent_code_mode` (page builds and runs), `entity_access`
  (receipts), `macro_authorization`, `scheduled_action` (the template data
  types in the domain; its service only in `outbound/scheduled_routines.rs`),
  `connection_gateway_client` (version notifications).
- Used by: `services/agent_harness_service`, its only host.

## Checks

- `cargo test -p marketplace` (`SQLX_OFFLINE` unset). The
  `scheduled_routines` tests run the adapter over the real scheduler service
  and Postgres, proving an install's task hash survives storage and a no-op
  update.

## Gotchas

- Authorization arrives as receipts: the caller's team membership
  (`MemberTeamRole`, absent without a team) and, to publish, an owner receipt
  for the document (`EntityType::Document`) or routine
  (`EntityType::ScheduledAction`), minted in the handler. The service checks
  both name the caller. Only a user-owned routine publishes.
- A version stores `html` or `routine` by its `kind`, enforced by a CHECK and
  a `(listing_id, kind)` key to its listing. A listing's source is `kind` +
  `source_id`, listed once at a time.
- A routine version's bump is the server's: when `RoutineSnapshot::interface`
  (variable keys and kinds, required apps, trigger kinds, knob types) differs
  from the newest version's, it is a major version whatever was asked
  (`routine_bump`), and `PublishedVersion.bump` says so.
- Installing a routine fills the newest version in on the server
  (`RoutineSnapshot::fill`) from the request's `VariableFill`s and time zone,
  merged over those stored, and creates the installer's routine, disabled
  unless asked. The install keeps the routine id, version, fills and the
  task's hash (`sdk` left out, keys sorted). A `choice` variable without a
  fill answers 422 `MissingRoutineFills` listing them. Taking a new major
  refills the same routine, replacing edits, and it follows again.
- A first routine install claims its row before creating the routine
  (`claim_routine_install`: `INSERT ... ON CONFLICT DO UPDATE ... WHERE`
  the claim is stale, `RETURNING`), so two installs at once make one
  routine; the loser answers 409 `InstallInProgress`. The claimer records
  the routine with `save_routine_install`, which ends the claim, or
  releases it when creating fails. A claim left by a stopped process goes
  stale after `CLAIM_STALE_AFTER_SECS` (120 s) and the next install takes it
  over. A claim without a routine is not an install: listings and counts
  leave it out.
- A minor routine version updates every install on its major behind it,
  after the version is stored, eight at a time, on a spawned task the
  request awaits (dropping the request can't leave a routine updated and its
  install unrecorded, which would read as an edit). A routine whose task no
  longer hashes alike was edited: the install is detached (`detached_at`) and
  the routine left alone. A failed update sets `update_failed_at`, is logged,
  and fails nothing else; a routine that can't be found (deleted, or a
  passing access error) is such a failure, never a detach, so a deleted one
  keeps failing until its person uninstalls or installs again. Nothing retries it
  on a timer: the next minor version retries every install still behind,
  and so does the person taking the listing again.
- The update goes through `ScheduledActionService::update_action` with an
  edit receipt minted for the installer, so it bumps the routine's
  `configuration_revision` and a run in progress keeps its configuration.
- Uninstalling a routine deletes only the install row: the routine stays the
  person's own.
- A listing with no `team_id` is seen by everyone: one Macro provides. No
  route publishes one yet.
- A minor version is live as soon as it is stored: an install's build reads
  the newest minor of its major. Publishing then sends
  `marketplace_listing_version` (`{ listingId, version }`, no content) to the
  installers on that major, and the web app refetches an open page's build;
  a run with an older build's `sourceHash` answers `409`, and a return to the
  window refetches, as fallbacks. A major version notifies every installer,
  so the Update button appears, and moves an install only through
  `PUT .../install`. Minor or major is read from the stored version (minor
  `0` is a major), so a routine version forced to a major notifies everyone.
  A minor routine version notifies after updating installs, so the listings
  the web app refetches show them.
- Version notifications are best effort: a failure is logged and never fails
  the publish. They skip installers no longer in the listing's team, and
  reach at most `MAX_NOTIFIED_INSTALLS` (1000) installers, most recently
  installed first, 100 per gateway request; the rest rely on the fallbacks.
  A routine install's pending claim isn't an installer.
  The `notify_installers` span records `recipient_count` and `truncated`,
  never user ids.
- Publishing refuses a page that doesn't build, so an install never shows
  one. A routine listing has no page: `installed_page` reads artifacts only.
- Every version keeps its publisher's `changelog` (`Changelog`: trimmed,
  blank is none, at most `MAX_CHANGELOG_CHARS` 2000, also a CHECK) and, for a
  routine, `forced_major` when `routine_bump` overrode a minor.
  `GET /marketplace/listings/{id}/versions` answers them newest first, without
  content, to whoever sees the listing. A listing's `installedVersion` is the
  routine's `routine_minor`, or the newest minor of a page install's major.
- An install can be pinned (`pinned_minor`, `PUT`/`DELETE
  .../install/pin`) to the version it has: `installed_page` builds that
  minor, `installs_behind` skips a pinned routine, and installers still get
  version notifications (the web app refreshes lists, not a pinned page).
  Unpinning a routine runs `update_install` once on its own task
  (`catch_up_routine`), with the same edit/detach rules. Taking a version
  (`install_latest`, `save_routine_install`) unpins. A detached routine
  can't be pinned.
- Every route answers `404` outside the rollout: `enable-marketplace` and
  `enable-html-code-mode` together, since listings are pages with programs.
  Runs refuse an agent's or a program's token, like a document's.
- An unlisted listing (`removed_at`) leaves browsing and keeps serving those
  who installed it. Its source can then be published again.
- `GET /marketplace/listings` pages newest first by `(created_at, id)`: `limit`
  (default 50, at most 100) and the opaque `nextCursor` it answers. It lists
  what is still listed, or with `installed=true` the caller's installs,
  unlisted ones included; `q` matches title or description (`ILIKE`, its
  wildcards escaped); `kind` and `sourceId` together find a document's or
  routine's live listing, which the publish dialog needs. The service's own
  one-listing lookup (`ListingRepo::listing`) sees listed and installed
  listings alike.
- Deleting a user cascades: their installs go, and so do the listings they
  published, with everyone's installs of them. Routines installs made stay
  (their own cleanup deletes the user's routines).

## Related

- Rules: `.agents/rules/rust/architecture.md`, `.agents/rules/rust/db.md`
