import { QueryClient, QueryObserver } from '@tanstack/query-core';
import { afterEach, describe, expect, it, vi } from 'vitest';

const client = vi.hoisted(() => ({ current: undefined as unknown }));
vi.mock('@queries/client', () => ({
  get queryClient() {
    return client.current;
  },
}));

import { marketplaceKeys } from './keys';
import { handleListingVersion } from './sync';

const unsubscribes: (() => void)[] = [];

afterEach(() => {
  for (const unsubscribe of unsubscribes.splice(0)) unsubscribe();
});

/** A query client with `listing`'s page open on version 1.0. */
async function openPage(listing: string) {
  const queryClient = new QueryClient();
  client.current = queryClient;
  const fetchBuild = vi.fn(async () => ({
    page: { status: 'plain' },
    version: { major: 1, minor: 0 },
  }));

  const observer = new QueryObserver(queryClient, {
    queryKey: marketplaceKeys.installedBuild(listing).queryKey,
    queryFn: fetchBuild,
    staleTime: Number.POSITIVE_INFINITY,
  });
  unsubscribes.push(observer.subscribe(() => {}));
  await queryClient.fetchQuery({
    queryKey: marketplaceKeys.installedBuild(listing).queryKey,
    queryFn: fetchBuild,
    staleTime: Number.POSITIVE_INFINITY,
  });
  fetchBuild.mockClear();

  return { fetchBuild };
}

describe('a published version message', () => {
  it('refetches the open page of its listing for a newer minor', async () => {
    const { fetchBuild } = await openPage('board');

    await handleListingVersion({
      listingId: 'board',
      version: { major: 1, minor: 1 },
    });

    expect(fetchBuild).toHaveBeenCalledOnce();
  });

  it('leaves the open page of another listing alone', async () => {
    const { fetchBuild } = await openPage('board');

    await handleListingVersion({
      listingId: 'roadmap',
      version: { major: 1, minor: 1 },
    });

    expect(fetchBuild).not.toHaveBeenCalled();
  });

  it('leaves the open page alone for a major version, which waits', async () => {
    const { fetchBuild } = await openPage('board');

    await handleListingVersion({
      listingId: 'board',
      version: { major: 2, minor: 0 },
    });

    expect(fetchBuild).not.toHaveBeenCalled();
  });

  it('refreshes every listings request, so a waiting major shows its Update button', async () => {
    const queryClient = new QueryClient();
    client.current = queryClient;
    const browse = marketplaceKeys.listings({ installed: false }).queryKey;
    const installed = marketplaceKeys.listings({ installed: true }).queryKey;
    for (const key of [browse, installed]) {
      queryClient.setQueryData(key, {
        pages: [{ listings: [] }],
        pageParams: [undefined],
      });
    }

    await handleListingVersion({
      listingId: 'board',
      version: { major: 2, minor: 0 },
    });

    expect(queryClient.getQueryState(browse)?.isInvalidated).toBe(true);
    expect(queryClient.getQueryState(installed)?.isInvalidated).toBe(true);
  });
});
