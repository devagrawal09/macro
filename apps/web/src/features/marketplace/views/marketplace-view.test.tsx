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
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ListingList,
  ListingsFilter,
  MarketplaceRoutines,
  MarketplaceSource,
} from '../context/marketplace-source';
import type { Listing } from '../core/listing';
import { MissingRoutineFills } from '../core/routine-fills';
import { MarketplaceView } from './marketplace-view';

// jsdom has no scrollTo, which opening and closing a dialog or menu reaches.
beforeEach(() => {
  vi.stubGlobal('scrollTo', vi.fn());
});

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
      ...list,
    }),
    install: vi.fn(async (id: string) => listing({ id, installedMajor: 1 })),
    uninstall: vi.fn(async () => {}),
    unlist: vi.fn(async () => {}),
    versions: vi.fn(async () => []),
    ...overrides,
  };
}

/** A history entry, as the server answers it. */
const entry = (major: number, minor: number, changelog?: string) => ({
  version: { major, minor },
  changelog,
  forcedMajor: false,
  createdAt: '2026-11-01T10:00:00Z',
});

/** Confirms the update dialog that opened, by its title. */
async function confirmUpdate(title: string, action: string) {
  const confirm = await screen.findByRole('dialog', { name: title });
  fireEvent.click(within(confirm).getByRole('button', { name: action }));
  return confirm;
}

function fakeRoutines(
  overrides: Partial<MarketplaceRoutines> = {}
): MarketplaceRoutines {
  return {
    fill: vi.fn(async () => ({ timezone: 'America/New_York', enabled: false })),
    open: vi.fn(),
    ...overrides,
  };
}

function renderMarketplace(
  source: MarketplaceSource,
  routines: MarketplaceRoutines = fakeRoutines()
) {
  render(() => <ImperativeDialogHost />);
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

  it('offers a waiting major version as Update, and takes it by installing again', async () => {
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
    await confirmUpdate('Update “Launch dashboard” to v2.0?', 'Update to v2');
    await vi.waitFor(() =>
      expect(source.install).toHaveBeenCalledWith(
        'dashboard',
        expect.anything()
      )
    );
  });

  it('shows what the versions an update brings changed before taking it', async () => {
    const versions = vi.fn(async () => [
      entry(2, 1, 'Sorts by owner'),
      entry(2, 0, 'Adds a timeline'),
      entry(1, 1, 'Tidier columns'),
      entry(1, 0, 'First cut'),
    ]);
    const source = fakeSource(
      () => [
        listing({
          id: 'dashboard',
          installedMajor: 1,
          installedVersion: { major: 1, minor: 1 },
          latestVersion: { major: 2, minor: 1 },
        }),
      ],
      { versions }
    );
    renderMarketplace(source);

    fireEvent.click(screen.getByRole('button', { name: 'Update to v2' }));
    const confirm = await screen.findByRole('dialog', {
      name: 'Update “Launch dashboard” to v2.1?',
    });
    const changes = within(confirm).getByRole('list', { name: 'What changed' });
    expect(
      within(changes)
        .getAllByRole('listitem')
        .map((item) => item.getAttribute('aria-label'))
    ).toEqual(['v2.1', 'v2.0']);
    expect(within(changes).getByText('Adds a timeline')).toBeTruthy();
    expect(within(confirm).queryByText('Tidier columns')).toBeNull();
    expect(versions).toHaveBeenCalledWith('dashboard');
    expect(source.install).not.toHaveBeenCalled();

    fireEvent.click(
      within(confirm).getByRole('button', { name: 'Update to v2' })
    );
    await vi.waitFor(() => expect(source.install).toHaveBeenCalledTimes(1));
  });

  it('shows a listing’s version history from its More actions', async () => {
    const versions = vi.fn(async () => [
      entry(1, 1, 'Tidier columns'),
      entry(1, 0),
    ]);
    const source = fakeSource(() => [listing({ id: 'dashboard' })], {
      versions,
    });
    renderMarketplace(source);

    await userEvent.click(
      screen.getByRole('button', { name: 'More actions for Launch dashboard' })
    );
    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Version history' })
    );
    const details = await screen.findByRole('dialog', {
      name: 'Launch dashboard',
    });
    const history = await within(details).findByRole('list', {
      name: 'Version history',
    });
    expect(within(history).getByText('Tidier columns')).toBeTruthy();
    expect(
      within(history).getByText('No notes for this version.')
    ).toBeTruthy();
    expect(versions).toHaveBeenCalledWith('dashboard');
  });

  it('opens a card’s More actions menu with a tap', async () => {
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
        expect(
          within(confirm).getByText(/Updating replaces your edits/)
        ).toBeTruthy();
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
      await confirmUpdate('Update “Weekly digest” to v2.0?', 'Update to v2');
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
});
