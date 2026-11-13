import { throwOnErr } from '@core/util/result';
import { Telemetry, type TraceContext } from '@macro-inc/observability';
import { queryClient } from '@queries/client';
import { type MutationCallbacks, withCallbacks } from '@queries/utils';
import {
  agentHarnessServiceClient,
  type InstallListing,
  isMissingRoutineFills,
  type PublishListing,
  type PublishListingErrorCode,
  type PublishListingVersion,
} from '@service-agent-harness/client';
import type {
  MarketplaceListings,
  PublishedVersion,
} from '@service-agent-harness/generated/schemas';
import { queryOptions, useMutation, useQuery } from '@tanstack/solid-query';
import type { Accessor } from 'solid-js';
import type { MarketplaceSource } from '../context/marketplace-source';
import type { Listing } from '../core/listing';
import { MissingRoutineFills } from '../core/routine-fills';
import { marketplaceKeys } from './keys';

export type { PublishListingErrorCode };

/** An action on one listing, sent in the trace of the span that started it. */
export type ListingAction = { listingId: string; parentSpan?: TraceContext };

/** An install, with a routine's fills when it is one. */
export type InstallAction = ListingAction & { request?: InstallListing };

/** A first publish, sent in the trace of the span that started it. */
export type PublishAction = {
  request: PublishListing;
  parentSpan?: TraceContext;
};

/** A new version of a listing, sent in the trace of the span that started it. */
export type PublishVersionAction = ListingAction & {
  request: PublishListingVersion;
};

/**
 * Send in the caller's span: a mutation runs after an await, which loses the
 * browser's trace context.
 */
function inTrace<T>(parentSpan: TraceContext | undefined, send: () => T): T {
  return (parentSpan ?? Telemetry.captureContext()).run(send);
}

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

/**
 * Sends an install; a routine that still needs fills fails as
 * `MissingRoutineFills`, naming them.
 */
async function sendInstall(args: InstallAction): Promise<Listing> {
  const result = await inTrace(args.parentSpan, () =>
    agentHarnessServiceClient.installListing(args.listingId, args.request)
  );
  if (result.isOk()) return result.value;

  const missing = result.error.find(isMissingRoutineFills);
  if (missing) throw new MissingRoutineFills(missing.missing);
  return throwOnErr(async () => result);
}

/** Installs a listing, or moves the viewer's install to its newest major. */
export function useInstallListingMutation(
  callbacks?: MutationCallbacks<Listing, Error, InstallAction>
) {
  return useMutation(() => ({
    mutationFn: sendInstall,
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
  callbacks?: MutationCallbacks<void, Error, ListingAction>
) {
  return useMutation(() => ({
    mutationFn: (args: ListingAction) =>
      inTrace(args.parentSpan, () =>
        throwOnErr(() =>
          agentHarnessServiceClient.uninstallListing(args.listingId)
        )
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
  callbacks?: MutationCallbacks<void, Error, ListingAction>
) {
  return useMutation(() => ({
    mutationFn: (args: ListingAction) =>
      inTrace(args.parentSpan, () =>
        throwOnErr(() =>
          agentHarnessServiceClient.unlistListing(args.listingId)
        )
      ),
    ...withCallbacks({ onSuccess: () => invalidateListings() }, callbacks),
  }));
}

/** Publishes a stored HTML document to the viewer's team, as version 1.0. */
export function usePublishListingMutation(
  callbacks?: MutationCallbacks<Listing, Error, PublishAction>
) {
  return useMutation(() => ({
    mutationFn: (args: PublishAction) =>
      inTrace(args.parentSpan, () =>
        throwOnErr(() => agentHarnessServiceClient.publishListing(args.request))
      ),
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

/**
 * A routine listing's newest version, to fill in before installing it.
 * Fetched when asked: the fill dialog opens on a click.
 */
export async function fetchListingRoutine(listingId: string) {
  return queryClient.fetchQuery({
    queryKey: marketplaceKeys.routine(listingId).queryKey,
    staleTime: 0,
    queryFn: ({ signal }) =>
      throwOnErr(() =>
        agentHarnessServiceClient.getMarketplaceListingRoutine(
          listingId,
          signal
        )
      ),
  });
}

/** Publishes a listing's source, as it is stored now, as its next version. */
export function usePublishListingVersionMutation(
  callbacks?: MutationCallbacks<PublishedVersion, Error, PublishVersionAction>
) {
  return useMutation(() => ({
    mutationFn: (args: PublishVersionAction) =>
      inTrace(args.parentSpan, () =>
        throwOnErr(() =>
          agentHarnessServiceClient.publishListingVersion(
            args.listingId,
            args.request
          )
        )
      ),
    ...withCallbacks(
      {
        onSuccess: async ({ listing }) => {
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
    install: (listingId, parentSpan, request) =>
      install.mutateAsync({ listingId, parentSpan, request }),
    uninstall: (listingId, parentSpan) =>
      uninstall.mutateAsync({ listingId, parentSpan }),
    unlist: (listingId, parentSpan) =>
      unlist.mutateAsync({ listingId, parentSpan }),
  };
}
