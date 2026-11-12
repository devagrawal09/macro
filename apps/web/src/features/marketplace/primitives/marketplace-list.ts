import type { Accessor } from 'solid-js';
import type { MarketplaceSource } from '../context/marketplace-source';
import {
  type Listing,
  listingsForTab,
  type MarketplaceTab,
} from '../core/listing';

export type MarketplaceListView =
  | { t: 'loading' }
  | { t: 'error' }
  | { t: 'empty'; tab: MarketplaceTab }
  | { t: 'ready'; listings: Listing[] };

/**
 * What a marketplace tab shows. Listings already loaded stay through a
 * failed refresh; an empty tab says so only once the listings loaded.
 */
export function createMarketplaceList(
  source: MarketplaceSource,
  tab: Accessor<MarketplaceTab>
): Accessor<MarketplaceListView> {
  return () => {
    const listings = listingsForTab(source.listings(), tab());

    if (listings.length > 0) return { t: 'ready', listings };
    if (source.loading()) return { t: 'loading' };
    if (source.failed()) return { t: 'error' };

    return { t: 'empty', tab: tab() };
  };
}
