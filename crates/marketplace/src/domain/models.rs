//! Domain models for the marketplace.

#[cfg(test)]
mod test;

use std::collections::BTreeMap;
use std::fmt;

use agent_code_mode::domain::{DocumentBuild, DocumentProgramError, SourceHash};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use entity_access::domain::models::{EntityAccessReceipt, OwnerAccessLevel};
use macro_user_id::user_id::MacroUserIdStr;
use scheduled_action::domain::templates::{
    RoutineSnapshot, TemplateVariable, VariableFill, fill::RoutineFills,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// The per-user rollout flag, shared with the web app's `defineFlag` key.
pub const MARKETPLACE_FLAG: &str = "enable-marketplace";

/// Longest title a listing takes, in characters.
pub const MAX_TITLE_CHARS: usize = 120;

/// Longest description a listing takes, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 1000;

/// The connection gateway message that tells installers a version was
/// published, shared with the web app's websocket handler.
pub const LISTING_VERSION_MESSAGE_TYPE: &str = "marketplace_listing_version";

/// Most installers one new version notifies. A listing with more installs
/// notifies its most recently installed ones; the rest pick the version up on
/// their next run or when their window regains focus.
pub const MAX_NOTIFIED_INSTALLS: u16 = 1000;

/// Identifies a listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct ListingId(Uuid);

impl ListingId {
    /// A new UUIDv7 identifier.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wrap an id that is already stored.
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// The inner UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ListingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What a listing publishes.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    ToSchema,
    strum::Display,
    strum::EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ListingKind {
    /// A stored HTML page.
    Artifact,
    /// A routine, which each install copies as its person's own.
    Routine,
}

/// Who sees a listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ListingScope {
    /// The members of one team.
    #[serde(rename_all = "camelCase")]
    Team {
        /// The team.
        team_id: Uuid,
    },
    /// Everyone: a listing Macro provides.
    Global,
}

impl ListingScope {
    /// The team a team listing belongs to.
    pub fn team_id(&self) -> Option<Uuid> {
        match self {
            Self::Team { team_id } => Some(*team_id),
            Self::Global => None,
        }
    }
}

/// Who a first publish lists to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublishScope {
    /// The publisher's team, at once.
    #[default]
    Team,
    /// Everyone, once another member of Macro's publisher team approves it.
    Global,
}

/// Where a version stands in review. Team versions are approved as they are
/// stored; a global version waits for a second publisher.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ReviewState {
    /// Waiting: only Macro's publishers see it.
    Pending,
    /// Live for everyone who sees the listing.
    Approved,
    /// Turned down: never shown, and out of the numbering.
    Rejected,
}

/// What a reviewer decided about a pending version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewDecision {
    /// It goes live.
    Approve,
    /// It never does, for this reason when given.
    Reject {
        /// Why, for its publisher.
        reason: Option<String>,
    },
}

/// Longest reason a rejection takes, in characters.
pub const MAX_REJECTION_REASON_CHARS: usize = 1000;

impl ReviewDecision {
    /// The decision trimmed, or why it can't be used. A blank reason is none.
    pub fn normalized(self) -> Result<Self, MarketplaceError> {
        match self {
            Self::Approve => Ok(Self::Approve),
            Self::Reject { reason } => {
                let reason = reason
                    .as_deref()
                    .map(str::trim)
                    .filter(|reason| !reason.is_empty());
                if reason.is_some_and(|reason| reason.chars().count() > MAX_REJECTION_REASON_CHARS)
                {
                    return Err(MarketplaceError::Invalid(format!(
                        "a rejection's reason is at most {MAX_REJECTION_REASON_CHARS} characters"
                    )));
                }
                Ok(Self::Reject {
                    reason: reason.map(str::to_owned),
                })
            }
        }
    }

    /// The state the version takes.
    pub fn state(&self) -> ReviewState {
        match self {
            Self::Approve => ReviewState::Approved,
            Self::Reject { .. } => ReviewState::Rejected,
        }
    }
}

/// Who is asking, as listing visibility needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Audience {
    /// Their team, whose listings they see.
    pub team: Option<Uuid>,
    /// They belong to Macro's publisher team, so they also see global
    /// listings and versions still waiting for review.
    pub global_publisher: bool,
}

