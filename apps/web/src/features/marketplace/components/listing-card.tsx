import { t } from '@app/lib/i18n';
import DotsIcon from '@icon/system/regular/dots-three.svg';
import { Badge, Button, Card, Dropdown } from '@ui';
import { Show } from 'solid-js';
import {
  installedRoutineState,
  isInstalled,
  isPinned,
  type Listing,
  newerThanPinned,
  updateAvailable,
  versionLabel,
} from '../core/listing';
import { updateLabel } from './update-changes';

/** One marketplace listing: what it is, who published it, and what to do with it. */
export function ListingCard(props: {
  listing: Listing;
  publisherName: string;
  /** The viewer published it, or is an admin of the team it belongs to. */
  canUnlist: boolean;
  /** An action on this listing is in flight. */
  busy: boolean;
  onInstall(): void;
  onOpen(): void;
  /** Take the major version that is waiting. */
  onUpdate(): void;
  onUninstall(): void;
  onUnlist(): void;
  /** Show its details and version history. */
  onDetails(): void;
  /** Pin the install to the version it has, or unpin it. */
  onPin(pinned: boolean): void;
}) {
  const installed = () => isInstalled(props.listing);
  // A routine its person edited gets no updates to hold back.
  const pinnable = () =>
    installed() &&
    !isPinned(props.listing) &&
    installedRoutineState(props.listing) !== 'detached';
  const unlistable = () => props.canUnlist && !props.listing.removed;

  return (
    <Card
      role="article"
      aria-label={t('marketplace.listingLabel', {
        title: props.listing.title,
      })}
      class="h-full"
    >
      <Card.Header class="flex-1">
        <Card.Metadata class="gap-x-1.5">
          <Badge variant="outline" size="xs">
            {props.listing.kind === 'routine'
              ? t('marketplace.kindRoutine')
              : t('marketplace.kindArtifact')}
          </Badge>
          <Badge variant="outline" size="xs">
            {versionLabel(props.listing.latestVersion)}
          </Badge>
          <Badge variant="outline" size="xs">
            {props.listing.scope.kind === 'team'
              ? t('marketplace.scopeTeam')
              : t('marketplace.scopeGlobal')}
          </Badge>
          <Show when={props.listing.removed}>
            <Badge variant="ghost" size="xs">
              {t('marketplace.unlisted')}
            </Badge>
          </Show>
          <Show when={isPinned(props.listing)}>
            <Badge variant="ghost" size="xs">
              {t('marketplace.pinned')}
            </Badge>
          </Show>
        </Card.Metadata>
        <Card.Title>{props.listing.title}</Card.Title>
        <Show when={props.listing.description}>
          <Card.Description class="line-clamp-3 text-xs leading-4">
            {props.listing.description}
          </Card.Description>
        </Show>
        <span class="truncate text-xs text-ink-extra-muted">
          {t('marketplace.byPublisher', { name: props.publisherName })}
        </span>
        <Show
          when={
            newerThanPinned(props.listing) && props.listing.installedVersion
          }
        >
          {(installed) => (
            <p role="status" class="text-xs text-ink-muted">
              {t('marketplace.pinnedNewer', {
                pinned: versionLabel(installed()),
                version: versionLabel(props.listing.latestVersion),
              })}
            </p>
          )}
        </Show>
        <Show when={installedRoutineState(props.listing) === 'detached'}>
          <p role="status" class="text-xs text-ink-muted">
            {t('marketplace.routineDetached')}
          </p>
        </Show>
        <Show when={installedRoutineState(props.listing) === 'update_failed'}>
          <p role="status" class="text-xs text-ink-muted">
            {t('marketplace.routineUpdateFailed')}
          </p>
        </Show>
      </Card.Header>
      <Card.Footer>
        <span class="text-xs text-ink-muted tabular-nums">
          {t('marketplace.installs', { count: props.listing.installCount })}
        </span>
        <Card.Actions>
          <Show
            when={installed()}
            fallback={
              <Button
                variant="primary"
                size="sm"
                class="touch:min-h-11"
                disabled={props.busy}
                onClick={() => props.onInstall()}
              >
                {t('marketplace.install')}
              </Button>
            }
          >
            <Show when={updateAvailable(props.listing)}>
              <Button
                variant="ghost"
                size="sm"
                class="touch:min-h-11"
                disabled={props.busy}
                onClick={() => props.onUpdate()}
              >
                {updateLabel(props.listing)}
              </Button>
            </Show>
            <Button
              variant="secondary"
              size="sm"
              class="touch:min-h-11"
              onClick={() => props.onOpen()}
            >
              {t('marketplace.open')}
            </Button>
          </Show>
          <Dropdown placement="bottom-end">
            <Dropdown.Trigger
              variant="ghost"
              size="icon-sm"
              class="touch:min-h-11 touch:min-w-11"
              disabled={props.busy}
              aria-label={t('marketplace.moreActions', {
                title: props.listing.title,
              })}
            >
              <DotsIcon class="size-4" />
            </Dropdown.Trigger>
            <Dropdown.Content class="min-w-36 max-w-64">
              <Dropdown.Group>
                <Dropdown.Item
                  class="touch:min-h-11"
                  onSelect={() => props.onDetails()}
                >
                  {t('marketplace.versionHistory')}
                </Dropdown.Item>
                <Show when={pinnable() && props.listing.installedVersion}>
                  {(version) => (
                    <Dropdown.Item
                      class="touch:min-h-11"
                      onSelect={() => props.onPin(true)}
                    >
                      {t('marketplace.pinTo', {
                        version: versionLabel(version()),
                      })}
                    </Dropdown.Item>
                  )}
                </Show>
                <Show when={isPinned(props.listing)}>
                  <Dropdown.Item
                    class="touch:min-h-11"
                    onSelect={() => props.onPin(false)}
                  >
                    {t('marketplace.unpin')}
                  </Dropdown.Item>
                </Show>
                <Show when={installed()}>
                  <Dropdown.Item
                    class="touch:min-h-11"
                    onSelect={() => props.onUninstall()}
                  >
                    {t('marketplace.uninstall')}
                  </Dropdown.Item>
                </Show>
                <Show when={unlistable()}>
                  <Dropdown.Item
                    class="text-failure-ink touch:min-h-11"
                    onSelect={() => props.onUnlist()}
                  >
                    {t('marketplace.unlist')}
                  </Dropdown.Item>
                </Show>
              </Dropdown.Group>
            </Dropdown.Content>
          </Dropdown>
        </Card.Actions>
      </Card.Footer>
    </Card>
  );
}
