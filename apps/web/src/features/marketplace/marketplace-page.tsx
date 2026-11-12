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
