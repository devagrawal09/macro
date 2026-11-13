import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { createSignal } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  ListingList,
  ListingsFilter,
  MarketplaceSource,
} from '../context/marketplace-source';
import type { Listing } from '../core/listing';
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
  sourceDocumentId: 'document',
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
    ...overrides,
  };
}

function renderMarketplace(source: MarketplaceSource) {
  render(() => (
    <MarketplaceView
      source={source}
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
      expect(screen.getByText('No pages match “roadmap”')).toBeTruthy();
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

    expect(screen.getByText('Could not load more pages.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(loadMore).toHaveBeenCalledTimes(1);
  });
});
