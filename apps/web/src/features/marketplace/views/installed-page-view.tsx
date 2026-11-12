import { ListSkeleton } from '@app/components/view-shell/ListSkeleton';
import { t } from '@app/lib/i18n';
import {
  BuildError,
  BuildProblems,
  PlainFrame,
  ProgramFrame,
} from '@block-code/component/HtmlPreview';
import { fireAndForget } from '@core/util/fireAndForget';
import { thrownResultErrorHasCode } from '@core/util/result';
import ArrowLeftIcon from '@icon/system/regular/arrow-left.svg';
import { useQuery } from '@tanstack/solid-query';
import { Badge, Button } from '@ui';
import { createMemo, createSignal, Match, Show, Switch } from 'solid-js';
import { type Listing, updateAvailable, versionLabel } from '../core/listing';
import {
  installedBuildQueryOptions,
  runInstalledProgram,
} from '../queries/installed-page';

/**
 * An installed listing's page, in place of the marketplace lists. It shows
 * the newest minor version of the major the viewer is on: a run that finds a
 * newer one published, or a return to the window, loads it.
 */
export function InstalledPageView(props: {
  listing: Listing;
  /** An action on this listing is in flight. */
  busy: boolean;
  onBack(): void;
  /** Take the major version that is waiting. */
  onUpdate(): void;
}) {
  const build = useQuery(() => installedBuildQueryOptions(props.listing.id));
  // A pending first load's data read would suspend the page.
  const installed = () => (build.isPending ? undefined : build.data);
  // Keyed by source: a refetch of the same version (on focus) keeps the
  // frame, its state and its event stream.
  const built = createMemo(
    () => {
      const page = installed()?.page;
      return page?.status === 'built' ? page : undefined;
    },
    undefined,
    { equals: (previous, next) => previous?.sourceHash === next?.sourceHash }
  );
  const failed = () => {
    const page = installed()?.page;
    return page?.status === 'failed' ? page : undefined;
  };
  const plain = () => {
    const current = installed();
    return current?.page.status === 'plain'
      ? { html: current.plainHtml ?? '' }
      : undefined;
  };
  const unavailable = () =>
    build.isError &&
    (thrownResultErrorHasCode(build.error, 'BUILD_UNAVAILABLE') ||
      thrownResultErrorHasCode(build.error, 'BUILD_TOO_LARGE'));
  // The build a run found stale; the frame that replaces it says why the
  // page reloaded, since the failure shown on the old one goes with it.
  const [staleBuild, setStaleBuild] = createSignal<string>();
  const refetch = () => {
    fireAndForget(build.refetch(), 'marketplace-installed-build');
  };

  return (
    <div class="flex size-full min-h-0 min-w-0 flex-col">
      <div class="flex shrink-0 flex-wrap items-center gap-2 border-b border-edge-muted px-3 py-2">
        <Button
          variant="ghost"
          size="sm"
          aria-label={t('marketplace.backToMarketplace')}
          onClick={() => props.onBack()}
        >
          <ArrowLeftIcon class="size-4" />
          {t('common.back')}
        </Button>
        <h2 class="min-w-0 truncate text-sm font-semibold text-ink">
          {props.listing.title}
        </h2>
        <Show when={installed()}>
          {(current) => (
            <Badge variant="outline" size="xs">
              {versionLabel(current().version)}
            </Badge>
          )}
        </Show>
        <Show when={updateAvailable(props.listing)}>
          <Button
            variant="ghost"
            size="sm"
            class="ml-auto"
            disabled={props.busy}
            onClick={() => props.onUpdate()}
          >
            {t('marketplace.updateTo', {
              major: props.listing.latestVersion.major,
            })}
          </Button>
        </Show>
      </div>
      <div class="min-h-0 flex-1 overflow-auto bg-surface touch:pb-(--mobile-content-inset-bottom)">
        <Switch>
          <Match when={build.isPending}>
            <ListSkeleton.Root
              label={t('marketplace.pageLoading')}
              class="flex flex-col gap-3 p-4"
            >
              <div class="flex flex-col gap-3">
                <ListSkeleton.Bar class="h-4 w-1/3" />
                <ListSkeleton.Bar class="w-2/3" />
                <ListSkeleton.Bar class="w-1/2" />
              </div>
            </ListSkeleton.Root>
          </Match>
          <Match when={failed()}>
            {(failure) => <BuildProblems problems={failure().diagnostics} />}
          </Match>
          <Match when={plain()}>
            {(page) => <PlainFrame html={page().html} />}
          </Match>
          <Match when={built()} keyed>
            {(current) => (
              <ProgramFrame
                documentId={props.listing.id}
                html={current.html}
                sourceHash={current.sourceHash}
                run={runInstalledProgram}
                notice={
                  staleBuild() !== undefined &&
                  staleBuild() !== current.sourceHash
                    ? t('htmlPreview.pageUpdated')
                    : undefined
                }
                onStale={() => {
                  setStaleBuild(current.sourceHash);
                  refetch();
                }}
              />
            )}
          </Match>
          <Match when={unavailable()}>
            <div role="alert" class="p-4 text-sm text-ink">
              <p class="font-medium">{t('marketplace.pageUnavailableTitle')}</p>
              <p class="mt-1 text-ink-muted">
                {t('marketplace.pageUnavailableDescription')}
              </p>
            </div>
          </Match>
          <Match when={build.isError}>
            <BuildError onRetry={refetch} />
          </Match>
        </Switch>
      </div>
    </div>
  );
}
