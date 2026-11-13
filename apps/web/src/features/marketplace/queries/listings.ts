import { throwOnErr } from '@core/util/result';
import { Telemetry, type TraceContext } from '@macro-inc/observability';
import { queryClient } from '@queries/client';
import { type MutationCallbacks, withCallbacks } from '@queries/utils';
import {
  agentHarnessServiceClient,
  type PublishListing,
  type PublishListingErrorCode,
  type PublishListingVersion,
} from '@service-agent-harness/client';
import type {
  ListMarketplaceListingsParams,
  MarketplaceListings,
} from '@service-agent-harness/generated/schemas';
import {
  type InfiniteData,
  infiniteQueryOptions,
  useInfiniteQuery,
  useMutation,
} from '@tanstack/solid-query';
import type { Accessor } from 'solid-js';
import type {
  ListingList,
  ListingsFilter,
  MarketplaceSource,
} from '../context/marketplace-source';
import type { Listing } from '../core/listing';
import { marketplaceKeys } from './keys';

export type { PublishListingErrorCode };

/** An action on one listing, sent in the trace of the span that started it. */
export type ListingAction = { listingId: string; parentSpan?: TraceContext };

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

/** Which listings one request reads; every page shares it. */
export type ListingsRequest = Omit<ListMarketplaceListingsParams, 'cursor'>;

/**
 * The listings the viewer may see that `request` picks, a page at a time,
 * newest first: their team's and everyone's. Always stale, so opening the
 * page or returning to the window shows what teammates published since.
 */
export function listingsQueryOptions(
  request: ListingsRequest,
  enabled: boolean
) {
  return infiniteQueryOptions({
    queryKey: marketplaceKeys.listings(request).queryKey,
    enabled,
    staleTime: 0,
    refetchOnWindowFocus: true,
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page: MarketplaceListings) =>
      page.nextCursor ?? undefined,
    queryFn: ({ pageParam, signal }) =>
      throwOnErr(() =>
        agentHarnessServiceClient.listMarketplaceListings(
          { ...request, cursor: pageParam },
          signal
        )
      ),
  });
}

/**
 * The live listing of a document, if it has one: whoever opens the document
 * keeps publishing its versions there.
 */
export function useDocumentListingQuery(
  documentId: Accessor<string>,
  enabled: Accessor<boolean>
) {
  return useInfiniteQuery(() =>
    listingsQueryOptions({ documentId: documentId() }, enabled())
  );
}

function invalidateListings() {
  return queryClient.invalidateQueries({
    queryKey: marketplaceKeys.listings._def,
  });
}

/** Show a listing the server just answered with, ahead of the refetch. */
function setListing(listing: Listing) {
  queryClient.setQueriesData<
    InfiniteData<MarketplaceListings, string | undefined>
  >({ queryKey: marketplaceKeys.listings._def }, (current) => {
    if (!current) return current;

    const pages = current.pages.map((page) => ({
      ...page,
      listings: page.listings.map((item) =>
        item.id === listing.id ? listing : item
      ),
    }));
    return { ...current, pages };
  });
}

/** Installs a listing, or moves the viewer's install to its newest major. */
export function useInstallListingMutation(
  callbacks?: MutationCallbacks<Listing, Error, ListingAction>
) {
  return useMutation(() => ({
    mutationFn: (args: ListingAction) =>
      inTrace(args.parentSpan, () =>
        throwOnErr(() =>
          agentHarnessServiceClient.installListing(args.listingId)
        )
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
          // Its document now has this listing: publishing it again offers
          // an update before the refetch lands.
          queryClient.setQueryData<
            InfiniteData<MarketplaceListings, string | undefined>
          >(
            marketplaceKeys.listings({ documentId: listing.sourceDocumentId })
              .queryKey,
            { pages: [{ listings: [listing] }], pageParams: [undefined] }
          );
          await invalidateListings();
        },
      },
      callbacks
    ),
  }));
}

/** Publishes a listing's page, as it is stored now, as its next version. */
export function usePublishListingVersionMutation(
  callbacks?: MutationCallbacks<Listing, Error, PublishVersionAction>
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

/** One marketplace list over the listings query its filter picks. */
function createListingList(
  filter: Accessor<ListingsFilter>,
  enabled: Accessor<boolean>
): ListingList {
  const query = useInfiniteQuery(() => {
    const { installed, search } = filter();
    return listingsQueryOptions(
      { installed, q: search === '' ? undefined : search },
      enabled()
    );
  });

  return {
    // A pending first page's data read would suspend the page.
    listings: () =>
      query.isPending
        ? []
        : (query.data?.pages.flatMap((page) => page.listings) ?? []),
    loading: () => query.isPending && query.isEnabled,
    failed: () => Boolean(query.error) && !query.isFetchNextPageError,
    hasMore: () => query.hasNextPage,
    loadingMore: () => query.isFetchingNextPage,
    loadMoreFailed: () => query.isFetchNextPageError,
    loadMore: async () => {
      await query.fetchNextPage();
    },
    retry: async () => {
      await query.refetch();
    },
  };
}

/** The marketplace page's listings and actions over the listings queries. */
export function createMarketplaceSource(
  enabled: Accessor<boolean>
): MarketplaceSource {
  const install = useInstallListingMutation();
  const uninstall = useUninstallListingMutation();
  const unlist = useUnlistListingMutation();

  return {
    list: (filter) => createListingList(filter, enabled),
    install: (listingId, parentSpan) =>
      install.mutateAsync({ listingId, parentSpan }),
    uninstall: (listingId, parentSpan) =>
      uninstall.mutateAsync({ listingId, parentSpan }),
    unlist: (listingId, parentSpan) =>
      unlist.mutateAsync({ listingId, parentSpan }),
  };
}
