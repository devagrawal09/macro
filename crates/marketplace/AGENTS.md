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
  types in the domain; its service only in `outbound/scheduled_routines.rs`).
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
  the newest minor of its major, and a run with an older build's `sourceHash`
  answers `409`, which makes the preview reload. A major version moves an
  install only through `PUT .../install`.
- Publishing refuses a page that doesn't build, so an install never shows
  one. A routine listing has no page: `installed_page` reads artifacts only.
- Every route answers `404` outside the rollout: `enable-marketplace` and
  `enable-html-code-mode` together, since listings are pages with programs.
  Runs refuse an agent's or a program's token, like a document's.
- An unlisted listing (`removed_at`) leaves browsing and keeps serving those
  who installed it. Its source can then be published again.
- Deleting a user cascades: their installs go, and so do the listings they
  published, with everyone's installs of them. Routines installs made stay
  (their own cleanup deletes the user's routines).

## Related

- Rules: `.agents/rules/rust/architecture.md`, `.agents/rules/rust/db.md`
