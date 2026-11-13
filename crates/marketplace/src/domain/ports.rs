//! Ports (trait contracts) for the marketplace.

use agent_code_mode::domain::{
    DocumentBuild, DocumentProgramError, RunProgram, UserExecutionReceipt,
};
use entity_access::domain::models::{
    EntityAccessReceipt, MemberTeamRole, OwnerAccessLevel, ViewAccessLevel,
};
use macro_user_id::user_id::MacroUserIdStr;
use scheduled_action::domain::templates::{RoutineSnapshot, TemplateVariable, fill::FilledRoutine};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::models::{
    AddedVersion, Changelog, FollowingInstall, InstallRequest, InstallUpdate, InstalledBuild,
    InstalledPage, Listing, ListingContent, ListingId, ListingPage, ListingQuery, ListingText,
    ListingVersion, MarketplaceError, NewListing, PublishSource, PublishedVersion, RoutineCopy,
    RoutineInstall, RoutineUpdate, RoutineVersion, TaskHash, Version, VersionBump, VersionNotes,
    VersionNotice,
};

/// Outbound persistence port for listings, their versions and installs.
pub trait ListingRepo: Send + Sync + 'static {
    /// The error type returned by repository operations.
    type Err: std::error::Error + Send + Sync + 'static;

    /// Store a listing with its first version. `false` when its source
    /// already has a listing of its kind, and nothing is stored.
    fn create_listing(
        &self,
        listing: NewListing,
    ) -> impl Future<Output = Result<bool, Self::Err>> + Send;

    /// The listing, as `user` sees it, when a member of `team` may see it:
    /// it is their team's or everyone's, and listed or installed by `user`.
    fn listing(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        id: ListingId,
    ) -> impl Future<Output = Result<Option<Listing>, Self::Err>> + Send;

    /// A page of the listings a member of `team` may see, as `user` sees
    /// them, newest first: their team's and everyone's, as `query` picks.
    fn listings(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        query: &ListingQuery,
    ) -> impl Future<Output = Result<ListingPage, Self::Err>> + Send;

    /// Store `content` as the listing's next version, with its notes,
    /// unless its newest version already holds the same content.
    fn add_version(
        &self,
        id: ListingId,
        bump: VersionBump,
        content: &ListingContent,
        notes: &VersionNotes,
    ) -> impl Future<Output = Result<AddedVersion, Self::Err>> + Send;

    /// Every version of the listing, newest first, without its content.
    fn versions(
        &self,
        id: ListingId,
    ) -> impl Future<Output = Result<Vec<ListingVersion>, Self::Err>> + Send;

    /// A routine listing's newest version, or the newest of `major`.
    fn routine_version(
        &self,
        id: ListingId,
        major: Option<i32>,
    ) -> impl Future<Output = Result<Option<RoutineVersion>, Self::Err>> + Send;

    /// `user`'s routine install, followed or detached.
    fn routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> impl Future<Output = Result<Option<RoutineInstall>, Self::Err>> + Send;

    /// Claim `user`'s first install of the routine listing on `major`, so
    /// only one install creates their routine: `true` for the claimer.
    /// `false` while another claim is fresher than `stale_after_secs` or the
    /// install already has a routine.
    fn claim_routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        major: i32,
        stale_after_secs: u32,
    ) -> impl Future<Output = Result<bool, Self::Err>> + Send;

    /// Give up a claim whose routine wasn't created. Leaves an install that
    /// has a routine alone.
    fn release_routine_claim(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// Install the routine listing for `user`, or replace their install,
    /// following updates again, unpinned. Ends a claim.
    fn save_routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        install: &RoutineInstall,
    ) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// Routine installs on `version`'s major, older than it, whose routine
    /// still follows the listing: not detached, not pinned.
    fn installs_behind(
        &self,
        id: ListingId,
        version: Version,
    ) -> impl Future<Output = Result<Vec<FollowingInstall>, Self::Err>> + Send;

    /// Record what became of updating `user`'s install.
    fn record_update(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        update: &InstallUpdate,
    ) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// Pin `user`'s install to the version it has, or unpin it. Pinning a
    /// pinned install keeps its pin. `false` when they have no install.
    fn set_pinned(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        pinned: bool,
    ) -> impl Future<Output = Result<bool, Self::Err>> + Send;

    /// Unlist a listing. Repeating it changes nothing.
    fn remove_listing(&self, id: ListingId) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// Put `user` on the listing's newest major version: installs it, or
    /// moves their install, unpinning it.
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
    /// member of `team` may see it: the newest minor of its major, or the one
    /// it is pinned to.
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
        version: VersionNotice,
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

