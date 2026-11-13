import { SearchBar } from '@app/components/view-shell';
import { t } from '@app/lib/i18n';
import { useInfiniteScrollSentinel } from '@app/lib/primitives/infinite-scroll-sentinel';
import {
  beginInteraction,
  TraceFlow,
  usePending,
} from '@app/observability/flow';
import { toast } from '@core/component/Toast/Toast';
import { isTouchDevice } from '@core/mobile/isTouchDevice';
import { debouncedDependent } from '@core/util/debounce';
import { fireAndForget } from '@core/util/fireAndForget';
import { catchToResult } from '@core/util/result';
import { type Span, Telemetry } from '@macro-inc/observability';
import { confirmDialog, Tabs } from '@ui';
import { createSignal, Show } from 'solid-js';
import { ListingCard } from '../components/listing-card';
import { ListingGrid } from '../components/listing-grid';
import type {
  MarketplaceRoutines,
  MarketplaceSource,
} from '../context/marketplace-source';
import {
  isInstalled,
  isPublisher,
  type Listing,
  listingTeamId,
  type MarketplaceTab,
  versionLabel,
} from '../core/listing';
import { MissingRoutineFills } from '../core/routine-fills';
import { createMarketplaceList } from '../primitives/marketplace-list';
import { InstalledPageView } from './installed-page-view';

/** The person closed a routine's fill dialog: nothing happened. */
const CANCELLED = Symbol('cancelled');

/** How long Browse waits for typing to pause before it searches. */
const SEARCH_DEBOUNCE_MS = 300;

/**
 * The marketplace: what the viewer's team published (Browse) and what the
 * viewer installed (Installed), as cards. Opening an installed page shows it
 * in place of the lists; opening an installed routine opens the routine.
 */