/// A listing's version. An install follows minor versions by itself and
/// takes a major version only when its person asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct Version {
    /// Changes an install waits for.
    pub major: i32,
    /// Changes every install on this major gets at once.
    pub minor: i32,
}

impl Version {
    /// A listing's first version.
    pub const FIRST: Self = Self { major: 1, minor: 0 };

    /// The version after this one.
    pub fn next(self, bump: VersionBump) -> Self {
        match bump {
            VersionBump::Minor => Self {
                major: self.major,
                minor: self.minor.saturating_add(1),
            },
            VersionBump::Major => Self {
                major: self.major.saturating_add(1),
                minor: 0,
            },
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// How a new version follows the newest one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VersionBump {
    /// Reaches every install on the newest major at once.
    Minor,
    /// Waits for each person to take it.
    Major,
}

/// A listing, as the person asking sees it.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// Its id.
    pub id: ListingId,
    /// What it publishes.
    pub kind: ListingKind,
    /// Who sees it.
    pub scope: ListingScope,
    /// Who published it.
    #[schema(value_type = String)]
    pub publisher: MacroUserIdStr<'static>,
    /// What its versions are published from: a document for an artifact, a
    /// routine for a routine.
    pub source_id: Uuid,
    /// Its name.
    pub title: String,
    /// What it does, in a few lines.
    pub description: String,
    /// Its newest approved version. A global listing not yet approved, which
    /// only Macro's publishers see, shows the version waiting for review.
    pub latest_version: Version,
    /// The version waiting for review, shown only to Macro's publishers.
    pub pending_version: Option<Version>,
    /// The major version the person asking is on, when they installed it.
    pub installed_major: Option<i32>,
    /// The routine the person asking got from installing it.
    pub installed_routine: Option<InstalledRoutine>,
    /// How many people installed it.
    pub install_count: i64,
    /// Unlisted: only those who installed it still see it.
    pub removed: bool,
    /// When it was first published.
    pub created_at: DateTime<Utc>,
    /// When it last changed.
    pub updated_at: DateTime<Utc>,
}

impl Listing {
    /// It has a version anyone who sees it may install. A global listing
    /// whose first version waits for review has none.
    pub fn has_approved_version(&self) -> bool {
        self.pending_version != Some(self.latest_version)
    }
}

/// Listings a page holds when the request doesn't say.
pub const DEFAULT_PAGE_SIZE: u16 = 50;

/// Most listings a page holds.
pub const MAX_PAGE_SIZE: u16 = 100;

/// Longest search a page takes, in characters.
pub const MAX_SEARCH_CHARS: usize = 200;

/// Which listings a page is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingsShown {
    /// Those still listed: what Browse shows.
    Listed,
    /// Those the person asking installed, unlisted ones included.
    Installed,
}

/// Where the next page of listings starts: after this listing, in their
/// newest-first order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListingCursor {
    /// The last listing's `created_at`.
    pub created_at: DateTime<Utc>,
    /// The last listing's id, which orders listings created together.
    pub id: ListingId,
}

impl ListingCursor {
    /// The cursor after `listing`.
    pub fn after(listing: &Listing) -> Self {
        Self {
            created_at: listing.created_at,
            id: listing.id,
        }
    }

    /// The cursor as a client holds it. Clients send it back as it is and
    /// read nothing into it.
    pub fn encode(&self) -> String {
        format!("{}.{}", self.created_at.timestamp_micros(), self.id)
    }

    /// A cursor a client sent back.
    pub fn decode(cursor: &str) -> Result<Self, MarketplaceError> {
        let invalid = || MarketplaceError::Invalid("invalid cursor".to_owned());
        let (micros, id) = cursor.split_once('.').ok_or_else(invalid)?;
        let micros = micros.parse::<i64>().map_err(|_| invalid())?;
        let created_at = DateTime::from_timestamp_micros(micros).ok_or_else(invalid)?;
        let id = Uuid::parse_str(id).map_err(|_| invalid())?;

        Ok(Self {
            created_at,
            id: ListingId::from_uuid(id),
        })
    }
}

