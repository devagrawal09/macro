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
    AddedVersion, Audience, Changelog, FollowingInstall, InstallRequest, InstallUpdate,
    InstalledBuild, InstalledPage, Listing, ListingContent, ListingCopy, ListingId, ListingKind,
    ListingPage, ListingQuery, ListingText, ListingVersion, MarketplaceError, NewCopy, NewListing,
    PendingReview, PendingVersion, PublishScope, PublishSource, PublishedVersion, ReviewDecision,
    ReviewOutcome, ReviewPreview, ReviewState, RoutineCopy, RoutineInstall, RoutineUpdate,
    RoutineVersion, TaskHash, Version, VersionBump, VersionNotes, VersionNotice, ViewerListings,
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

    /// The listing, as `user` sees it, when `audience` may see it: it is
    /// their team's or everyone's, listed or installed by `user`, and has an
    /// approved version, or is global and `audience` is a global publisher.
    fn listing(
        &self,
        user: &MacroUserIdStr<'_>,
        audience: Audience,
        id: ListingId,
    ) -> impl Future<Output = Result<Option<Listing>, Self::Err>> + Send;

    /// A page of the listings `audience` may see, as `user` sees them, newest
    /// first: their team's and everyone's, as `query` picks.
    fn listings(
        &self,
        user: &MacroUserIdStr<'_>,
        audience: Audience,
        query: &ListingQuery,
    ) -> impl Future<Output = Result<ListingPage, Self::Err>> + Send;

    /// Store `content` as the listing's next version in `review`, with its
    /// notes, numbered after its newest version not rejected, unless that
    /// version already holds the same content or another version waits for
    /// review.
    fn add_version(
        &self,
        id: ListingId,
        bump: VersionBump,
        content: &ListingContent,
        review: ReviewState,
        notes: &VersionNotes,
    ) -> impl Future<Output = Result<AddedVersion, Self::Err>> + Send;

    /// The listing's versions, newest first, without their content: the
    /// approved ones, and the one waiting for review when `include_pending`.
    /// Rejected versions are never listed.
    fn versions(
        &self,
        id: ListingId,
        include_pending: bool,
    ) -> impl Future<Output = Result<Vec<ListingVersion>, Self::Err>> + Send;

    /// Up to `limit` versions waiting for review, oldest first.
    fn pending_versions(
        &self,
        limit: u16,
    ) -> impl Future<Output = Result<Vec<PendingVersion>, Self::Err>> + Send;

    /// The listing's version waiting for review, with its content.
    fn pending_review(
        &self,
        id: ListingId,
    ) -> impl Future<Output = Result<Option<PendingReview>, Self::Err>> + Send;

    /// Record `reviewer`'s decision on the listing's pending `version`:
    /// `false` when it is no longer pending.
    fn decide_review(
        &self,
        id: ListingId,
        version: Version,
        reviewer: &MacroUserIdStr<'_>,
        decision: &ReviewDecision,
    ) -> impl Future<Output = Result<bool, Self::Err>> + Send;

    /// A routine listing's newest approved version, or the newest of `major`.
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

    /// Record a copy a person made. Recording it again changes nothing.
    fn record_copy(&self, copy: &NewCopy) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// The listing the document or routine `copy_id` was copied from, when
    /// it is a recorded copy of that kind.
    fn copied_from(
        &self,
        kind: ListingKind,
        copy_id: Uuid,
    ) -> impl Future<Output = Result<Option<ListingId>, Self::Err>> + Send;

    /// Unlist a listing. Repeating it changes nothing.
    fn remove_listing(&self, id: ListingId) -> impl Future<Output = Result<(), Self::Err>> + Send;

    /// Put `user` on the listing's newest approved major version: installs
    /// it, or moves their install, unpinning it.
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
    /// member of `team` may see it: the newest approved minor of its major,
    /// or the one it is pinned to.
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

/// Outbound port for documents a person keeps as their own.
pub trait Documents: Send + Sync + 'static {
    /// Create an HTML document named `name`, owned by `user`, holding
    /// `html`. Answers its id.
    fn create_html(
        &self,
        user: &MacroUserIdStr<'static>,
        name: &str,
        html: &str,
    ) -> impl Future<Output = Result<Uuid, MarketplaceError>> + Send;
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
    /// A page of the listings the caller may see, as `query` picks, and
    /// whether they are a global publisher.
    fn listings(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        query: ListingQuery,
    ) -> impl Future<Output = Result<ViewerListings, MarketplaceError>> + Send;

    /// Publish a document or routine the caller owns, as version 1.0 saying
    /// `changelog`: to their team at once, or to everyone once another
    /// member of Macro's publisher team approves it. Only that team's members
    /// publish to everyone.
    fn publish(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        source: PublishSource,
        text: ListingText,
        scope: PublishScope,
        changelog: Changelog,
    ) -> impl Future<Output = Result<Listing, MarketplaceError>> + Send;

    /// Publish the listing's source as its next version. Only its publisher
    /// may, and for a global listing only while they are a global publisher;
    /// a global version then waits for review. Once a version is live, a
    /// minor routine version updates the routines installed from it and its
    /// installers' open clients are told, best effort.
    fn publish_version(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        source: PublishSource,
        bump: VersionBump,
        changelog: Changelog,
    ) -> impl Future<Output = Result<PublishedVersion, MarketplaceError>> + Send;

    /// The listing's versions, newest first, to whoever may see it: the
    /// approved ones, and for Macro's publishers the one waiting for review,
    /// marked as pending.
    fn versions(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<Vec<ListingVersion>, MarketplaceError>> + Send;

    /// A routine listing's newest approved version, for someone deciding to
    /// install it or take its newest major.
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

    /// Make the caller's own copy of what they installed, and record where
    /// it came from. An artifact becomes a new HTML document of the version
    /// their install shows, and the install stays. A routine install is
    /// detached: the routine is theirs and gets no more updates.
    fn copy(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<ListingCopy, MarketplaceError>> + Send;

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

    /// The versions waiting for review. Only global publishers may list them.
    fn pending_versions(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
    ) -> impl Future<Output = Result<Vec<PendingVersion>, MarketplaceError>> + Send;

    /// The listing's pending version as a global publisher reviews it: a
    /// page's build and source, or a routine's snapshot and interface
    /// changes. Nothing in it runs.
    fn review_preview(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> impl Future<Output = Result<ReviewPreview, MarketplaceError>> + Send;

    /// Approve or reject the listing's pending `version`. A global publisher
    /// other than its publisher may. Approving makes it live: a minor
    /// routine version updates installs and its installers are told.
    /// Rejecting a new listing's first version unlists it.
    fn review(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        version: Version,
        decision: ReviewDecision,
    ) -> impl Future<Output = Result<ReviewOutcome, MarketplaceError>> + Send;
}
