import { thrownResultErrorHasCode, throwOnErr } from '@core/util/result';
import {
  agentHarnessServiceClient,
  type RunDocumentProgram,
  type UserExecutionReceipt,
} from '@service-agent-harness/client';
import { queryOptions } from '@tanstack/solid-query';
import { marketplaceKeys } from './keys';

/** Retries of a build the server failed, with the default backoff. */
const BUILD_RETRIES = 3;

/** Retry only a server failure; a refusal answers the same again. */
function retryBuild(failures: number, error: unknown): boolean {
  return (
    failures < BUILD_RETRIES && thrownResultErrorHasCode(error, 'SERVER_ERROR')
  );
}

/**
 * The build of the page the viewer's install shows: the newest minor version
 * of the major they are on. Always stale, so opening the page or returning
 * to the window picks up a minor version published since.
 */
export function installedBuildQueryOptions(listingId: string) {
  return queryOptions({
    queryKey: marketplaceKeys.installedBuild(listingId).queryKey,
    staleTime: 0,
    refetchOnWindowFocus: true,
    retry: retryBuild,
    queryFn: ({ signal }) =>
      throwOnErr(() =>
        agentHarnessServiceClient.getInstalledListingBuild(listingId, signal)
      ),
  });
}

/**
 * Run one program of the page an install shows, on the server, as the
 * viewer. Not cached: every call is a fresh run whose writes stand.
 */
export async function runInstalledProgram(
  listingId: string,
  request: RunDocumentProgram,
  signal?: AbortSignal
): Promise<UserExecutionReceipt> {
  return throwOnErr(() =>
    agentHarnessServiceClient.runInstalledListingProgram(
      listingId,
      request,
      signal
    )
  );
}