/// One page of listings to read, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingQuery {
    /// Which listings it is drawn from.
    pub shown: ListingsShown,
    /// Text the title or description holds, ignoring case.
    pub search: Option<String>,
    /// Only listings of this kind.
    pub kind: Option<ListingKind>,
    /// Only listings published from this source: with `kind`, a document's
    /// or routine's live listing, which the publish dialog looks up.
    pub source_id: Option<Uuid>,
    /// Start after this listing; the first page when absent.
    pub after: Option<ListingCursor>,
    /// Most listings the page holds.
    pub limit: u16,
}

impl ListingQuery {
    /// A page as a client asks for it, or why it can't be read. A blank
    /// search searches nothing.
    pub fn new(
        shown: ListingsShown,
        search: Option<&str>,
        kind: Option<ListingKind>,
        source_id: Option<Uuid>,
        cursor: Option<&str>,
        limit: Option<u16>,
    ) -> Result<Self, MarketplaceError> {
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
        if !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(MarketplaceError::Invalid(format!(
                "limit must be between 1 and {MAX_PAGE_SIZE}"
            )));
        }

        let search = search.map(str::trim).filter(|search| !search.is_empty());
        if search.is_some_and(|search| search.chars().count() > MAX_SEARCH_CHARS) {
            return Err(MarketplaceError::Invalid(format!(
                "a search is at most {MAX_SEARCH_CHARS} characters"
            )));
        }

        Ok(Self {
            shown,
            search: search.map(str::to_owned),
            kind,
            source_id,
            after: cursor.map(ListingCursor::decode).transpose()?,
            limit,
        })
    }
}

/// One page of listings, newest first.
#[derive(Debug, Clone, PartialEq)]
pub struct ListingPage {
    /// The page's listings.
    pub listings: Vec<Listing>,
    /// Where the next page starts; absent on the last page.
    pub next: Option<ListingCursor>,
}

/// A page of listings, and what the person asking may do beyond their team.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewerListings {
    /// The page.
    pub page: ListingPage,
    /// They belong to Macro's publisher team: they may publish to everyone
    /// and review what other publishers did.
    pub global_publisher: bool,
}

/// What a person types to publish a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingText {
    /// Its name.
    pub title: String,
    /// What it does.
    pub description: String,
}

impl ListingText {
    /// The text trimmed, or why it can't be used.
    pub fn normalized(self) -> Result<Self, MarketplaceError> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err(MarketplaceError::Invalid(
                "a listing needs a title".to_owned(),
            ));
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(MarketplaceError::Invalid(format!(
                "a listing's title is at most {MAX_TITLE_CHARS} characters"
            )));
        }

        let description = self.description.trim();
        if description.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(MarketplaceError::Invalid(format!(
                "a listing's description is at most {MAX_DESCRIPTION_CHARS} characters"
            )));
        }

        Ok(Self {
            title: title.to_owned(),
            description: description.to_owned(),
        })
    }
}

/// A page's source as published: what a version stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedPage {
    /// The hash of `html`.
    pub source_hash: SourceHash,
    /// The page's source.
    pub html: String,
}

/// A routine's snapshot as published: what a version stores.
#[derive(Debug, Clone, PartialEq)]
pub struct PublishedRoutine {
    /// The hash of the snapshot.
    pub source_hash: SourceHash,
    /// The routine as each install gets it.
    pub snapshot: RoutineSnapshot,
}

impl PublishedRoutine {
    /// Hash `snapshot` by its content, so publishing it again is noticed.
    pub fn new(snapshot: RoutineSnapshot) -> Self {
        let json = serde_json::to_value(&snapshot)
            .map(|value| canonical_json(&value))
            .unwrap_or_default();

        Self {
            source_hash: SourceHash::of(&json),
            snapshot,
        }
    }
}

/// What a version holds.
#[derive(Debug, Clone, PartialEq)]
pub enum ListingContent {
    /// An artifact's page.
    Page(PublishedPage),
    /// A routine's snapshot.
    Routine(PublishedRoutine),
}

impl ListingContent {
    /// The kind of listing that holds it.
    pub fn kind(&self) -> ListingKind {
        match self {
            Self::Page(_) => ListingKind::Artifact,
            Self::Routine(_) => ListingKind::Routine,
        }
    }

