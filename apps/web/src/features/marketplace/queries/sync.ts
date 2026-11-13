import { queryClient } from '@queries/client';
import type { InstalledListingBuild } from '@service-agent-harness/generated/schemas';
import { z } from 'zod';
import { marketplaceKeys } from './keys';

/**
 * Sent to a listing's installers when a version is published: the minor's
 * major for a minor version, every installer for a major one. Matches
 * `LISTING_VERSION_MESSAGE_TYPE` in `crates/marketplace`.
 */
export const LISTING_VERSION_MESSAGE_TYPE = 'marketplace_listing_version';

const messageSchema = z.object({
  listingId: z.string().min(1),
  version: z.object({ major: z.number().int(), minor: z.number().int() }),
});

/**
 * A version was published: refresh the listings, so a waiting major (or,
 * for a pinned install, any newer version) shows its Update button, and the
 * open page of that listing when the version is a newer minor of the major
 * it shows and the page isn't pinned. The run's `409` and the window focus
 * refetch stay as fallbacks for a missed message.
 */
export async function handleListingVersion(data: unknown): Promise<void> {
  const parsed = messageSchema.safeParse(data);
  if (!parsed.success) return;
  const { listingId, version } = parsed.data;

  // Every listings request: Browse, Installed, searches and source lookups.
  const listings = queryClient.invalidateQueries({
    queryKey: marketplaceKeys.listings._def,
  });

  // Invalidating refetches the build only where its page is open; a closed
  // page refetches when it opens anyway.
  const buildKey = marketplaceKeys.installedBuild(listingId).queryKey;
  const shown = queryClient.getQueryData<InstalledListingBuild>(buildKey);
  // A pinned page keeps its version: only its lists learn of the new one.
  const newerMinor =
    shown !== undefined &&
    !shown.pinned &&
    version.major === shown.version.major &&
    version.minor > shown.version.minor;
  if (!newerMinor) {
    await listings;
    return;
  }

  await Promise.all([
    listings,
    queryClient.invalidateQueries({ queryKey: buildKey }),
  ]);
}
