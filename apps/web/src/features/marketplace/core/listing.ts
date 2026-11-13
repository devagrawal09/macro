import type {
  Listing,
  ListingKind,
  ListingVersion,
  Version,
} from '@service-agent-harness/generated/schemas';

export type { Listing, ListingKind, ListingVersion };

/** A version as people read it: `v1.2`. */
export function versionLabel(version: Version): string {
  return `v${version.major}.${version.minor}`;
}

/** Orders versions oldest first: negative when `a` came before `b`. */
export function compareVersions(a: Version, b: Version): number {
  return a.major === b.major ? a.minor - b.minor : a.major - b.major;
}

/**
 * The approved versions after `from` up to and including `to`, newest
 * first: what taking `to` brings someone who has `from`. Everything up to
 * `to` when `from` is unknown. A version waiting for review, which a
 * publisher's history holds, brings nothing until it is approved.
 */
export function versionsBetween(
  versions: readonly ListingVersion[],
  from: Version | undefined,
  to: Version
): ListingVersion[] {
  return versions
    .filter(
      (entry) =>
        entry.review === 'approved' &&
        compareVersions(entry.version, to) <= 0 &&
        (from === undefined || compareVersions(entry.version, from) > 0)
    )
    .sort((a, b) => compareVersions(b.version, a.version));
}

/** Whether the viewer installed the listing. */
export function isInstalled(listing: Listing): boolean {
  return typeof listing.installedMajor === 'number';
}

/** Whether the viewer pinned their install to the version it has. */
export function isPinned(listing: Listing): boolean {
  return isInstalled(listing) && listing.pinned === true;
}

/**
 * A version newer than a pinned install's, which it doesn't get until the
 * viewer updates or unpins. Nothing for an install that isn't pinned.
 */
export function newerThanPinned(listing: Listing): boolean {
  const installed = listing.installedVersion;
  return (
    isPinned(listing) &&
    installed !== null &&
    installed !== undefined &&
    compareVersions(installed, listing.latestVersion) < 0
  );
}

/**
 * Whether a version newer than the viewer's install is waiting for them to
 * take: a newer major, or for a pinned install any newer version. Minor
 * versions don't wait otherwise: every install on a major shows its newest
 * minor.
 */
export function updateAvailable(listing: Listing): boolean {
  return (
    (typeof listing.installedMajor === 'number' &&
      listing.installedMajor < listing.latestVersion.major) ||
    newerThanPinned(listing)
  );
}

export function isPublisher(
  listing: Listing,
  userId: string | undefined
): boolean {
  return userId !== undefined && listing.publisher === userId;
}

/** The team a listing is published to; a global listing has none. */
export function listingTeamId(listing: Listing): string | undefined {
  return listing.scope.kind === 'team' ? listing.scope.teamId : undefined;
}

/** The listing a person keeps publishing a document's or routine's versions to. */
export function ownListingForSource(
  listings: readonly Listing[],
  kind: ListingKind,
  sourceId: string,
  userId: string | undefined
): Listing | undefined {
  return listings.find(
    (listing) =>
      listing.kind === kind &&
      listing.sourceId === sourceId &&
      isPublisher(listing, userId) &&
      !listing.removed
  );
}

/**
 * Where the viewer's routine from a routine listing stands: following its
 * minor versions, paused because they edited it, or behind after an update
 * failed. Nothing for an artifact, or a routine not installed.
 */
export type InstalledRoutineState = 'following' | 'detached' | 'update_failed';

export function installedRoutineState(
  listing: Listing
): InstalledRoutineState | undefined {
  const routine = listing.installedRoutine;
  if (!routine) return undefined;
  if (routine.detached) return 'detached';
  return routine.updateFailed ? 'update_failed' : 'following';
}

/**
 * Whether the listing has a version people may install. A global listing
 * whose first version waits for review, which only Macro's publishers see,
 * has none.
 */
export function hasApprovedVersion(listing: Listing): boolean {
  const pending = listing.pendingVersion;
  return (
    !pending ||
    pending.major !== listing.latestVersion.major ||
    pending.minor !== listing.latestVersion.minor
  );
}

/**
 * The lists of the marketplace page. Review shows only to Macro's
 * publishers.
 */
export type MarketplaceTab = 'browse' | 'installed' | 'review';
