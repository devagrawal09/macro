import { SettingsPage } from '@app/features/settings/primitives';
import { t } from '@app/lib/i18n';
import { useUserId } from '@core/context/user';
import { idToDisplayName } from '@core/user';
import EmptyStateNoAccessGraphic from '@design/empty-state-no-access.svg';
import { useIsTeamAdmin } from '@queries/team/teams';
import { EmptyStatePanel } from '@ui';
import { Match, Switch } from 'solid-js';
import { ListingsSkeleton } from './components/listing-grid';
import { createMarketplaceSource } from './queries/listings';
import { useMarketplaceFlag } from './use-marketplace-flag';
import { MarketplaceView } from './views/marketplace-view';

function Marketplace() {
  const flag = useMarketplaceFlag();
  const userId = useUserId();
  const isTeamAdmin = useIsTeamAdmin();
  const source = createMarketplaceSource(
    () => flag().enabled && Boolean(userId())
  );

  return (
    <MarketplaceView
      source={source}
      userId={userId()}
      isTeamAdmin={isTeamAdmin}
      publisherName={idToDisplayName}
    />
  );
}

/**
 * Settings → Marketplace: the touch entry to the page the Agents sidebar
 * links on desktop. The tab shows only within the marketplace's rollout, and
 * an installed page opens in place, so nothing here leaves settings.
 */
export function MarketplaceSettings() {
  return (
    <SettingsPage
      title={t('settingsNav.tabMarketplace')}
      bodyClass="[--mobile-content-inset-top:0px]"
    >
      <Marketplace />
    </SettingsPage>
  );
}

/**
 * Agents → Marketplace: the pages the viewer's team published, to install
 * and open. Mounts nothing of the marketplace until its rollout is known,
 * and says it isn't available when a direct visit finds it off.
 */
export function MarketplacePage() {
  const flag = useMarketplaceFlag();

  return (
    <Switch>
      <Match when={flag().loading}>
        <ListingsSkeleton />
      </Match>
      <Match when={flag().enabled}>
        <Marketplace />
      </Match>
      <Match when={true}>
        <EmptyStatePanel
          centered
          topSpacerClass="grow-[0.5] basis-0 mobile:basis-0"
          graphic={EmptyStateNoAccessGraphic}
          title={t('marketplace.unavailable')}
        />
      </Match>
    </Switch>
  );
}
