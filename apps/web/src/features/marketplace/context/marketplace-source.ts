import type { TraceContext } from '@macro-inc/observability';
import type {
  InstallListing,
  TemplateVariable,
} from '@service-agent-harness/generated/schemas';
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
   * A routine is filled in from `request`; one that needs fills it doesn't
   * have fails with `MissingRoutineFills`.
   */
  install(
    listingId: string,
    parentSpan?: TraceContext,
    request?: InstallListing
  ): Promise<Listing>;
  uninstall(listingId: string, parentSpan?: TraceContext): Promise<void>;
  /** Hide a listing from Browse; people who installed it keep it. */
  unlist(listingId: string, parentSpan?: TraceContext): Promise<void>;
};

/** What the person decided while filling in a routine to install. */
export type RoutineFillRequest = {
  listing: Listing;
  /** Only these variables, when a new major added them; else all of them. */
  missing?: readonly TemplateVariable[];
};

/**
 * The host's routine side: asking the person to fill a routine in, and
 * opening the routine an install made.
 */
export type MarketplaceRoutines = {
  /** The fills to install with, or nothing when they cancel. */
  fill(request: RoutineFillRequest): Promise<InstallListing | undefined>;
  open(routineId: string): void;
};
