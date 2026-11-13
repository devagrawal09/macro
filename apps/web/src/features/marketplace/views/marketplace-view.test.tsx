import type { PendingVersion } from '@service-agent-harness/generated/schemas';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { ImperativeDialogHost } from '@ui';
import { createSignal } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  ListingList,
  ListingsFilter,
  MarketplaceRoutines,
  MarketplaceSource,
  PendingList,
} from '../context/marketplace-source';
import type { Listing } from '../core/listing';
import { MissingRoutineFills } from '../core/routine-fills';
import { MarketplaceView } from './marketplace-view';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const listing = (overrides: Partial<Listing>): Listing => ({
  id: 'listing',
  kind: 'artifact',
  scope: { kind: 'team', teamId: 'team' },
  publisher: 'macro|ada@example.com',
  sourceId: 'document',
  title: 'Launch dashboard',
  description: 'Tracks the launch.',
  latestVersion: { major: 1, minor: 0 },
  installCount: 3,
  removed: false,
  createdAt: '2026-11-01T10:00:00Z',
  updatedAt: '2026-11-01T10:00:00Z',
  ...overrides,
});

/** A source whose lists read `listings` for their filter, a page at once. */
function fakeSource(
  listings: (filter: ListingsFilter) => Listing[],
  overrides: Partial<MarketplaceSource> = {},
  list: Partial<ListingList> = {}
): MarketplaceSource {
  return {
    list: (filter) => ({
      listings: () => listings(filter()),
      loading: () => false,
      failed: () => false,
      hasMore: () => false,
      loadingMore: () => false,
      loadMoreFailed: () => false,
      loadMore: vi.fn(async () => {}),
      retry: vi.fn(async () => {}),
      globalPublisher: () => false,
      ...list,
    }),
    install: vi.fn(async (id: string) => listing({ id, installedMajor: 1 })),
    uninstall: vi.fn(async () => {}),
    unlist: vi.fn(async () => {}),
    pending: () => fakePending([]),
    review: vi.fn<MarketplaceSource['review']>(async (_id, request) => ({
      state: request.decision === 'approve' ? 'approved' : 'rejected',
      updates: { updated: 0, detached: 0, failed: 0 },
    })),
    ...overrides,
  } satisfies MarketplaceSource;
}

function fakePending(versions: PendingVersion[]): PendingList {
  return {
    versions: () => versions,
    loading: () => false,
    failed: () => false,
    retry: vi.fn(async () => {}),
  };
}

const pendingVersion = (
  overrides: Partial<PendingVersion> = {}
): PendingVersion => ({
  listingId: 'board',
  kind: 'artifact',
  title: 'Launch board',
  description: 'Tracks launches.',
  publisher: 'macro|ada@example.com',
  version: { major: 1, minor: 1 },
  approvedVersion: { major: 1, minor: 0 },
  publishedAt: '2026-11-01T10:00:00Z',
  ...overrides,
});

function fakeRoutines(
  overrides: Partial<MarketplaceRoutines> = {}
): MarketplaceRoutines {
  return {
    fill: vi.fn(async () => ({ timezone: 'America/New_York', enabled: false })),
    open: vi.fn(),
    triggerLines: () => [],
    ...overrides,
  };
}

function renderMarketplace(
  source: MarketplaceSource,
  routines: MarketplaceRoutines = fakeRoutines()
) {
  render(() => (
    <MarketplaceView
      source={source}
      routines={routines}
      userId="macro|grace@example.com"
      isTeamAdmin={() => false}
      publisherName={() => 'Ada Lovelace'}
    />
  ));
}

