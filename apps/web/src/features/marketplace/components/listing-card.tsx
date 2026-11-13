import { t } from '@app/lib/i18n';
import DotsIcon from '@icon/system/regular/dots-three.svg';
import { Badge, Button, Card, Dropdown } from '@ui';
import { Show } from 'solid-js';
import {
  isInstalled,
  type Listing,
  updateAvailable,
  versionLabel,
} from '../core/listing';

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
}) {
  const installed = () => isInstalled(props.listing);
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
                {t('marketplace.updateTo', {
                  major: props.listing.latestVersion.major,
                })}
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
          <Show when={installed() || unlistable()}>
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
          </Show>
        </Card.Actions>
      </Card.Footer>
    </Card>
  );
}
