import { t } from '@app/lib/i18n';
import { toast } from '@core/component/Toast/Toast';
import { fireAndForget } from '@core/util/fireAndForget';
import { catchToResult } from '@core/util/result';
import { Telemetry } from '@macro-inc/observability';
import type {
  ListingSource,
  TemplateVariable,
  VersionBump,
} from '@service-agent-harness/generated/schemas';
import { ActionDialogShell, Button, Dialog, Tabs, TextField } from '@ui';
import { createSignal, Show } from 'solid-js';
import {
  type Listing,
  ownListingForSource,
  versionLabel,
} from '../core/listing';
import {
  type PublishListingErrorCode,
  useListingsQuery,
  usePublishListingMutation,
  usePublishListingVersionMutation,
} from '../queries/listings';
import { useMarketplaceFlag } from '../use-marketplace-flag';

/** What is published: an HTML document, or a routine. */
export type PublishTarget =
  | {
      kind: 'artifact';
      id: string;
      /** The editor has changes whose save hasn't landed. */
      unsaved: boolean;
    }
  | {
      kind: 'routine';
      id: string;
      /** Its references, marked as when sharing it as a template. */
      variables(): Promise<TemplateVariable[]>;
    };

async function listingSource(target: PublishTarget): Promise<ListingSource> {
  if (target.kind === 'artifact')
    return { kind: 'artifact', documentId: target.id };

  return {
    kind: 'routine',
    routineId: target.id,
    variables: await target.variables(),
  };
}

/**
 * Publishing an HTML document or a routine to the marketplace. The first
 * time, it names the listing and publishes version 1.0; after that, it
 * publishes the source as a minor update (every install gets it at once) or
 * a major one (each person chooses when to take it). The server publishes
 * what is saved, so it waits for the editor's changes to save. A routine
 * update that changes what installs rely on is published as a major update
 * whatever was chosen, and the toast says so.
 */
export function PublishListingDialog(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  target: PublishTarget;
  /** Starts the listing's title. */
  name: string;
  userId: string | undefined;
}) {
  const flag = useMarketplaceFlag();
  const listings = useListingsQuery(() => flag().enabled);
  const unsaved = () =>
    props.target.kind === 'artifact' && props.target.unsaved;
  const routine = () => props.target.kind === 'routine';
  // A pending load's data read would suspend the dialog.
  const existing = () =>
    listings.isPending
      ? undefined
      : ownListingForSource(
          listings.data?.listings ?? [],
          props.target.kind,
          props.target.id,
          props.userId
        );
  const publish = usePublishListingMutation();
  const publishVersion = usePublishListingVersionMutation();
  const [title, setTitle] = createSignal(props.name);
  const [description, setDescription] = createSignal('');
  const [bump, setBump] = createSignal<VersionBump>('minor');
  const [refusal, setRefusal] = createSignal<string>();
  const pending = () => publish.isPending || publishVersion.isPending;
  const blocked = () =>
    pending() ||
    unsaved() ||
    listings.isPending ||
    (!existing() && title().trim() === '');

  const close = (open: boolean) => {
    if (pending()) return;
    props.onOpenChange(open);
  };

  /**
   * Publishes in its own span, which the request joins, then closes with a
   * toast, or stays open saying why not.
   */
  const submit = (current: Listing | undefined) =>
    Telemetry.span(
      current ? 'marketplace.publish_version' : 'marketplace.publish',
      async (span) => {
        span.setAttr('marketplace.kind', props.target.kind);
        span.setAttr(
          props.target.kind === 'artifact' ? 'document.id' : 'routine.id',
          props.target.id
        );
        if (current) {
          span.setAttr('marketplace.listing_id', current.id);
          span.setAttr('marketplace.bump', bump());
        }
        setRefusal(undefined);
        const target = props.target;
        const result = await catchToResult(async () => {
          const source = await listingSource(target);
          if (current) {
            const requested = bump();
            const published = await publishVersion.mutateAsync({
              listingId: current.id,
              request: { source, bump: requested },
              parentSpan: span,
            });
            return {
              listing: published.listing,
              forced: published.bump !== requested,
              updated: published.updates.updated,
            };
          }

          const listing = await publish.mutateAsync({
            request: {
              source,
              title: title().trim(),
              description: description().trim(),
            },
            parentSpan: span,
          });
          return { listing, forced: false, updated: 0 };
        });

        if (result.isErr()) {
          span.error('Publishing to the marketplace failed');
          const code = result.error[0]?.code;
          if (code) span.setAttr('marketplace.error_code', code);
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
            setRefusal(
              routine()
                ? t('marketplace.publishRoutineConflict')
                : t('marketplace.publishConflict')
            );
          } else {
            setRefusal(t('marketplace.publishFailed'));
          }
          return;
        }

        const { listing, forced, updated } = result.value;
        span.setAttr('marketplace.listing_id', listing.id);
        span.setAttr('marketplace.forced_major', forced);
        const version = versionLabel(listing.latestVersion);

        if (!current) {
          toast.success(
            t('marketplace.publishedToast', { title: listing.title })
          );
        } else if (forced) {
          toast.success(
            t('marketplace.publishedForcedMajorToast', {
              title: listing.title,
              version,
            }),
            { subtext: t('marketplace.publishedForcedMajorSubtext') }
          );
        } else {
          toast.success(
            t('marketplace.publishedVersionToast', {
              title: listing.title,
              version,
            }),
            updated > 0
              ? {
                  subtext: t('marketplace.publishedRoutineUpdates', {
                    count: updated,
                  }),
                }
              : undefined
          );
        }
        props.onOpenChange(false);
      }
    );

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
                      {routine()
                        ? t('marketplace.publishRoutineDescription')
                        : t('marketplace.publishDescription')}
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
                      {bump() === 'major'
                        ? t('marketplace.publishMajorHint')
                        : routine()
                          ? t('marketplace.publishRoutineMinorHint')
                          : t('marketplace.publishMinorHint')}
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
            <Show when={unsaved() && !pending()}>
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