    /// The hash that tells two versions' content apart.
    pub fn source_hash(&self) -> &SourceHash {
        match self {
            Self::Page(page) => &page.source_hash,
            Self::Routine(routine) => &routine.source_hash,
        }
    }
}

/// A listing's routine version.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutineVersion {
    /// Its version.
    pub version: Version,
    /// The routine it holds.
    pub snapshot: RoutineSnapshot,
}

/// A listing to store, with its first version.
#[derive(Debug, Clone, PartialEq)]
pub struct NewListing {
    /// Its id.
    pub id: ListingId,
    /// Who publishes it.
    pub publisher: MacroUserIdStr<'static>,
    /// The team that sees it; none for everyone.
    pub team_id: Option<Uuid>,
    /// The document or routine it is published from.
    pub source_id: Uuid,
    /// Its title and description.
    pub text: ListingText,
    /// Its first version's content, which decides its kind.
    pub content: ListingContent,
    /// Its first version's review: approved for a team, pending for everyone.
    pub review: ReviewState,
}

/// What became of a new version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddedVersion {
    /// It was stored as this version.
    Added(Version),
    /// The newest version already holds this source.
    Unchanged,
    /// Another version is still waiting for review.
    ReviewPending,
    /// The listing is gone or unlisted.
    Missing,
}

/// A version waiting for review, as the review list shows it.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingVersion {
    /// Its listing.
    pub listing_id: ListingId,
    /// What the listing publishes.
    pub kind: ListingKind,
    /// The listing's name.
    pub title: String,
    /// What it does.
    pub description: String,
    /// Who published the version, who may not review it.
    #[schema(value_type = String)]
    pub publisher: MacroUserIdStr<'static>,
    /// The version waiting.
    pub version: Version,
    /// The listing's newest approved version, absent for a new listing.
    pub approved_version: Option<Version>,
    /// When it was published.
    pub published_at: DateTime<Utc>,
}

/// A pending version with what a reviewer needs to judge it.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingReview {
    /// The version and its listing.
    pub pending: PendingVersion,
    /// What it holds.
    pub content: ListingContent,
}

/// A pending version as a reviewer reads it: never run.
#[derive(Debug, Clone)]
pub enum ReviewPreview {
    /// A page: its build, which the reviewer renders with programs refused,
    /// and its source, which shows what each program does.
    Page {
        /// The version.
        pending: PendingVersion,
        /// Its build.
        build: DocumentBuild,
        /// Its source.
        html: String,
    },
    /// A routine's snapshot, and how its interface differs from the newest
    /// approved version's.
    Routine {
        /// The version.
        pending: PendingVersion,
        /// The routine as installs would get it.
        snapshot: RoutineSnapshot,
        /// What it asks of installs that the approved version didn't, or no
        /// longer asks; everything for a new listing.
        interface_changes: Vec<InterfaceChange>,
    },
}

/// One part of a routine's interface added or removed by a version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceChange {
    /// Which part.
    pub part: InterfacePart,
    /// Added or removed.
    pub change: InterfaceChangeKind,
    /// The part as people read it, such as a variable's key and kind.
    pub name: String,
}

/// A part of a routine's interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterfacePart {
    /// A variable each install fills in.
    Variable,
    /// An app it requires.
    App,
    /// A kind of trigger that starts it.
    Trigger,
    /// A code step's knob.
    Knob,
}

/// Whether a part of the interface came or went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceChangeKind {
    /// New in this version.
    Added,
    /// Gone in this version.
    Removed,
}

