import type { TraceContext } from '@macro-inc/observability';
import type {
  ActionTrigger,
  InstallListing,
  ListingCopy,
  PendingVersion,
  ReviewOutcome,
  ReviewVersion,
  TemplateVariable,
} from '@service-agent-harness/generated/schemas';
import type { Accessor } from 'solid-js';
import type { Listing, ListingVersion } from '../core/listing';

/** Which listings a marketplace list reads. */
export type ListingsFilter = {
  /** The viewer's installs, unlisted ones included; otherwise Browse's. */
  installed: boolean;
  /** Text a title or description holds; empty lists them all. */
  search: string;
};

/** The listings one filter picks, newest first, and their paging. */
export type ListingList = {
  listings: Accessor<readonly Listing[]>;
  /** The first page is still loading. */
  loading: Accessor<boolean>;
  /** The list itself failed; a failed continuation page is `loadMoreFailed`. */
  failed: Accessor<boolean>;
  hasMore: Accessor<boolean>;
  loadingMore: Accessor<boolean>;
  loadMoreFailed: Accessor<boolean>;
  loadMore(): Promise<void>;
  retry(): Promise<void>;
  /**
   * The viewer belongs to Macro's publisher team: they may publish to
   * everyone and review other publishers' versions.
   */
  globalPublisher: Accessor<boolean>;
};

/** The versions waiting for review, as Macro's publishers see them. */
export type PendingList = {
  versions: Accessor<readonly PendingVersion[]>;
  loading: Accessor<boolean>;
  failed: Accessor<boolean>;
  retry(): Promise<void>;
};

/** The listings the marketplace page shows, and what a person does to them. */
export type MarketplaceSource = {
  /**
   * The listings `filter` picks, a page at a time, following it as it
   * changes. Call it once, when the view is set up.
   */
  list(filter: Accessor<ListingsFilter>): ListingList;
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
  /**
   * The versions waiting for review, read while `enabled`. Call it once,
   * when the view is set up.
   */
  pending(enabled: Accessor<boolean>): PendingList;
  /** Approve or reject a listing's pending version. */
  review(
    listingId: string,
    request: ReviewVersion,
    parentSpan?: TraceContext
  ): Promise<ReviewOutcome>;
  /** A listing's versions, newest first, with what each one changed. */
  versions(listingId: string): Promise<readonly ListingVersion[]>;
  /**
   * Pin the viewer's install to the version it has, or unpin it, which
   * moves it to the newest minor of its major.
   */
  pin(
    listingId: string,
    pinned: boolean,
    parentSpan?: TraceContext
  ): Promise<Listing>;
  /**
   * Make the viewer's own copy of what they installed: a new HTML document
   * of the version a page install shows (the install stays), or their
   * routine detached from the listing.
   */
  copy(listingId: string, parentSpan?: TraceContext): Promise<ListingCopy>;
};

/** The host's documents side: opening a document a copy made. */
export type MarketplaceDocuments = {
  open(documentId: string): void;
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
  /** A routine's trigger as people read it, one line per trigger. */
  triggerLines(trigger: ActionTrigger): string[];
};
