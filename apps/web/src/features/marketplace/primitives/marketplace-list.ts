import type { Accessor } from 'solid-js';
import type { ListingList } from '../context/marketplace-source';
import type { Listing, MarketplaceTab } from '../core/listing';

export type MarketplaceListView =
  | { t: 'loading' }
  | { t: 'error' }
  | { t: 'empty'; tab: MarketplaceTab; search: string }
  | { t: 'ready'; listings: readonly Listing[] };

/**
 * What a marketplace tab shows. Listings already loaded stay through a
 * failed refresh; an empty tab says so only once the listings loaded, and
 * names the search that found nothing.
 */
export function createMarketplaceList(
  list: ListingList,
  tab: Accessor<MarketplaceTab>,
  search: Accessor<string>
): Accessor<MarketplaceListView> {
  return () => {
    const listings = list.listings();

    if (listings.length > 0) return { t: 'ready', listings };
    if (list.loading()) return { t: 'loading' };
    if (list.failed()) return { t: 'error' };

    return { t: 'empty', tab: tab(), search: search() };
  };
}
