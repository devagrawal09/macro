import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  MarketplaceRoutines,
  MarketplaceSource,
} from '../context/marketplace-source';
import type { Listing } from '../core/listing';
import { MissingRoutineFills } from '../core/routine-fills';
import { MarketplaceView } from './marketplace-view';

afterEach(cleanup);

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
});
