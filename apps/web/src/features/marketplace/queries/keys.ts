import { createQueryKeys } from '@lukemorales/query-key-factory';
import type { ListMarketplaceListingsParams } from '@service-agent-harness/generated/schemas';

export const marketplaceKeys = createQueryKeys('marketplace', {
  /** Every page of one listings request; `_def` covers them all. */
  listings: (params: Omit<ListMarketplaceListingsParams, 'cursor'>) => ({
    queryKey: [params],
  }),
  installedBuild: (listingId: string) => ({
    queryKey: [listingId],
  }),
  routine: (listingId: string) => ({
    queryKey: [listingId],
  }),
  /** The versions waiting for review. */
  pendingVersions: null,
  /** One pending version as a reviewer reads it. */
  review: (listingId: string) => ({
    queryKey: [listingId],
  }),
  /** A listing's version history. */
  versions: (listingId: string) => ({
    queryKey: [listingId],
  }),
});
