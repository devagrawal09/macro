import { ListSkeleton } from '@app/components/view-shell/ListSkeleton';
import { t } from '@app/lib/i18n';
import EmptyStateDocGraphic from '@design/empty-state-doc.svg';
import EmptyStateNoAccessGraphic from '@design/empty-state-no-access.svg';
import { Key } from '@solid-primitives/keyed';
import { Button, Card, EmptyStatePanel } from '@ui';
import { type Accessor, For, type JSX, Match, Show, Switch } from 'solid-js';
import type { Listing } from '../core/listing';
import type { MarketplaceListView } from '../primitives/marketplace-list';

/** The card grid: as many 15rem columns as fit, like the routine templates. */
function CardGrid(props: {
  role?: 'list';
  label?: string;
  children: JSX.Element;
}) {
  return (
    <div
      role={props.role}
      aria-label={props.label}
      class="grid grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-2.5 p-4 mobile:px-3"
    >
      {props.children}
    </div>
  );
}

type EmptyView = Extract<MarketplaceListView, { t: 'empty' }>;

function emptyTitle(empty: EmptyView): string {
  if (empty.search !== '')
    return t('marketplace.searchEmptyTitle', { search: empty.search });

  return empty.tab === 'browse'
    ? t('marketplace.browseEmptyTitle')
    : t('marketplace.installedEmptyTitle');
}

function emptyDescription(empty: EmptyView): string {
  if (empty.search !== '') return t('marketplace.searchEmptyDescription');

  return empty.tab === 'browse'
    ? t('marketplace.browseEmptyDescription')
    : t('marketplace.installedEmptyDescription');
}

/** The grid's shape while its listings, or `count` more of them, load. */
export function ListingsSkeleton(props: { label?: string; count?: number }) {
  return (
    <ListSkeleton.Root label={props.label ?? t('marketplace.loading')}>
      <CardGrid>
        <For each={Array.from({ length: props.count ?? 6 })}>
          {() => (
            <Card>
              <Card.Header class="gap-2.5">
                <ListSkeleton.Bar class="h-4 w-16" />
                <ListSkeleton.Bar class="w-2/3" />
                <ListSkeleton.Bar class="h-2 w-full opacity-60" />
                <ListSkeleton.Bar class="h-2 w-1/3 opacity-60" />
              </Card.Header>
              <Card.Footer>
                <ListSkeleton.Bar class="h-2 w-14 opacity-60" />
                <ListSkeleton.Bar class="h-7 w-16" />
              </Card.Footer>
            </Card>
          )}
        </For>
      </CardGrid>
    </ListSkeleton.Root>
  );
}

/**
 * A marketplace tab: its listings as a grid of cards, or its state. More
 * load as the end of the grid scrolls into view.
 */
export function ListingGrid(props: {
  view: MarketplaceListView;
  /** The tab's name, which labels the grid. */
  label: string;
  hasMore: boolean;
  loadingMore: boolean;
  loadMoreFailed: boolean;
  /** Placed after the last card; loading more starts when it scrolls into view. */
  sentinelRef: (element: HTMLDivElement) => void;
  onRetry: () => void;
  onLoadMore: () => void;
  children: (listing: Accessor<Listing>) => JSX.Element;
}) {
  return (
    <Switch>
      <Match when={props.view.t === 'loading'}>
        <ListingsSkeleton />
      </Match>
      <Match when={props.view.t === 'error'}>
        <EmptyStatePanel
          centered
          topSpacerClass="grow-[0.5] basis-0 mobile:basis-0"
          graphic={EmptyStateNoAccessGraphic}
          title={t('marketplace.loadFailed')}
          primaryAction={{
            label: t('common.retry'),
            onClick: props.onRetry,
          }}
        />
      </Match>
      <Match when={props.view.t === 'empty' && props.view}>
        {(empty) => (
          <EmptyStatePanel
            centered
            topSpacerClass="grow-[0.5] basis-0 mobile:basis-0"
            graphic={EmptyStateDocGraphic}
            title={emptyTitle(empty())}
            description={emptyDescription(empty())}
          />
        )}
      </Match>
      <Match when={props.view.t === 'ready' && props.view}>
        {(ready) => (
          <>
            <CardGrid role="list" label={props.label}>
              {/* Keyed by id, so an install or a refresh keeps each card. */}
              <Key each={ready().listings} by="id">
                {(listing) => (
                  <div role="listitem">{props.children(listing)}</div>
                )}
              </Key>
            </CardGrid>
            <Show
              when={!props.loadMoreFailed}
              fallback={
                <div class="flex items-center justify-center gap-2 p-3 text-xs text-ink-muted">
                  <span>{t('marketplace.loadMoreFailed')}</span>
                  <Button
                    variant="ghost"
                    size="sm"
                    class="touch:min-h-11"
                    onClick={() => props.onLoadMore()}
                  >
                    {t('common.retry')}
                  </Button>
                </div>
              }
            >
              <Show when={props.hasMore}>
                <div ref={props.sentinelRef} class="h-px" />
              </Show>
              <Show when={props.loadingMore}>
                <ListingsSkeleton
                  label={t('marketplace.loadingMore')}
                  count={2}
                />
              </Show>
            </Show>
          </>
        )}
      </Match>
    </Switch>
  );
}