export function MarketplaceView(props: {
  source: MarketplaceSource;
  routines: MarketplaceRoutines;
  userId: string | undefined;
  /** Whether the viewer is an admin of that team. */
  isTeamAdmin: (teamId: string) => boolean;
  /** A publisher's name as people read it. */
  publisherName: (publisherId: string) => string;
}) {
  const [tab, setTab] = createSignal<MarketplaceTab>('browse');
  const [searchText, setSearchText] = createSignal('');
  // Browse searches once typing pauses; Installed lists every install.
  const typedSearch = debouncedDependent(
    () => searchText().trim(),
    SEARCH_DEBOUNCE_MS
  );
  const search = () => (tab() === 'browse' ? typedSearch() : '');
  const [openId, setOpenId] = createSignal<string>();
  const [busy, setBusy] = createSignal<ReadonlySet<string>>(new Set());
  const list = props.source.list(() => ({
    installed: tab() === 'installed',
    search: search(),
  }));
  const view = createMarketplaceList(list, tab, search);
  const [sentinel, setSentinel] = createSignal<HTMLDivElement>();
  const loadMore = () => fireAndForget(list.loadMore(), 'marketplace');
  // An install that goes away (uninstalled, or gone with its listing) closes.
  const openListing = () => {
    const id = openId();

    return list
      .listings()
      .find((listing) => listing.id === id && isInstalled(listing));
  };

  useInfiniteScrollSentinel({
    sentinel,
    hasNextPage: () => list.hasMore() && !list.loadMoreFailed(),
    isFetchingNextPage: list.loadingMore,
    fetchNextPage: loadMore,
  });
  usePending('marketplace.listings', () => view().t === 'loading');

  /**
   * Runs one action on a listing in its own span, which its request joins:
   * its card waits, and a toast says how it went.
   */
  const act = (
    name: string,
    listing: Listing,
    action: (span: Span) => Promise<unknown>,
    messages: { done: string; failed: string }
  ) =>
    Telemetry.span(name, async (span) => {
      span.setAttr('marketplace.listing_id', listing.id);
      setBusy((current) => new Set(current).add(listing.id));
      const result = await catchToResult(() => action(span));
      setBusy((current) => {
        const next = new Set(current);
        next.delete(listing.id);
        return next;
      });

      if (result.isOk() && result.value === CANCELLED) return;
      if (result.isErr()) {
        span.error(`${name} failed`);
        toast.failure(messages.failed);
        return;
      }
      toast.success(messages.done);
    });

  /**
   * Install a routine listing, or take its newest major: the server fills it
   * in from what is stored, and asks through a 422 for what it can't, which
   * the person fills in before it is sent again. A first install always asks.
   */
  const installRoutine = async (
    listing: Listing,
    span: Span,
    first: boolean
  ) => {
    if (first) {
      const request = await props.routines.fill({ listing });
      if (!request) return CANCELLED;
      return props.source.install(listing.id, span, request);
    }

    try {
      return await props.source.install(listing.id, span);
    } catch (error) {
      if (!(error instanceof MissingRoutineFills)) throw error;
      span.setAttr('marketplace.missing_fills', error.missing.length);
      const request = await props.routines.fill({
        listing,
        missing: error.missing,
      });
      if (!request) return CANCELLED;
      return props.source.install(listing.id, span, request);
    }
  };

  /**
   * Show an installed listing: its page in place, or the routine it made.
   * A page's flow joins this interaction.
   */
  const open = (listing: Listing) => {
    const routine = listing.installedRoutine;
    if (routine) {
      props.routines.open(routine.routineId);
      return;
    }

    const interaction = beginInteraction('marketplace.open', {
      attributes: { 'marketplace.listing_id': listing.id },
      closeOnJoin: true,
    });
    try {
      setOpenId(listing.id);
    } finally {
      interaction.end();
    }
  };

  const install = (listing: Listing) =>
    act(
      'marketplace.install',
      listing,
      (span) =>
        listing.kind === 'routine'
          ? installRoutine(listing, span, true)
          : props.source.install(listing.id, span),
      {
        done: t('marketplace.installedToast', { title: listing.title }),
        failed: t('marketplace.installFailed', { title: listing.title }),
      }
    );
  /**
   * A routine its person edited is refilled from the new major, replacing
   * their edits: ask first.
   */
  const confirmReplacingEdits = (listing: Listing) =>
    confirmDialog({
      title: t('marketplace.updateEditedTitle', {
        title: listing.title,
        version: versionLabel(listing.latestVersion),
      }),
      body: t('marketplace.updateEditedBody'),
      confirmLabel: t('marketplace.updateTo', {
        major: listing.latestVersion.major,
      }),
      tone: 'danger',
    });

  // Installing again moves the install to the newest major.
  const update = async (listing: Listing) => {
    if (listing.installedRoutine?.detached) {
      const confirmed = await confirmReplacingEdits(listing);
      if (!confirmed) return;
    }

    await act(
      'marketplace.update',
      listing,
      (span) =>
        listing.kind === 'routine'
          ? installRoutine(listing, span, false)
          : props.source.install(listing.id, span),
      {
        done: t('marketplace.updatedToast', {
          title: listing.title,
          version: versionLabel(listing.latestVersion),
        }),
        failed: t('marketplace.updateFailed', { title: listing.title }),
      }
    );
  };
  const uninstall = async (listing: Listing) => {
    // The routine an install made stays the person's own: say so first.
    if (listing.installedRoutine) {
      const confirmed = await confirmDialog({
        title: t('marketplace.uninstallRoutineTitle', { title: listing.title }),
        body: t('marketplace.uninstallRoutineBody'),
        confirmLabel: t('marketplace.uninstall'),
      });
      if (!confirmed) return;
    }

    await act(
      'marketplace.uninstall',
      listing,
      (span) => props.source.uninstall(listing.id, span),
      {
        done: t('marketplace.uninstalledToast', { title: listing.title }),
        failed: t('marketplace.uninstallFailed', { title: listing.title }),
      }
    );
  };
  const unlist = async (listing: Listing) => {
    const confirmed = await confirmDialog({
      title: t('marketplace.unlistConfirmTitle', { title: listing.title }),
      body: t('marketplace.unlistConfirmBody'),
      confirmLabel: t('marketplace.unlist'),
      tone: 'danger',
    });
    if (!confirmed) return;

    await act(
      'marketplace.unlist',
      listing,
      (span) => props.source.unlist(listing.id, span),
      {
        done: t('marketplace.unlistedToast', { title: listing.title }),
        failed: t('marketplace.unlistFailed', { title: listing.title }),
      }
    );
  };
  const canUnlist = (listing: Listing) => {
    if (isPublisher(listing, props.userId)) return true;

    const teamId = listingTeamId(listing);
    return teamId !== undefined && props.isTeamAdmin(teamId);
  };

  return (
    <Show
      when={openListing()}
      fallback={
        <div class="flex size-full min-h-0 min-w-0 flex-col">
          <div class="flex shrink-0 flex-wrap items-center gap-2 px-4 pt-3 mobile:px-3">
            <Tabs
              aria-label={t('marketplace.tabsLabel')}
              size={isTouchDevice() ? 'lg' : undefined}
              options={[
                { value: 'browse' as const, label: t('marketplace.browse') },
                {
                  value: 'installed' as const,
                  label: t('marketplace.installed'),
                },
              ]}
              value={tab()}
              onChange={setTab}
            />
            <Show when={tab() === 'browse'}>
              <SearchBar
                label={t('marketplace.search')}
                placeholder={t('marketplace.search')}
                value={searchText()}
                onValueChange={setSearchText}
                class="max-w-80 min-w-48 flex-1 mobile:max-w-none"
              />
            </Show>
          </div>
          <div class="min-h-0 flex-1 overflow-auto touch:pb-(--mobile-content-inset-bottom)">
            <ListingGrid
              view={view()}
              label={
                tab() === 'browse'
                  ? t('marketplace.browse')
                  : t('marketplace.installed')
              }
              hasMore={list.hasMore()}
              loadingMore={list.loadingMore()}
              loadMoreFailed={list.loadMoreFailed()}
              sentinelRef={setSentinel}
              onRetry={() => fireAndForget(list.retry(), 'marketplace')}
              onLoadMore={loadMore}
            >
              {(listing) => (
                <ListingCard
                  listing={listing()}
                  publisherName={props.publisherName(listing().publisher)}
                  canUnlist={canUnlist(listing())}
                  busy={busy().has(listing().id)}
                  onInstall={() =>
                    fireAndForget(install(listing()), 'marketplace')
                  }
                  onOpen={() => open(listing())}
                  onUpdate={() =>
                    fireAndForget(update(listing()), 'marketplace')
                  }
                  onUninstall={() =>
                    fireAndForget(uninstall(listing()), 'marketplace')
                  }
                  onUnlist={() =>
                    fireAndForget(unlist(listing()), 'marketplace')
                  }
                />
              )}
            </ListingGrid>
          </div>
        </div>
      }
    >
      {(listing) => (
        <TraceFlow
          name="marketplace.installed_page"
          attributes={{ 'marketplace.listing_id': listing().id }}
          key={() => listing().id}
        >
          <InstalledPageView
            listing={listing()}
            busy={busy().has(listing().id)}
            onBack={() => setOpenId(undefined)}
            onUpdate={() => fireAndForget(update(listing()), 'marketplace')}
          />
        </TraceFlow>
      )}
    </Show>
  );
}
