import { openAgentsPage } from '@app/features/agents-view/primitives/open-page';
import { variableKey } from '@app/features/routines/core/template';
import { openFillTemplateDialog } from '@app/features/routines/routine-template';
import { SettingsPage } from '@app/features/settings/primitives';
import { t } from '@app/lib/i18n';
import { useSplitLayout } from '@components/app/split-layout/layout';
import { useSettingsState } from '@core/constant/SettingsState';
import { useUserId } from '@core/context/user';
import { isMobile } from '@core/mobile/isMobile';
import { idToDisplayName } from '@core/user';
import EmptyStateNoAccessGraphic from '@design/empty-state-no-access.svg';
import { useIsTeamAdmin } from '@queries/team/teams';
import { EmptyStatePanel } from '@ui';
import { getOwner, Match, Switch } from 'solid-js';
import { ListingsSkeleton } from './components/listing-grid';
import type {
  MarketplaceRoutines,
  RoutineFillRequest,
} from './context/marketplace-source';
import { installFills } from './core/routine-fills';
import {
  createMarketplaceSource,
  fetchListingRoutine,
} from './queries/listings';
import { useMarketplaceFlag } from './use-marketplace-flag';
import { MarketplaceView } from './views/marketplace-view';

/**
 * Routine listings over the routines feature: its template dialog asks for
 * fills, and an installed routine opens in Agents → Routines, after which
 * `onOpened` runs.
 */
function createMarketplaceRoutines(onOpened?: () => void): MarketplaceRoutines {
  const owner = getOwner();
  const layout = useSplitLayout();

  const fill = async ({ listing, missing }: RoutineFillRequest) => {
    const { routine } = await fetchListingRoutine(listing.id);
    const only = missing && new Set(missing.map(variableKey));
    const update = only !== undefined;
    const filled = await openFillTemplateDialog({
      template: routine,
      only,
      labels: {
        title: update
          ? t('marketplace.fillUpdateTitle', { title: listing.title })
          : t('marketplace.fillInstallTitle', { title: listing.title }),
        byline: t('marketplace.fillByline', {
          name: idToDisplayName(listing.publisher),
        }),
        action: update
          ? t('marketplace.fillUpdate')
          : t('marketplace.fillInstall'),
        pending: update
          ? t('marketplace.fillUpdating')
          : t('marketplace.fillInstalling'),
        footnote: t('marketplace.fillFootnote'),
      },
      owner,
      key: `marketplace-fill:${listing.id}`,
    });
    if (!filled) return undefined;

    // A new major keeps the time zone stored at install.
    return installFills({
      decisions: filled.decisions,
      timezone: update ? undefined : filled.timezone,
    });
  };

  return {
    fill,
    open: (routineId) => {
      openAgentsPage(layout, 'routines', { routineId });
      onOpened?.();
    },
  };
}

function Marketplace(props: { onRoutineOpened?: () => void }) {
  const flag = useMarketplaceFlag();
  const userId = useUserId();
  const isTeamAdmin = useIsTeamAdmin();
  const source = createMarketplaceSource(
    () => flag().enabled && Boolean(userId())
  );
  const routines = createMarketplaceRoutines(props.onRoutineOpened);

  return (
    <MarketplaceView
      source={source}
      routines={routines}
      userId={userId()}
      isTeamAdmin={isTeamAdmin}
      publisherName={idToDisplayName}
    />
  );
}

/**
 * Settings → Marketplace: the touch entry to the page the Agents sidebar
 * links on desktop. The tab shows only within the marketplace's rollout, and
 * an installed page opens in place. An installed routine opens in Agents →
 * Routines, which takes the settings split's place; on a phone settings is
 * a sheet over it, so it closes.
 */
export function MarketplaceSettings() {
  const settings = useSettingsState();

  return (
    <SettingsPage
      title={t('settingsNav.tabMarketplace')}
      bodyClass="[--mobile-content-inset-top:0px]"
    >
      <Marketplace
        onRoutineOpened={() => {
          if (isMobile()) settings.closeSettings();
        }}
      />
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
