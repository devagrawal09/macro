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
});
