import { createQueryKeys } from '@lukemorales/query-key-factory';

export const marketplaceKeys = createQueryKeys('marketplace', {
  listings: null,
  installedBuild: (listingId: string) => ({
    queryKey: [listingId],
  }),
});
