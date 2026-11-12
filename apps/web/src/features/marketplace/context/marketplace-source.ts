import type { TraceContext } from '@macro-inc/observability';
import type { Accessor } from 'solid-js';
import type { Listing } from '../core/listing';

/** The listings the marketplace page shows, and what a person does to them. */
export type MarketplaceSource = {
  /** Every listing the viewer may see, newest first. */
  listings: Accessor<readonly Listing[]>;
  /** The listings are still loading for the first time. */
  loading: Accessor<boolean>;
  failed: Accessor<boolean>;
  retry(): Promise<void>;
  /**
   * Install a listing, or move the viewer's install to its newest major.
   * Each action sends in `parentSpan`, the span of the click that started it.
   */
  install(listingId: string, parentSpan?: TraceContext): Promise<Listing>;
  uninstall(listingId: string, parentSpan?: TraceContext): Promise<void>;
  /** Hide a listing from Browse; people who installed it keep it. */
  unlist(listingId: string, parentSpan?: TraceContext): Promise<void>;
};
