import { t } from '@app/lib/i18n';
import { usePending } from '@app/observability/flow';
import { toast } from '@core/component/Toast/Toast';
import { fireAndForget } from '@core/util/fireAndForget';
import { catchToResult } from '@core/util/result';
import { confirmDialog, Tabs } from '@ui';
import { createSignal, Show } from 'solid-js';
import { ListingCard } from '../components/listing-card';
import { ListingGrid } from '../components/listing-grid';
import type { MarketplaceSource } from '../context/marketplace-source';
import {
  isInstalled,
  isPublisher,
  type Listing,
  listingTeamId,
  type MarketplaceTab,
  versionLabel,
} from '../core/listing';
import { createMarketplaceList } from '../primitives/marketplace-list';
import { InstalledPageView } from './installed-page-view';

/**
 * The marketplace: what the viewer's team published (Browse) and what the
 * viewer installed (Installed), as cards. Opening an installed listing shows
 * its page in place of the lists.
 */
export function MarketplaceView(props: {
  source: MarketplaceSource;
  userId: string | undefined;
  /** Whether the viewer is an admin of that team. */
  isTeamAdmin: (teamId: string) => boolean;
  /** A publisher's name as people read it. */
  publisherName: (publisherId: string) => string;
}) {
  const [tab, setTab] = createSignal<MarketplaceTab>('browse');
  const [openId, setOpenId] = createSignal<string>();
  const [busy, setBusy] = createSignal<ReadonlySet<string>>(new Set());
  const view = createMarketplaceList(props.source, tab);
  // An install that goes away (uninstalled, or gone with its listing) closes.
  const openListing = () => {
    const id = openId();

    return props.source
      .listings()
      .find((listing) => listing.id === id && isInstalled(listing));
  };
  usePending('marketplace.listings', () => view().t === 'loading');

  /** Runs one action on a listing: its card waits, and a toast says how it went. */
  const act = async (
    listing: Listing,
    action: () => Promise<unknown>,
    messages: { done: string; failed: string }
  ) => {
    setBusy((current) => new Set(current).add(listing.id));
    const result = await catchToResult(action);
    setBusy((current) => {
      const next = new Set(current);
      next.delete(listing.id);
      return next;
    });

    if (result.isErr()) {
      toast.failure(messages.failed);
      return;
    }
    toast.success(messages.done);
  };

  const install = (listing: Listing) =>
    act(listing, () => props.source.install(listing.id), {
      done: t('marketplace.installedToast', { title: listing.title }),
      failed: t('marketplace.installFailed', { title: listing.title }),
    });
  // Installing again moves the install to the newest major.
  const update = (listing: Listing) =>
    act(listing, () => props.source.install(listing.id), {
      done: t('marketplace.updatedToast', {
        title: listing.title,
        version: versionLabel(listing.latestVersion),
      }),
      failed: t('marketplace.updateFailed', { title: listing.title }),
    });
  const uninstall = (listing: Listing) =>
    act(listing, () => props.source.uninstall(listing.id), {
      done: t('marketplace.uninstalledToast', { title: listing.title }),
      failed: t('marketplace.uninstallFailed', { title: listing.title }),
    });
  const unlist = async (listing: Listing) => {
    const confirmed = await confirmDialog({
      title: t('marketplace.unlistConfirmTitle', { title: listing.title }),
      body: t('marketplace.unlistConfirmBody'),
      confirmLabel: t('marketplace.unlist'),
      tone: 'danger',
    });
    if (!confirmed) return;

    await act(listing, () => props.source.unlist(listing.id), {
      done: t('marketplace.unlistedToast', { title: listing.title }),
      failed: t('marketplace.unlistFailed', { title: listing.title }),
    });
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
          <div class="flex shrink-0 items-center px-4 pt-3 mobile:px-3">
            <Tabs
              aria-label={t('marketplace.tabsLabel')}
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
          </div>
          <div class="min-h-0 flex-1 overflow-auto touch:pb-(--mobile-content-inset-bottom)">
            <ListingGrid
              view={view()}
              label={
                tab() === 'browse'
                  ? t('marketplace.browse')
                  : t('marketplace.installed')
              }
              onRetry={() => fireAndForget(props.source.retry(), 'marketplace')}
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
                  onOpen={() => setOpenId(listing().id)}
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
        <InstalledPageView
          listing={listing()}
          busy={busy().has(listing().id)}
          onBack={() => setOpenId(undefined)}
          onUpdate={() => fireAndForget(update(listing()), 'marketplace')}
        />
      )}
    </Show>
  );
}
