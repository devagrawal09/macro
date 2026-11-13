//! Domain models for the marketplace.

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

/// A listing's version. An install follows minor versions by itself and
/// takes a major version only when its person asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
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
    /// Its newest version.
    pub latest_version: Version,
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
    /// The team that sees it.
    pub team_id: Uuid,
    /// The document or routine it is published from.
    pub source_id: Uuid,
    /// Its title and description.
    pub text: ListingText,
    /// Its first version's content, which decides its kind.
    pub content: ListingContent,
}

/// What became of a new version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddedVersion {
    /// It was stored as this version.
    Added(Version),
    /// The newest version already holds this source.
    Unchanged,
    /// The listing is gone or unlisted.
    Missing,
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
