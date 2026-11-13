//! Ports (trait contracts) for the marketplace.

use agent_code_mode::domain::{
    DocumentBuild, DocumentProgramError, RunProgram, UserExecutionReceipt,
};
use entity_access::domain::models::{
    EntityAccessReceipt, MemberTeamRole, OwnerAccessLevel, ViewAccessLevel,
};
use macro_user_id::user_id::MacroUserIdStr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::models::{
    AddedVersion, InstalledBuild, InstalledPage, Listing, ListingId, ListingText, MarketplaceError,
    NewListing, PublishedPage, PublishedVersion, VersionBump,
};

/// Outbound persistence port for listings, their versions and installs.
pub trait ListingRepo: Send + Sync + 'static {
    /// The error type returned by repository operations.
    type Err: std::error::Error + Send + Sync + 'static;

    /// Store a listing with its first version. `false` when its document
    /// already has a listing, and nothing is stored.
    fn create_listing(
        &self,
        listing: NewListing,
    ) -> impl Future<Output = Result<bool, Self::Err>> + Send;

    /// The listings a member of `team` may see, as `user` sees them, newest
    /// first: their team's and everyone's, leaving out unlisted ones `user`
    /// hasn't installed. Only `only` when given.
    fn listings(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        only: Option<ListingId>,
    ) -> impl Future<Output = Result<Vec<Listing>, Self::Err>> + Send;

    /// Store `page` as the listing's next version, unless its newest version
    /// already holds the same source.
    fn add_version(
        &self,
        id: ListingId,
        bump: VersionBump,
        page: &PublishedPage,
    ) -> impl Future<Output = Result<AddedVersion, Self::Err>> + Send;

    /// Unlist a listing. Repeating it changes nothing.
    fn remove_listing(&self, id: ListingId) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// Put `user` on the listing's newest major version: installs it, or
    /// moves their install.
    fn install_latest(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// Remove `user`'s install. Repeating it changes nothing.
    fn uninstall(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// The page `user`'s install shows, when they installed the listing and a
    /// member of `team` may see it.
    fn installed_page(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
    ) -> impl Future<Output = Result<Option<InstalledPage>, Self::Err>> + Send;

    /// Up to `limit` people who installed the listing and may still see it,
    /// most recently installed first: only those on `major` when given.
    fn installers(
        &self,
        id: ListingId,
        major: Option<i32>,
        limit: u16,
    ) -> impl Future<Output = Result<Vec<MacroUserIdStr<'static>>, Self::Err>> + Send;
}

/// Outbound port that tells people's open clients a version was published.
pub trait VersionNotifier: Send + Sync + 'static {
    /// The error type returned when a notification can't be sent.
    type Err: std::error::Error + Send + Sync + 'static;

    /// Tell `installers` of `version`. Best effort: a person with no open
    /// client hears nothing.
    fn notify(
        &self,
        version: PublishedVersion,
        installers: &[MacroUserIdStr<'static>],
    ) -> impl Future<Output = Result<(), Self::Err>> + Send;
}

/// Outbound port for reading, building and running HTML pages.
pub trait Pages: Send + Sync + 'static {
    /// The stored source of a document the caller may view.
    fn source(
        &self,
        document: &EntityAccessReceipt<ViewAccessLevel>,
    ) -> impl Future<Output = Result<String, DocumentProgramError>> + Send;

    /// The build of `html`.
    fn build(
        &self,
        html: &str,
    ) -> impl Future<Output = Result<DocumentBuild, DocumentProgramError>> + Send;

    /// Run a program of `html`'s build as `user`.
    fn run(
        &self,
        user: &MacroUserIdStr<'static>,
        html: &str,
        request: RunProgram,
        cancel: CancellationToken,
    ) -> impl Future<Output = Result<UserExecutionReceipt, DocumentProgramError>> + Send;
}

/// Outbound port for the marketplace's per-user rollout: a gate, never an
/// authorization.
pub trait Rollout: Send + Sync + 'static {
    /// `false` when the rollout excludes the user or cannot be read.
    fn allows(&self, user: &MacroUserIdStr<'static>) -> impl Future<Output = bool> + Send;
}

/// Inbound service port: the marketplace API used by drivers (HTTP).
///
/// `team` is the caller's team membership, when they have one. Every method
/// answers [`MarketplaceError::NotFound`] to a caller outside the rollout.
pub trait MarketplaceService: Send + Sync + 'static {
    /// The listings the caller may see.
    fn listings(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
    ) -> impl Future<Output = Result<Vec<Listing>, MarketplaceError>> + Send;

    /// Publish a document the caller owns to their team, as version 1.0.
    fn publish(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        document: EntityAccessReceipt<OwnerAccessLevel>,
        text: ListingText,
    ) -> impl Future<Output = Result<Listing, MarketplaceError>> + Send;

    /// Publish the listing's document as its next version. Only its
    /// publisher may. Its installers' open clients are told, best effort.
    fn publish_version(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        document: EntityAccessReceipt<OwnerAccessLevel>,
        bump: VersionBump,
    ) -> impl Future<Output = Result<Listing, MarketplaceError>> + Send;

    /// Unlist a listing. Its publisher or an admin of its team may.
    fn unlist(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<(), MarketplaceError>> + Send;

    /// Put the caller on the listing's newest major version: installs it, or
    /// takes a major version they were waiting on.
    fn install(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<Listing, MarketplaceError>> + Send;

    /// Remove the caller's install.
    fn uninstall(
        &self,
        user: &MacroUserIdStr<'static>,
        id: ListingId,
    ) -> impl Future<Output = Result<(), MarketplaceError>> + Send;

    /// The build of the page the caller's install shows.
    fn installed_build(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<InstalledBuild, MarketplaceError>> + Send;

    /// Run a program of the page the caller's install shows, as the caller.
    fn run_installed(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        request: RunProgram,
        cancel: CancellationToken,
    ) -> impl Future<Output = Result<UserExecutionReceipt, MarketplaceError>> + Send;
}
