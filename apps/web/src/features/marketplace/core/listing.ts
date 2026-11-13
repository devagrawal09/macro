import type {
  Listing,
  ListingKind,
  Version,
} from '@service-agent-harness/generated/schemas';

export type { Listing, ListingKind };

/** A version as people read it: `v1.2`. */
export function versionLabel(version: Version): string {
  return `v${version.major}.${version.minor}`;
}

/** Whether the viewer installed the listing. */
export function isInstalled(listing: Listing): boolean {
  return typeof listing.installedMajor === 'number';
}

/**
 * Whether a major version newer than the viewer's install is waiting. Minor
 * versions never wait: every install on a major shows its newest minor.
 */
export function updateAvailable(listing: Listing): boolean {
  return (
    typeof listing.installedMajor === 'number' &&
    listing.installedMajor < listing.latestVersion.major
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

/** The two lists of the marketplace page. */
export type MarketplaceTab = 'browse' | 'installed';