/// Outbound port for routines: reading one to publish, and saving the copies
/// installs make, each owned by the person who installed it.
pub trait Routines: Send + Sync + 'static {
    /// The routine the receipt names as its owner shares it. Only a routine
    /// a person owns can be published.
    fn snapshot(
        &self,
        routine: &EntityAccessReceipt<OwnerAccessLevel>,
        description: &str,
        variables: Vec<TemplateVariable>,
    ) -> impl Future<Output = Result<RoutineSnapshot, MarketplaceError>> + Send;

    /// Create `user`'s own routine.
    fn create(
        &self,
        user: &MacroUserIdStr<'static>,
        routine: &FilledRoutine,
        enabled: bool,
    ) -> impl Future<Output = Result<RoutineCopy, MarketplaceError>> + Send;

    /// Replace `user`'s routine's name, description, trigger and task. With
    /// `unedited`, only while its task still hashes to it.
    fn update(
        &self,
        user: &MacroUserIdStr<'static>,
        routine_id: Uuid,
        routine: &FilledRoutine,
        unedited: Option<&TaskHash>,
    ) -> impl Future<Output = Result<RoutineUpdate, MarketplaceError>> + Send;
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
    /// A page of the listings the caller may see, as `query` picks.
    fn listings(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        query: ListingQuery,
    ) -> impl Future<Output = Result<ListingPage, MarketplaceError>> + Send;

    /// Publish a document or routine the caller owns to their team, as
    /// version 1.0 saying `changelog`.
    fn publish(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        source: PublishSource,
        text: ListingText,
        changelog: Changelog,
    ) -> impl Future<Output = Result<Listing, MarketplaceError>> + Send;

    /// Publish the listing's source as its next version. Only its publisher
    /// may. A minor routine version updates the routines installed from it.
    /// Its installers' open clients are told, best effort.
    fn publish_version(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        source: PublishSource,
        bump: VersionBump,
        changelog: Changelog,
    ) -> impl Future<Output = Result<PublishedVersion, MarketplaceError>> + Send;

    /// The listing's versions, newest first, to whoever may see it.
    fn versions(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<Vec<ListingVersion>, MarketplaceError>> + Send;

    /// A routine listing's newest version, for someone deciding to install
    /// it or take its newest major.
    fn routine_version(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<RoutineVersion, MarketplaceError>> + Send;

    /// Unlist a listing. Its publisher or an admin of its team may.
    fn unlist(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<(), MarketplaceError>> + Send;

    /// Put the caller on the listing's newest major version: installs it, or
    /// takes a major version they were waiting on. A routine is filled in
    /// as the caller's own from `request` and the fills stored before.
    fn install(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        request: InstallRequest,
    ) -> impl Future<Output = Result<Listing, MarketplaceError>> + Send;

    /// Pin the caller's install to the version it has, so newer minor
    /// versions don't reach it, or unpin it. Unpinning a routine install
    /// brings its routine to the newest minor of its major, unless its person
    /// edited it, which detaches it as an automatic update would.
    fn set_pinned(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        pinned: bool,
    ) -> impl Future<Output = Result<Listing, MarketplaceError>> + Send;

    /// Remove the caller's install. A routine it created stays theirs.
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
