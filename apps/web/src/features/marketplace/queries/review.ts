import { throwOnErr } from '@core/util/result';
import { Telemetry, type TraceContext } from '@macro-inc/observability';
import { queryClient } from '@queries/client';
import { type MutationCallbacks, withCallbacks } from '@queries/utils';
import {
  agentHarnessServiceClient,
  type ReviewVersion,
  type RunDocumentProgram,
  type UserExecutionReceipt,
} from '@service-agent-harness/client';
import type { ReviewOutcome } from '@service-agent-harness/generated/schemas';
import { queryOptions, useMutation, useQuery } from '@tanstack/solid-query';
import type { Accessor } from 'solid-js';
import type { PendingList } from '../context/marketplace-source';
import { marketplaceKeys } from './keys';

/**
 * The versions waiting for review. Always stale, so opening the Review tab
 * or returning to the window shows what other publishers sent since.
 */
export function pendingVersionsQueryOptions(enabled: boolean) {
  return queryOptions({
    queryKey: marketplaceKeys.pendingVersions.queryKey,
    enabled,
    staleTime: 0,
    refetchOnWindowFocus: true,
    queryFn: ({ signal }) =>
      throwOnErr(() => agentHarnessServiceClient.listPendingVersions(signal)),
  });
}

/** The pending versions as the Review tab lists them. */
export function createPendingList(enabled: Accessor<boolean>): PendingList {
  const query = useQuery(() => pendingVersionsQueryOptions(enabled()));

  return {
    // A pending first load's data read would suspend the page.
    versions: () => (query.isPending ? [] : (query.data?.versions ?? [])),
    loading: () => query.isPending && query.isEnabled,
    failed: () => query.isError,
    retry: async () => {
      await query.refetch();
    },
  };
}

/** A listing's pending version as a reviewer reads it. */
export function versionReviewQueryOptions(listingId: string) {
  return queryOptions({
    queryKey: marketplaceKeys.review(listingId).queryKey,
    staleTime: 0,
    queryFn: ({ signal }) =>
      throwOnErr(() =>
        agentHarnessServiceClient.getVersionReview(listingId, signal)
      ),
  });
}

/** A decision on one pending version, sent in the span that started it. */
export type ReviewAction = {
  listingId: string;
  request: ReviewVersion;
  parentSpan?: TraceContext;
};

/**
 * Approve or reject a pending version. Afterwards the review list and every
 * listings list refetch: an approved version is live.
 */
export function useReviewVersionMutation(
  callbacks?: MutationCallbacks<ReviewOutcome, Error, ReviewAction>
) {
  return useMutation(() => ({
    // Sent in the caller's span: a mutation runs after an await, which
    // loses the browser's trace context.
    mutationFn: (args: ReviewAction) =>
      (args.parentSpan ?? Telemetry.captureContext()).run(() =>
        throwOnErr(() =>
          agentHarnessServiceClient.reviewVersion(args.listingId, args.request)
        )
      ),
    ...withCallbacks(
      {
        onSuccess: async (_outcome, { listingId }) => {
          queryClient.removeQueries({
            queryKey: marketplaceKeys.review(listingId).queryKey,
          });
          await Promise.all([
            queryClient.invalidateQueries({
              queryKey: marketplaceKeys.pendingVersions.queryKey,
            }),
            queryClient.invalidateQueries({
              queryKey: marketplaceKeys.listings._def,
            }),
          ]);
        },
      },
      callbacks
    ),
  }));
}

/** Why a program on a page under review didn't run. */
export class ReviewRunRefused extends Error {
  constructor() {
    super('Programs don’t run while a page is being reviewed.');
    this.name = 'ReviewRunRefused';
  }
}

/**
 * The `run` a page under review gets: it refuses every program, so nothing
 * runs on the server while a reviewer looks. No route would run one anyway.
 */
export async function refuseReviewRun(
  _listingId: string,
  _request: RunDocumentProgram,
  _signal?: AbortSignal
): Promise<UserExecutionReceipt> {
  throw new ReviewRunRefused();
}
