import { throwOnErr } from '@core/util/result';
import { queryClient } from '@queries/client';
import { type MutationCallbacks, withCallbacks } from '@queries/utils';
import {
  agentHarnessServiceClient,
  type PublishListing,
  type PublishListingErrorCode,
  type PublishListingVersion,
} from '@service-agent-harness/client';
import type { MarketplaceListings } from '@service-agent-harness/generated/schemas';
import { queryOptions, useMutation, useQuery } from '@tanstack/solid-query';
import type { Accessor } from 'solid-js';
import type { MarketplaceSource } from '../context/marketplace-source';
import type { Listing } from '../core/listing';
import { marketplaceKeys } from './keys';

export type { PublishListingErrorCode };

/**
 * The listings the viewer may see: their team's and everyone's. Always
 * stale, so opening the page or returning to the window shows what
 * teammates published since.
 */
export function listingsQueryOptions(enabled: boolean) {
  return queryOptions({
    queryKey: marketplaceKeys.listings.queryKey,
    enabled,
    staleTime: 0,
    refetchOnWindowFocus: true,
    queryFn: ({ signal }) =>
      throwOnErr(() =>
        agentHarnessServiceClient.listMarketplaceListings(signal)
      ),
  });
}

export function useListingsQuery(enabled: Accessor<boolean>) {
  return useQuery(() => listingsQueryOptions(enabled()));
}

function invalidateListings() {
  return queryClient.invalidateQueries({
    queryKey: marketplaceKeys.listings.queryKey,
  });
}

/** Show a listing the server just answered with, ahead of the refetch. */
function setListing(listing: Listing) {
  queryClient.setQueryData<MarketplaceListings>(
    marketplaceKeys.listings.queryKey,
    (current) => {
      if (!current) return current;

      const known = current.listings.some((item) => item.id === listing.id);
      const listings = known
        ? current.listings.map((item) =>
            item.id === listing.id ? listing : item
          )
        : [listing, ...current.listings];

      return { ...current, listings };
    }
  );
}

/** Installs a listing, or moves the viewer's install to its newest major. */
export function useInstallListingMutation(
  callbacks?: MutationCallbacks<Listing, Error, { listingId: string }>
) {
  return useMutation(() => ({
    mutationFn: (args: { listingId: string }) =>
      throwOnErr(() =>
        agentHarnessServiceClient.installListing(args.listingId)
      ),
    ...withCallbacks(
      {
        onSuccess: async (listing) => {
          setListing(listing);
          // An open installed page moves to the major just taken.
          await Promise.all([
            invalidateListings(),
            queryClient.invalidateQueries({
              queryKey: marketplaceKeys.installedBuild(listing.id).queryKey,
            }),
          ]);
        },
      },
      callbacks
    ),
  }));
}

export function useUninstallListingMutation(
  callbacks?: MutationCallbacks<void, Error, { listingId: string }>
) {
  return useMutation(() => ({
    mutationFn: (args: { listingId: string }) =>
      throwOnErr(() =>
        agentHarnessServiceClient.uninstallListing(args.listingId)
      ),
    ...withCallbacks(
      {
        onSuccess: async (_result, { listingId }) => {
          queryClient.removeQueries({
            queryKey: marketplaceKeys.installedBuild(listingId).queryKey,
          });
          await invalidateListings();
        },
      },
      callbacks
    ),
  }));
}

export function useUnlistListingMutation(
  callbacks?: MutationCallbacks<void, Error, { listingId: string }>
) {
  return useMutation(() => ({
    mutationFn: (args: { listingId: string }) =>
      throwOnErr(() => agentHarnessServiceClient.unlistListing(args.listingId)),
    ...withCallbacks({ onSuccess: () => invalidateListings() }, callbacks),
  }));
}

/** Publishes a stored HTML document to the viewer's team, as version 1.0. */
export function usePublishListingMutation(
  callbacks?: MutationCallbacks<Listing, Error, PublishListing>
) {
  return useMutation(() => ({
    mutationFn: (request: PublishListing) =>
      throwOnErr(() => agentHarnessServiceClient.publishListing(request)),
    ...withCallbacks(
      {
        onSuccess: async (listing) => {
          setListing(listing);
          await invalidateListings();
        },
      },
      callbacks
    ),
  }));
}

/** Publishes a listing's page, as it is stored now, as its next version. */
export function usePublishListingVersionMutation(
  callbacks?: MutationCallbacks<
    Listing,
    Error,
    { listingId: string; request: PublishListingVersion }
  >
) {
  return useMutation(() => ({
    mutationFn: (args: { listingId: string; request: PublishListingVersion }) =>
      throwOnErr(() =>
        agentHarnessServiceClient.publishListingVersion(
          args.listingId,
          args.request
        )
      ),
    ...withCallbacks(
      {
        onSuccess: async (listing) => {
          setListing(listing);
          // A minor version replaces the page every install shows.
          await Promise.all([
            invalidateListings(),
            queryClient.invalidateQueries({
              queryKey: marketplaceKeys.installedBuild(listing.id).queryKey,
            }),
          ]);
        },
      },
      callbacks
    ),
  }));
}

/** The marketplace page's listings and actions over the listings query. */
export function createMarketplaceSource(
  enabled: Accessor<boolean>
): MarketplaceSource {
  const query = useListingsQuery(enabled);
  const install = useInstallListingMutation();
  const uninstall = useUninstallListingMutation();
  const unlist = useUnlistListingMutation();

  return {
    // A pending first load's data read would suspend the page.
    listings: () => (query.isPending ? [] : (query.data?.listings ?? [])),
    loading: () => query.isPending && query.isEnabled,
    failed: () => query.isError,
    retry: async () => {
      await query.refetch();
    },
    install: (listingId) => install.mutateAsync({ listingId }),
    uninstall: (listingId) => uninstall.mutateAsync({ listingId }),
    unlist: (listingId) => unlist.mutateAsync({ listingId }),
  };
}