/// How `next`'s interface differs from `approved`'s, or all of it when
/// nothing is approved yet: removals first, then additions, each by part.
pub fn interface_changes(
    approved: Option<&RoutineSnapshot>,
    next: &RoutineSnapshot,
) -> Vec<InterfaceChange> {
    fn named(snapshot: &RoutineSnapshot) -> std::collections::BTreeSet<(InterfacePart, String)> {
        let interface = snapshot.interface();
        let variables = interface
            .variables
            .iter()
            .map(|(key, kind)| (InterfacePart::Variable, format!("{key} ({kind})")));
        let apps = interface
            .apps
            .iter()
            .map(|app| (InterfacePart::App, app.clone()));
        let triggers = interface
            .triggers
            .iter()
            .map(|trigger| (InterfacePart::Trigger, (*trigger).to_owned()));
        let knobs = interface.knobs.iter().map(|(step, knob, kind, read_as)| {
            (
                InterfacePart::Knob,
                format!("{step}.{knob} ({kind}: {read_as})"),
            )
        });
        variables.chain(apps).chain(triggers).chain(knobs).collect()
    }

    let before = approved.map(named).unwrap_or_default();
    let after = named(next);
    let removed = before
        .difference(&after)
        .map(|(part, name)| (InterfaceChangeKind::Removed, *part, name.clone()));
    let added = after
        .difference(&before)
        .map(|(part, name)| (InterfaceChangeKind::Added, *part, name.clone()));
    removed
        .chain(added)
        .map(|(change, part, name)| InterfaceChange { part, change, name })
        .collect()
}

/// A version just published, as its installers hear of it: no content, only
/// what to refetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionNotice {
    /// The listing.
    pub listing_id: ListingId,
    /// Its new version.
    pub version: Version,
}

/// The page a person's install shows: the newest minor version of the major
/// they are on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPage {
    /// The version shown.
    pub version: Version,
    /// Its source.
    pub html: String,
}

/// An installed page's build.
#[derive(Debug, Clone)]
pub struct InstalledBuild {
    /// The version built.
    pub version: Version,
    /// The page to render, or why it doesn't build.
    pub build: DocumentBuild,
    /// The version's source, which a page without programs is shown as.
    pub html: String,
}

/// What a person publishes from: a document they own or a routine they own.
#[derive(Debug, Clone)]
pub enum PublishSource {
    /// A stored HTML document, as an artifact.
    Artifact(EntityAccessReceipt<OwnerAccessLevel>),
    /// A routine, with the references marked as variables.
    Routine {
        /// The owner's receipt for the routine.
        routine: EntityAccessReceipt<OwnerAccessLevel>,
        /// Its user-specific parts, as when sharing it as a template.
        variables: Vec<TemplateVariable>,
    },
}

impl PublishSource {
    /// The receipt naming what is published.
    pub fn receipt(&self) -> &EntityAccessReceipt<OwnerAccessLevel> {
        match self {
            Self::Artifact(receipt)
            | Self::Routine {
                routine: receipt, ..
            } => receipt,
        }
    }

    /// The kind of listing it publishes.
    pub fn kind(&self) -> ListingKind {
        match self {
            Self::Artifact(_) => ListingKind::Artifact,
            Self::Routine { .. } => ListingKind::Routine,
        }
    }
}

/// A new version and how it was numbered.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedVersion {
    /// The listing with its new version.
    pub listing: Listing,
    /// The bump applied: a routine whose interface changed takes a major
    /// version whatever was asked.
    pub bump: VersionBump,
    /// Installs a minor routine version reached.
    pub updates: RoutineUpdates,
    /// `pending` for a global listing's version, which reaches nobody until
    /// another publisher approves it.
    pub review: ReviewState,
}

/// What a review decided, and the installs an approved minor routine
/// version reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOutcome {
    /// `approved` or `rejected`.
    pub state: ReviewState,
    /// Installs the version reached when it went live.
    pub updates: RoutineUpdates,
}

/// What became of the installs a minor routine version reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoutineUpdates {
    /// Routines updated to the new version.
    pub updated: u32,
    /// Routines their person edited, now detached and left alone.
    pub detached: u32,
    /// Routines that could not be updated this time.
    pub failed: u32,
}

/// What a person sends to install a routine listing, or take its newest
/// major version.
#[derive(Debug, Clone, Default)]
pub struct InstallRequest {
    /// Their time zone; required the first time.
    pub timezone: Option<Tz>,
    /// Fills for the routine's variables, by key, on top of those stored.
    pub variables: BTreeMap<String, VariableFill>,
    /// Whether their routine starts switched on.
    pub enabled: bool,
}

/// A routine install: the copy a person got and how it was filled in.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutineInstall {
    /// The version their routine was last filled in from.
    pub version: Version,
    /// Their routine.
    pub routine_id: Uuid,
    /// What they chose when installing it.
    pub fills: RoutineFills,
    /// The hash of their routine's task as installed or last updated.
    pub task_hash: TaskHash,
}

