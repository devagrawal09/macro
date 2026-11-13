//! Domain models for the marketplace.

#[cfg(test)]
mod test;

use std::fmt;

use agent_code_mode::domain::{DocumentBuild, DocumentProgramError, SourceHash};
use chrono::{DateTime, Utc};
use macro_user_id::user_id::MacroUserIdStr;
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
    /// The document its versions are published from.
    pub source_document_id: Uuid,
    /// Its name.
    pub title: String,
    /// What it does, in a few lines.
    pub description: String,
    /// Its newest version.
    pub latest_version: Version,
    /// The major version the person asking is on, when they installed it.
    pub installed_major: Option<i32>,
    /// How many people installed it.
    pub install_count: i64,
    /// Unlisted: only those who installed it still see it.
    pub removed: bool,
    /// When it was first published.
    pub created_at: DateTime<Utc>,
    /// When it last changed.
    pub updated_at: DateTime<Utc>,
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
    /// Only listings published from this document.
    pub source_document_id: Option<Uuid>,
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
        source_document_id: Option<Uuid>,
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
            source_document_id,
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

/// A listing to store, with its first version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewListing {
    /// Its id.
    pub id: ListingId,
    /// What it publishes.
    pub kind: ListingKind,
    /// Who publishes it.
    pub publisher: MacroUserIdStr<'static>,
    /// The team that sees it.
    pub team_id: Uuid,
    /// The document it is published from.
    pub source_document_id: Uuid,
    /// Its title and description.
    pub text: ListingText,
    /// Its first version's page.
    pub page: PublishedPage,
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
    #[error("this page is already published; publish a new version of its listing")]
    AlreadyPublished,
    /// The page is the same as the newest version.
    #[error("the page hasn't changed since its newest version")]
    Unchanged,
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
