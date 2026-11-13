import { describe, expect, it } from 'vitest';
import {
  isInstalled,
  isPublisher,
  type Listing,
  listingsForTab,
  ownListingForSource,
  updateAvailable,
  versionLabel,
} from './listing';

const listing = (overrides: Partial<Listing> = {}): Listing => ({
  id: 'listing',
  kind: 'artifact',
  scope: { kind: 'team', teamId: 'team' },
  publisher: 'macro|ada@example.com',
  sourceId: 'document',
  title: 'Launch dashboard',
  description: '',
  latestVersion: { major: 1, minor: 0 },
  installCount: 0,
  removed: false,
  createdAt: '2026-11-01T10:00:00Z',
  updatedAt: '2026-11-01T10:00:00Z',
  ...overrides,
});

describe('marketplace listings', () => {
  it('writes a version as people read it', () => {
    expect(versionLabel({ major: 2, minor: 13 })).toBe('v2.13');
  });

  it('counts a listing as installed only on a major the viewer is on', () => {
    expect(isInstalled(listing())).toBe(false);
    expect(isInstalled(listing({ installedMajor: null }))).toBe(false);
    expect(isInstalled(listing({ installedMajor: 1 }))).toBe(true);
  });

  it('offers an update only for a newer major, never a minor', () => {
    const latestVersion = { major: 2, minor: 3 };

    expect(updateAvailable(listing({ latestVersion }))).toBe(false);
    expect(updateAvailable(listing({ latestVersion, installedMajor: 2 }))).toBe(
      false
    );
    expect(updateAvailable(listing({ latestVersion, installedMajor: 1 }))).toBe(
      true
    );
  });

  it('knows a listing’s publisher', () => {
    expect(isPublisher(listing(), 'macro|ada@example.com')).toBe(true);
    expect(isPublisher(listing(), 'macro|grace@example.com')).toBe(false);
    expect(isPublisher(listing(), undefined)).toBe(false);
  });

  it('keeps unlisted listings out of Browse but in Installed', () => {
    const open = listing({ id: 'open' });
    const installed = listing({ id: 'installed', installedMajor: 1 });
    const unlisted = listing({
      id: 'unlisted',
      installedMajor: 1,
      removed: true,
    });
    const listings = [open, installed, unlisted];

    expect(listingsForTab(listings, 'browse')).toEqual([open, installed]);
    expect(listingsForTab(listings, 'installed')).toEqual([
      installed,
      unlisted,
    ]);
  });

  it('finds the live listing a person publishes a document’s versions to', () => {
    const mine = listing({ id: 'mine' });
    const listings = [
      listing({ id: 'other-document', sourceId: 'other' }),
      listing({ id: 'a-routine', kind: 'routine' }),
      listing({ id: 'unlisted', removed: true }),
      listing({ id: 'theirs', publisher: 'macro|grace@example.com' }),
      mine,
    ];

    expect(
      ownListingForSource(
        listings,
        'artifact',
        'document',
        'macro|ada@example.com'
      )
    ).toBe(mine);
    expect(
      ownListingForSource(
        listings,
        'artifact',
        'document',
        'macro|linus@example.com'
      )
    ).toBeUndefined();
  });
});