/// A routine install the marketplace still keeps up to date.
#[derive(Debug, Clone, PartialEq)]
pub struct FollowingInstall {
    /// Who installed it.
    pub user: MacroUserIdStr<'static>,
    /// The install.
    pub install: RoutineInstall,
}

/// The routine an install created, as its person sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRoutine {
    /// Their routine.
    pub routine_id: Uuid,
    /// They edited it, so updates no longer reach it.
    pub detached: bool,
    /// The last update could not be applied; the next version tries again.
    pub update_failed: bool,
}

/// The hash of a routine's task, which tells whether its person edited it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskHash(pub String);

/// A routine created for an install.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutineCopy {
    /// Its id.
    pub routine_id: Uuid,
    /// The hash of its task as saved.
    pub task_hash: TaskHash,
}

/// What updating an installed routine came to.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutineUpdate {
    /// Saved; its task now hashes to this.
    Updated(TaskHash),
    /// Its person edited it since, so it was left alone.
    Edited,
    /// It no longer exists or is no longer theirs.
    Gone,
}

/// What became of one install's update, as recorded.
#[derive(Debug, Clone, PartialEq)]
pub enum InstallUpdate {
    /// Its routine is on `version`, with this task.
    Updated {
        /// The version applied.
        version: Version,
        /// Its task's hash now.
        task_hash: TaskHash,
    },
    /// Its routine is its person's own from now on.
    Detached,
    /// It failed; the install stays where it was.
    Failed,
}

/// JSON with every object's keys sorted, so equal values hash alike however
/// they were stored.
pub fn canonical_json(value: &serde_json::Value) -> String {
    fn sorted(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => {
                let mut keys: Vec<&String> = fields.keys().collect();
                keys.sort();
                serde_json::Value::Object(
                    keys.into_iter()
                        .map(|key| (key.clone(), sorted(&fields[key])))
                        .collect(),
                )
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(sorted).collect())
            }
            other => other.clone(),
        }
    }

    sorted(value).to_string()
}

/// Why a marketplace request could not be done.
#[derive(Debug, thiserror::Error)]
pub enum MarketplaceError {
    /// No such listing, or not one the caller may see.
    #[error("listing not found")]
    NotFound,
    /// Publishing needs a team to publish to.
    #[error("join a team to publish to its marketplace")]
    NoTeam,
    /// The caller may see the listing but not change it.
    #[error("only a listing's publisher or a team admin can change it")]
    Forbidden,
    /// The request can't be used as given.
    #[error("{0}")]
    Invalid(String),
    /// The document already has a listing.
    #[error("this is already published; publish a new version of its listing")]
    AlreadyPublished,
    /// The page is the same as the newest version.
    #[error("nothing changed since its newest version")]
    Unchanged,
    /// The routine's variables left to each person need their fills.
    #[error("choose what to do with {} of this routine's references", .0.len())]
    MissingFills(Vec<TemplateVariable>),
    /// Another request is installing this routine for the same person.
    #[error("this routine is already being installed; try again in a moment")]
    InstallInProgress,
    /// Publishing to everyone, or reviewing, needs Macro's publisher team.
    #[error("only Macro's publishers can publish to everyone or review what is published")]
    NotGlobalPublisher,
    /// A publisher can't review their own version.
    #[error("another publisher must review a version you published")]
    OwnVersion,
    /// A version of the listing is still waiting for review.
    #[error("a version of this listing is waiting for review")]
    ReviewPending,
    /// The routine can't be saved as given, such as an app its person
    /// hasn't connected.
    #[error("{0}")]
    RoutineRejected(String),
    /// The page has problems its author must fix first.
    #[error("this page doesn't build, so it can't be published")]
    DoesNotBuild,
    /// Reading, building or running the page failed.
    #[error(transparent)]
    Page(#[from] DocumentProgramError),
    /// Anything else.
    #[error("internal marketplace error: {0:?}")]
    Internal(rootcause::Report),
}

impl From<rootcause::Report> for MarketplaceError {
    fn from(report: rootcause::Report) -> Self {
        Self::Internal(report)
    }
}