describe('Marketplace', () => {
  it('offers Install for a listing the viewer has not installed, and installs it', async () => {
    const [listings, setListings] = createSignal([
      listing({ id: 'dashboard' }),
    ]);
    const source = fakeSource(listings, {
      install: vi.fn(async (id: string) => {
        const installed = listing({ id, installedMajor: 1 });
        setListings([installed]);
        return installed;
      }),
    });
    renderMarketplace(source);

    const card = screen.getByRole('article', {
      name: 'Marketplace page: Launch dashboard',
    });
    expect(within(card).queryByRole('button', { name: 'Open' })).toBeNull();

    fireEvent.click(within(card).getByRole('button', { name: 'Install' }));
    expect(source.install).toHaveBeenCalledWith('dashboard', expect.anything());
    expect(
      await within(card).findByRole('button', { name: 'Open' })
    ).toBeTruthy();
    expect(within(card).queryByRole('button', { name: 'Install' })).toBeNull();
  });

  it('offers a waiting major version as Update, and takes it by installing again', () => {
    const source = fakeSource(() => [
      listing({
        id: 'dashboard',
        installedMajor: 1,
        latestVersion: { major: 2, minor: 0 },
      }),
      listing({
        id: 'notes',
        title: 'Release notes',
        installedMajor: 1,
        latestVersion: { major: 1, minor: 4 },
      }),
    ]);
    renderMarketplace(source);

    const current = screen.getByRole('article', {
      name: 'Marketplace page: Release notes',
    });
    expect(
      within(current).queryByRole('button', { name: /Update/ })
    ).toBeNull();

    const behind = screen.getByRole('article', {
      name: 'Marketplace page: Launch dashboard',
    });
    fireEvent.click(
      within(behind).getByRole('button', { name: 'Update to v2' })
    );
    expect(source.install).toHaveBeenCalledWith('dashboard', expect.anything());
  });

  it('opens a card’s More actions menu with a tap', async () => {
    // jsdom has no scrollTo, which opening the menu reaches.
    vi.stubGlobal('scrollTo', vi.fn());
    const source = fakeSource(() => [
      listing({ id: 'dashboard', installedMajor: 1 }),
    ]);
    renderMarketplace(source);

    const trigger = screen.getByRole('button', {
      name: 'More actions for Launch dashboard',
    });
    await userEvent.pointer({ keys: '[TouchA]', target: trigger });

    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Uninstall' })
    );
    expect(source.uninstall).toHaveBeenCalledWith(
      'dashboard',
      expect.anything()
    );
  });

  it('lists the viewer’s installs under Installed', () => {
    const dashboard = listing({ id: 'dashboard' });
    const notes = listing({
      id: 'notes',
      title: 'Release notes',
      installedMajor: 1,
    });
    const source = fakeSource((filter) =>
      filter.installed ? [notes] : [dashboard, notes]
    );
    renderMarketplace(source);

    fireEvent.click(screen.getByRole('radio', { name: 'Installed' }));
    const cards = within(
      screen.getByRole('list', { name: 'Installed' })
    ).getAllByRole('article');
    expect(cards).toHaveLength(1);
    expect(cards[0].textContent).toContain('Release notes');
    expect(
      screen.queryByRole('searchbox', { name: 'Search marketplace' })
    ).toBeNull();
  });

  it('searches Browse once typing pauses, and says when nothing matches', () => {
    vi.useFakeTimers();
    try {
      const searches: string[] = [];
      const source = fakeSource((filter) => {
        searches.push(filter.search);
        return [
          listing({ id: 'dashboard' }),
          listing({ id: 'notes', title: 'Release notes' }),
        ].filter((item) =>
          item.title.toLowerCase().includes(filter.search.toLowerCase())
        );
      });
      renderMarketplace(source);

      fireEvent.input(
        screen.getByRole('searchbox', { name: 'Search marketplace' }),
        { target: { value: ' notes ' } }
      );
      expect(screen.getAllByRole('article')).toHaveLength(2);

      vi.advanceTimersByTime(300);
      const cards = screen.getAllByRole('article');
      expect(cards).toHaveLength(1);
      expect(cards[0].textContent).toContain('Release notes');
      expect(searches.at(-1)).toBe('notes');

      fireEvent.input(
        screen.getByRole('searchbox', { name: 'Search marketplace' }),
        { target: { value: 'roadmap' } }
      );
      vi.advanceTimersByTime(300);
      expect(screen.getByText('Nothing matches “roadmap”')).toBeTruthy();
    } finally {
      vi.useRealTimers();
    }
  });

  it('retries loading more after a page failed', () => {
    const loadMore = vi.fn(async () => {});
    const source = fakeSource(
      () => [listing({ id: 'dashboard' })],
      {},
      { loadMoreFailed: () => true, loadMore }
    );
    renderMarketplace(source);

    expect(screen.getByText('Could not load more.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(loadMore).toHaveBeenCalledTimes(1);
  });

  describe('routine listings', () => {
    const digest = (overrides: Partial<Listing> = {}) =>
      listing({
        id: 'digest',
        kind: 'routine',
        title: 'Weekly digest',
        ...overrides,
      });

    it('marks a routine and asks for its fills before installing it', async () => {
      const source = fakeSource(() => [digest()]);
      const routines = fakeRoutines({
        fill: vi.fn(async () => ({
          timezone: 'America/New_York',
          enabled: false,
          variables: { board: 'shared' as const },
        })),
      });
      renderMarketplace(source, routines);

      const card = screen.getByRole('article', {
        name: 'Marketplace page: Weekly digest',
      });
      expect(within(card).getByText('Routine')).toBeTruthy();

      fireEvent.click(within(card).getByRole('button', { name: 'Install' }));
      await vi.waitFor(() =>
        expect(source.install).toHaveBeenCalledWith(
          'digest',
          expect.anything(),
          {
            timezone: 'America/New_York',
            enabled: false,
            variables: { board: 'shared' },
          }
        )
      );
      expect(routines.fill).toHaveBeenCalledWith({
        listing: expect.objectContaining({ id: 'digest' }),
      });
    });

    it('installs nothing when the person closes the fill dialog', async () => {
      const source = fakeSource(() => [digest()]);
      const routines = fakeRoutines({ fill: vi.fn(async () => undefined) });
      renderMarketplace(source, routines);

      fireEvent.click(screen.getByRole('button', { name: 'Install' }));
      await vi.waitFor(() => expect(routines.fill).toHaveBeenCalled());
      expect(source.install).not.toHaveBeenCalled();
    });

    it('opens the routine an install made, and says when updates are paused', () => {
      const source = fakeSource(() => [
        digest({
          installedMajor: 1,
          installedRoutine: {
            routineId: 'routine-1',
            detached: true,
            updateFailed: false,
          },
        }),
      ]);
      const routines = fakeRoutines();
      renderMarketplace(source, routines);

      const card = screen.getByRole('article', {
        name: 'Marketplace page: Weekly digest',
      });
      expect(
        within(card).getByText(
          'You edited this routine, so updates are paused.'
        )
      ).toBeTruthy();

      fireEvent.click(within(card).getByRole('button', { name: 'Open' }));
      expect(routines.open).toHaveBeenCalledWith('routine-1');
    });

    it.each([
      ['Update to v2', 1],
      ['Cancel', 0],
    ] as const)(
      'confirms before replacing an edited routine: %s',
      async (choice, installs) => {
        const source = fakeSource(() => [
          digest({
            installedMajor: 1,
            latestVersion: { major: 2, minor: 0 },
            installedRoutine: {
              routineId: 'routine-1',
              detached: true,
              updateFailed: false,
            },
          }),
        ]);
        render(() => <ImperativeDialogHost />);
        renderMarketplace(source);

        const card = screen.getByRole('article', {
          name: 'Marketplace page: Weekly digest',
        });
        fireEvent.click(
          within(card).getByRole('button', { name: 'Update to v2' })
        );
        const confirm = await screen.findByRole('dialog', {
          name: 'Update “Weekly digest” to v2.0?',
        });
        expect(source.install).not.toHaveBeenCalled();

        fireEvent.click(within(confirm).getByRole('button', { name: choice }));
        await vi.waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
        await vi.waitFor(() =>
          expect(source.install).toHaveBeenCalledTimes(installs)
        );
      }
    );

    it('asks only for what a new major added, then takes it', async () => {
      const board = {
        kind: 'item' as const,
        id: 'board',
        block_name: 'md',
        label: 'Board',
        fill: 'choice' as const,
      };
      const install = vi
        .fn<MarketplaceSource['install']>()
        .mockRejectedValueOnce(new MissingRoutineFills([board]))
        .mockResolvedValueOnce(digest({ installedMajor: 2 }));
      const source = fakeSource(
        () => [
          digest({
            installedMajor: 1,
            latestVersion: { major: 2, minor: 0 },
            installedRoutine: {
              routineId: 'routine-1',
              detached: false,
              updateFailed: false,
            },
          }),
        ],
        { install }
      );
      const routines = fakeRoutines({
        fill: vi.fn(async () => ({
          enabled: false,
          variables: { board: 'choice' as const },
        })),
      });
      renderMarketplace(source, routines);

      fireEvent.click(screen.getByRole('button', { name: 'Update to v2' }));
      await vi.waitFor(() => expect(install).toHaveBeenCalledTimes(2));
      expect(install).toHaveBeenNthCalledWith(1, 'digest', expect.anything());
      expect(routines.fill).toHaveBeenCalledWith({
        listing: expect.objectContaining({ id: 'digest' }),
        missing: [board],
      });
      expect(install).toHaveBeenNthCalledWith(2, 'digest', expect.anything(), {
        enabled: false,
        variables: { board: 'choice' },
      });
    });
  });

  describe('global listings and review', () => {
    it('shows a pending version to a publisher and offers no Install before approval', () => {
      const source = fakeSource(
        () => [
          listing({
            id: 'board',
            scope: { kind: 'global' },
            latestVersion: { major: 1, minor: 0 },
            pendingVersion: { major: 1, minor: 0 },
          }),
        ],
        {},
        { globalPublisher: () => true }
      );
      renderMarketplace(source);

      const card = screen.getByRole('article', {
        name: 'Marketplace page: Launch dashboard',
      });
      expect(within(card).getByText('Macro')).toBeTruthy();
      expect(within(card).getByText('v1.0 in review')).toBeTruthy();
      expect(
        within(card).queryByRole('button', { name: 'Install' })
      ).toBeNull();
    });

    it('offers Review only to Macro’s publishers', () => {
      renderMarketplace(fakeSource(() => []));
      expect(screen.queryByRole('radio', { name: 'Review' })).toBeNull();
      cleanup();

      renderMarketplace(
        fakeSource(() => [], {}, { globalPublisher: () => true })
      );
      expect(screen.getByRole('radio', { name: 'Review' })).toBeTruthy();
    });

    it('approves another publisher’s version, and never one’s own', async () => {
      const source = fakeSource(
        () => [],
        {
          pending: () =>
            fakePending([
              pendingVersion(),
              pendingVersion({
                listingId: 'mine',
                title: 'My digest',
                kind: 'routine',
                publisher: 'macro|grace@example.com',
                version: { major: 1, minor: 0 },
                approvedVersion: undefined,
              }),
            ]),
        },
        { globalPublisher: () => true }
      );
      renderMarketplace(source);

      fireEvent.click(screen.getByRole('radio', { name: 'Review' }));
      const mine = screen.getByRole('article', {
        name: 'Waiting for review: My digest v1.0',
      });
      expect(within(mine).getByText('New listing')).toBeTruthy();
      expect(
        within(mine)
          .getByRole('button', { name: 'Approve' })
          .hasAttribute('disabled')
      ).toBe(true);
      expect(
        within(mine)
          .getByRole('button', { name: 'Reject' })
          .hasAttribute('disabled')
      ).toBe(true);

      const theirs = screen.getByRole('article', {
        name: 'Waiting for review: Launch board v1.1',
      });
      expect(within(theirs).getByText('Update from v1.0')).toBeTruthy();
      fireEvent.click(within(theirs).getByRole('button', { name: 'Approve' }));
      await vi.waitFor(() =>
        expect(source.review).toHaveBeenCalledWith(
          'board',
          {
            version: { major: 1, minor: 1 },
            decision: 'approve',
            reason: undefined,
          },
          expect.anything()
        )
      );
    });

    it('rejects a version with the reason the reviewer gives', async () => {
      const source = fakeSource(
        () => [],
        { pending: () => fakePending([pendingVersion()]) },
        { globalPublisher: () => true }
      );
      renderMarketplace(source);

      fireEvent.click(screen.getByRole('radio', { name: 'Review' }));
      const card = screen.getByRole('article', {
        name: 'Waiting for review: Launch board v1.1',
      });
      fireEvent.click(within(card).getByRole('button', { name: 'Reject' }));
      fireEvent.input(
        within(card).getByRole('textbox', { name: 'Reason (optional)' }),
        { target: { value: '  It shows internal data.  ' } }
      );
      fireEvent.click(within(card).getByRole('button', { name: 'Reject' }));

      await vi.waitFor(() =>
        expect(source.review).toHaveBeenCalledWith(
          'board',
          {
            version: { major: 1, minor: 1 },
            decision: 'reject',
            reason: 'It shows internal data.',
          },
          expect.anything()
        )
      );
    });
  });
});
