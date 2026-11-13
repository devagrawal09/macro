import { t } from '@app/lib/i18n';
import { ActionDialogShell, Button, Dialog } from '@ui';
import { createResource, Match, Show, Switch } from 'solid-js';
import { VersionHistory } from '../components/version-history';
import type { Listing, ListingVersion } from '../core/listing';

/**
 * A listing's details: what it is, who published it, and its version
 * history with each version's changelog, read when it opens.
 */
export function ListingDetailsDialog(props: {
  listing: Listing;
  publisherName: string;
  onOpenChange: (open: boolean) => void;
  versions(listingId: string): Promise<readonly ListingVersion[]>;
}) {
  const [history, { refetch }] = createResource(
    () => props.listing.id,
    (listingId) => props.versions(listingId)
  );

  return (
    <Dialog
      open
      onOpenChange={props.onOpenChange}
      position="center"
      class="w-110"
      visibleScrim
    >
      <ActionDialogShell>
        <ActionDialogShell.Body>
          <ActionDialogShell.Header>
            <ActionDialogShell.Title>
              {props.listing.title}
            </ActionDialogShell.Title>
            <ActionDialogShell.Description as="div">
              <p>
                {t('marketplace.byPublisher', { name: props.publisherName })}
              </p>
              <Show when={props.listing.description}>
                <p class="mt-1 whitespace-pre-line">
                  {props.listing.description}
                </p>
              </Show>
            </ActionDialogShell.Description>
          </ActionDialogShell.Header>
          <section class="flex flex-col gap-2">
            <h3 class="text-xs font-medium text-ink-muted">
              {t('marketplace.historyTitle')}
            </h3>
            <Switch>
              <Match when={history.error}>
                <div role="alert" class="flex items-center gap-2 text-sm">
                  <span class="text-ink-muted">
                    {t('marketplace.historyFailed')}
                  </span>
                  <Button
                    variant="ghost"
                    size="sm"
                    class="touch:min-h-11"
                    onClick={() => void refetch()}
                  >
                    {t('common.retry')}
                  </Button>
                </div>
              </Match>
              <Match when={history.loading}>
                <p role="status" class="text-sm text-ink-muted">
                  {t('marketplace.historyLoading')}
                </p>
              </Match>
              <Match when={history()}>
                {(versions) => (
                  <VersionHistory
                    versions={versions()}
                    label={t('marketplace.historyTitle')}
                  />
                )}
              </Match>
            </Switch>
          </section>
        </ActionDialogShell.Body>
        <ActionDialogShell.Footer>
          <Button
            type="button"
            variant="ghost"
            onClick={() => props.onOpenChange(false)}
          >
            {t('common.close')}
          </Button>
        </ActionDialogShell.Footer>
      </ActionDialogShell>
    </Dialog>
  );
}
