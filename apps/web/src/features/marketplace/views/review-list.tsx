import { t } from '@app/lib/i18n';
import { fireAndForget } from '@core/util/fireAndForget';
import EmptyStateDocGraphic from '@design/empty-state-doc.svg';
import EmptyStateNoAccessGraphic from '@design/empty-state-no-access.svg';
import type { PendingVersion } from '@service-agent-harness/generated/schemas';
import { Key } from '@solid-primitives/keyed';
import { Badge, Button, Card, EmptyStatePanel, TextField } from '@ui';
import { createSignal, Match, Show, Switch } from 'solid-js';
import { CardGrid, ListingsSkeleton } from '../components/listing-grid';
import type { PendingList } from '../context/marketplace-source';
import { versionLabel } from '../core/listing';

/**
 * The Review tab: the versions other publishers sent to everyone, each with
 * Preview, Approve and Reject. A publisher's own version can't be approved
 * or rejected by them.
 */
export function ReviewList(props: {
  list: PendingList;
  userId: string | undefined;
  publisherName: (publisherId: string) => string;
  /** Listings with a decision in flight. */
  busy: ReadonlySet<string>;
  onPreview(pending: PendingVersion): void;
  onApprove(pending: PendingVersion): void;
  onReject(pending: PendingVersion, reason: string): void;
}) {
  return (
    <Switch>
      <Match when={props.list.loading()}>
        <ListingsSkeleton label={t('marketplace.reviewLoading')} />
      </Match>
      <Match when={props.list.failed()}>
        <EmptyStatePanel
          centered
          topSpacerClass="grow-[0.5] basis-0 mobile:basis-0"
          graphic={EmptyStateNoAccessGraphic}
          title={t('marketplace.reviewLoadFailed')}
          primaryAction={{
            label: t('common.retry'),
            onClick: () =>
              fireAndForget(props.list.retry(), 'marketplace-review'),
          }}
        />
      </Match>
      <Match when={props.list.versions().length === 0}>
        <EmptyStatePanel
          centered
          topSpacerClass="grow-[0.5] basis-0 mobile:basis-0"
          graphic={EmptyStateDocGraphic}
          title={t('marketplace.reviewEmptyTitle')}
          description={t('marketplace.reviewEmptyDescription')}
        />
      </Match>
      <Match when={true}>
        <CardGrid role="list" label={t('marketplace.review')}>
          <Key each={props.list.versions()} by="listingId">
            {(pending) => (
              <div role="listitem">
                <PendingCard
                  pending={pending()}
                  own={pending().publisher === props.userId}
                  publisherName={props.publisherName(pending().publisher)}
                  busy={props.busy.has(pending().listingId)}
                  onPreview={() => props.onPreview(pending())}
                  onApprove={() => props.onApprove(pending())}
                  onReject={(reason) => props.onReject(pending(), reason)}
                />
              </div>
            )}
          </Key>
        </CardGrid>
      </Match>
    </Switch>
  );
}

/** One pending version, with a reason to fill in before rejecting it. */
function PendingCard(props: {
  pending: PendingVersion;
  /** The viewer published it, so another publisher must decide. */
  own: boolean;
  publisherName: string;
  busy: boolean;
  onPreview(): void;
  onApprove(): void;
  onReject(reason: string): void;
}) {
  const [rejecting, setRejecting] = createSignal(false);
  const [reason, setReason] = createSignal('');
  const approved = () => props.pending.approvedVersion;

  return (
    <Card
      role="article"
      aria-label={t('marketplace.reviewItemLabel', {
        title: props.pending.title,
        version: versionLabel(props.pending.version),
      })}
      class="h-full"
    >
      <Card.Header class="flex-1">
        <Card.Metadata class="gap-x-1.5">
          <Badge variant="outline" size="xs">
            {props.pending.kind === 'routine'
              ? t('marketplace.kindRoutine')
              : t('marketplace.kindArtifact')}
          </Badge>
          <Badge variant="outline" size="xs">
            {versionLabel(props.pending.version)}
          </Badge>
          <Badge variant="outline" size="xs">
            {t('marketplace.scopeGlobal')}
          </Badge>
        </Card.Metadata>
        <Card.Title>{props.pending.title}</Card.Title>
        <Show when={props.pending.description}>
          <Card.Description class="line-clamp-3 text-xs leading-4">
            {props.pending.description}
          </Card.Description>
        </Show>
        <span class="truncate text-xs text-ink-extra-muted">
          {t('marketplace.byPublisher', { name: props.publisherName })}
        </span>
        <p class="text-xs text-ink-muted">
          <Show when={approved()} fallback={t('marketplace.reviewNewListing')}>
            {(current) =>
              t('marketplace.reviewUpdateFrom', {
                version: versionLabel(current()),
              })
            }
          </Show>
        </p>
        <Show when={props.own}>
          <p role="status" class="text-xs text-ink-muted">
            {t('marketplace.reviewOwnVersion')}
          </p>
        </Show>
        <Show when={rejecting()}>
          <form
            class="flex flex-col gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              props.onReject(reason().trim());
            }}
          >
            <TextField value={reason()} onChange={setReason}>
              <TextField.Label>
                {t('marketplace.reviewRejectReason')}
              </TextField.Label>
              <TextField.TextArea
                placeholder={t('marketplace.reviewRejectReasonPlaceholder')}
                readOnly={props.busy}
                maxLength={1000}
              />
            </TextField>
            <div class="flex justify-end gap-2">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                class="touch:min-h-11"
                disabled={props.busy}
                onClick={() => setRejecting(false)}
              >
                {t('common.cancel')}
              </Button>
              <Button
                type="submit"
                variant="danger"
                size="sm"
                class="touch:min-h-11"
                disabled={props.busy}
              >
                {t('marketplace.reviewRejectConfirm')}
              </Button>
            </div>
          </form>
        </Show>
      </Card.Header>
      <Card.Footer>
        <Button
          variant="secondary"
          size="sm"
          class="touch:min-h-11"
          onClick={() => props.onPreview()}
        >
          {t('marketplace.reviewPreview')}
        </Button>
        <Card.Actions>
          <Show when={!rejecting()}>
            <Button
              variant="ghost"
              size="sm"
              class="touch:min-h-11"
              disabled={props.busy || props.own}
              onClick={() => setRejecting(true)}
            >
              {t('marketplace.reviewReject')}
            </Button>
          </Show>
          <Button
            variant="primary"
            size="sm"
            class="touch:min-h-11"
            disabled={props.busy || props.own}
            onClick={() => props.onApprove()}
          >
            {t('marketplace.reviewApprove')}
          </Button>
        </Card.Actions>
      </Card.Footer>
    </Card>
  );
}
