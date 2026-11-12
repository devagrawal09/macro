import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { MarketplaceSource } from '../context/marketplace-source';
import type { Listing } from '../core/listing';
import { MarketplaceView } from './marketplace-view';

afterEach(cleanup);

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

function fakeSource(
  listings: () => Listing[],
  overrides: Partial<MarketplaceSource> = {}
): MarketplaceSource {
  return {
    listings,
    loading: () => false,
    failed: () => false,
    retry: vi.fn(async () => {}),
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
    expect(source.install).toHaveBeenCalledWith('dashboard');
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
    expect(source.install).toHaveBeenCalledWith('dashboard');
  });

  it('lists only installed pages under Installed', () => {
    const source = fakeSource(() => [
      listing({ id: 'dashboard' }),
      listing({ id: 'notes', title: 'Release notes', installedMajor: 1 }),
    ]);
    renderMarketplace(source);

    fireEvent.click(screen.getByRole('radio', { name: 'Installed' }));
    const cards = within(
      screen.getByRole('list', { name: 'Installed' })
    ).getAllByRole('article');
    expect(cards).toHaveLength(1);
    expect(cards[0].textContent).toContain('Release notes');
  });
});
