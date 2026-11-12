import type {
  Listing,
  Version,
} from '@service-agent-harness/generated/schemas';

export type { Listing };

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

/** The listing a person keeps publishing a document's versions to. */
export function ownListingForDocument(
  listings: readonly Listing[],
  documentId: string,
  userId: string | undefined
): Listing | undefined {
  return listings.find(
    (listing) =>
      listing.sourceDocumentId === documentId &&
      isPublisher(listing, userId) &&
      !listing.removed
  );
}

/** The two lists of the marketplace page. */
export type MarketplaceTab = 'browse' | 'installed';

/** Browse lists what can be installed; Installed keeps unlisted installs too. */
export function listingsForTab(
  listings: readonly Listing[],
  tab: MarketplaceTab
): Listing[] {
  if (tab === 'installed') return listings.filter(isInstalled);

  return listings.filter((listing) => !listing.removed);
}
