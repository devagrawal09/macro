import { t } from '@app/lib/i18n';
import { toast } from '@core/component/Toast/Toast';
import { fireAndForget } from '@core/util/fireAndForget';
import { catchToResult } from '@core/util/result';
import type { VersionBump } from '@service-agent-harness/generated/schemas';
import { ActionDialogShell, Button, Dialog, Tabs, TextField } from '@ui';
import { createSignal, Show } from 'solid-js';
import {
  type Listing,
  ownListingForDocument,
  versionLabel,
} from '../core/listing';
import {
  type PublishListingErrorCode,
  useListingsQuery,
  usePublishListingMutation,
  usePublishListingVersionMutation,
} from '../queries/listings';
import { useMarketplaceFlag } from '../use-marketplace-flag';

/**
 * Publishing an HTML document to the marketplace. The first time, it names
 * the listing and publishes version 1.0; after that, it publishes the page
 * as a minor update (every install gets it at once) or a major one (each
 * person chooses when to take it). The server publishes the saved file, so
 * it waits for the editor's changes to save.
 */
export function PublishListingDialog(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  documentId: string;
  /** Starts the listing's title. */
  documentName: string;
  userId: string | undefined;
  /** The editor has changes whose save hasn't landed. */
  unsaved: boolean;
}) {
  const flag = useMarketplaceFlag();
  const listings = useListingsQuery(() => flag().enabled);
  // A pending load's data read would suspend the dialog.
  const existing = () =>
    listings.isPending
      ? undefined
      : ownListingForDocument(
          listings.data?.listings ?? [],
          props.documentId,
          props.userId
        );
  const publish = usePublishListingMutation();
  const publishVersion = usePublishListingVersionMutation();
  const [title, setTitle] = createSignal(props.documentName);
  const [description, setDescription] = createSignal('');
  const [bump, setBump] = createSignal<VersionBump>('minor');
  const [refusal, setRefusal] = createSignal<string>();
  const pending = () => publish.isPending || publishVersion.isPending;
  const blocked = () =>
    pending() ||
    props.unsaved ||
    listings.isPending ||
    (!existing() && title().trim() === '');

  const close = (open: boolean) => {
    if (pending()) return;
    props.onOpenChange(open);
  };

  /** Publishes, then closes with a toast, or stays open saying why not. */
  const submit = async (current: Listing | undefined) => {
    setRefusal(undefined);
    const result = await catchToResult(() =>
      current
        ? publishVersion.mutateAsync({
            listingId: current.id,
            request: { documentId: props.documentId, bump: bump() },
          })
        : publish.mutateAsync({
            documentId: props.documentId,
            title: title().trim(),
            description: description().trim(),
          })
    );

    if (result.isErr()) {
      const refused = (code: PublishListingErrorCode) =>
        result.error.some((error) => error.code === code);

      if (current && refused('PAGE_UNCHANGED')) {
        setRefusal(
          t('marketplace.publishUnchanged', {
            version: versionLabel(current.latestVersion),
          })
        );
      } else if (refused('PAGE_DOES_NOT_BUILD')) {
        setRefusal(t('marketplace.publishDoesNotBuild'));
      } else if (refused('PUBLISH_CONFLICT')) {
        setRefusal(t('marketplace.publishConflict'));
      } else {
        setRefusal(t('marketplace.publishFailed'));
      }
      return;
    }

    const listing = result.value;
    toast.success(
      current
        ? t('marketplace.publishedVersionToast', {
            title: listing.title,
            version: versionLabel(listing.latestVersion),
          })
        : t('marketplace.publishedToast', { title: listing.title })
    );
    props.onOpenChange(false);
  };

  return (
    <Dialog
      open={props.open}
      onOpenChange={close}
      position="center"
      class="w-110"
      visibleScrim
    >
      <ActionDialogShell>
        <form
          class="flex min-h-0 flex-col"
          aria-busy={pending()}
          onSubmit={(event) => {
            event.preventDefault();
            if (blocked()) return;
            fireAndForget(submit(existing()), 'marketplace-publish');
          }}
        >
          <ActionDialogShell.Body>
            <Show
              when={existing()}
              fallback={
                <>
                  <ActionDialogShell.Header>
                    <ActionDialogShell.Title>
                      {t('marketplace.publishTitle')}
                    </ActionDialogShell.Title>
                    <ActionDialogShell.Description>
                      {t('marketplace.publishDescription')}
                    </ActionDialogShell.Description>
                  </ActionDialogShell.Header>
                  <TextField value={title()} onChange={setTitle}>
                    <TextField.Label>
                      {t('marketplace.publishFieldTitle')}
                    </TextField.Label>
                    <TextField.Input readOnly={pending()} />
                  </TextField>
                  <TextField value={description()} onChange={setDescription}>
                    <TextField.Label>
                      {t('marketplace.publishFieldDescription')}
                    </TextField.Label>
                    <TextField.TextArea
                      placeholder={t(
                        'marketplace.publishFieldDescriptionPlaceholder'
                      )}
                      readOnly={pending()}
                    />
                  </TextField>
                </>
              }
            >
              {(current) => (
                <>
                  <ActionDialogShell.Header>
                    <ActionDialogShell.Title>
                      {t('marketplace.publishUpdateTitle')}
                    </ActionDialogShell.Title>
                    <ActionDialogShell.Description>
                      {t('marketplace.publishCurrentVersion', {
                        title: current().title,
                        version: versionLabel(current().latestVersion),
                      })}
                    </ActionDialogShell.Description>
                  </ActionDialogShell.Header>
                  <div class="flex flex-col gap-2">
                    <Tabs
                      aria-label={t('marketplace.publishBumpLabel')}
                      fullWidth
                      disabled={pending()}
                      options={[
                        {
                          value: 'minor' as const,
                          label: t('marketplace.publishMinor'),
                        },
                        {
                          value: 'major' as const,
                          label: t('marketplace.publishMajor'),
                        },
                      ]}
                      value={bump()}
                      onChange={(next) => {
                        setBump(next);
                        setRefusal(undefined);
                      }}
                    />
                    <p class="text-sm text-ink-muted">
                      {bump() === 'minor'
                        ? t('marketplace.publishMinorHint')
                        : t('marketplace.publishMajorHint')}
                    </p>
                  </div>
                </>
              )}
            </Show>
            <Show when={refusal()}>
              {(message) => (
                <p role="alert" class="text-sm text-failure-ink">
                  {message()}
                </p>
              )}
            </Show>
            <Show when={props.unsaved && !pending()}>
              <p role="status" class="text-sm text-ink-muted">
                {t('marketplace.publishWaitingForSave')}
              </p>
            </Show>
          </ActionDialogShell.Body>
          <ActionDialogShell.Footer>
            <Button
              type="button"
              variant="ghost"
              disabled={pending()}
              onClick={() => close(false)}
            >
              {t('common.cancel')}
            </Button>
            <Button type="submit" variant="primary" disabled={blocked()}>
              {existing()
                ? t('marketplace.publishUpdateSubmit')
                : t('marketplace.publishSubmit')}
            </Button>
          </ActionDialogShell.Footer>
        </form>
      </ActionDialogShell>
    </Dialog>
  );
}
