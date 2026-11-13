import { ListSkeleton } from '@app/components/view-shell/ListSkeleton';
import { t } from '@app/lib/i18n';
import { usePending } from '@app/observability/flow';
import {
  BuildError,
  BuildProblems,
  PlainFrame,
  ProgramFrame,
} from '@block-code/component/HtmlPreview';
import { fireAndForget } from '@core/util/fireAndForget';
import ArrowLeftIcon from '@icon/system/regular/arrow-left.svg';
import type {
  ActionTrigger,
  InterfaceChange,
  PendingVersion,
  RoutineSnapshot,
  RoutineTask,
  VersionReview,
} from '@service-agent-harness/generated/schemas';
import { useQuery } from '@tanstack/solid-query';
import { Badge, Button } from '@ui';
import { For, type JSX, Match, Show, Switch } from 'solid-js';
import { versionLabel } from '../core/listing';
import { refuseReviewRun, versionReviewQueryOptions } from '../queries/review';

type PageReview = Extract<VersionReview, { kind: 'artifact' }>;
type RoutineReview = Extract<VersionReview, { kind: 'routine' }>;

/**
 * A pending version as its reviewer reads it, in place of the review list.
 * A page renders with every program refused, beside its program ids and its
 * source; a routine shows its trigger, steps, apps and what it changes for
 * installs. Nothing in either runs.
 */
export function ReviewPreviewView(props: {
  pending: PendingVersion;
  onBack(): void;
  /** A routine's trigger as people read it. */
  triggerLines: (trigger: ActionTrigger) => string[];
}) {
  const review = useQuery(() =>
    versionReviewQueryOptions(props.pending.listingId)
  );
  usePending('marketplace.review_preview', () => review.isPending);
  // A pending first load's data read would suspend the page.
  const loaded = () => (review.isPending ? undefined : review.data);
  const page = () => {
    const current = loaded();
    return current?.kind === 'artifact' ? current : undefined;
  };
  const routine = () => {
    const current = loaded();
    return current?.kind === 'routine' ? current : undefined;
  };

  return (
    <div class="flex size-full min-h-0 min-w-0 flex-col">
      <div class="flex shrink-0 items-center gap-2 border-b border-edge-muted px-3 py-2 mobile:gap-1.5 mobile:px-2">
        <Button
          variant="ghost"
          size="sm"
          class="touch:min-h-11 touch:min-w-11"
          aria-label={t('marketplace.backToReview')}
          onClick={() => props.onBack()}
        >
          <ArrowLeftIcon class="size-4" />
          <span class="mobile:hidden">{t('common.back')}</span>
        </Button>
        <h2 class="min-w-0 flex-1 truncate text-sm font-semibold text-ink">
          {props.pending.title}
        </h2>
        <Badge variant="outline" size="xs" class="shrink-0">
          {versionLabel(props.pending.version)}
        </Badge>
      </div>
      <div class="min-h-0 flex-1 overflow-auto touch:pb-(--mobile-content-inset-bottom)">
        <Switch>
          <Match when={review.isPending}>
            <ListSkeleton.Root
              label={t('marketplace.reviewPreviewLoading')}
              class="flex flex-col gap-3 p-4"
            >
              <div class="flex flex-col gap-3">
                <ListSkeleton.Bar class="h-4 w-1/3" />
                <ListSkeleton.Bar class="w-2/3" />
                <ListSkeleton.Bar class="w-1/2" />
              </div>
            </ListSkeleton.Root>
          </Match>
          <Match when={page()}>
            {(current) => <PagePreview review={current()} />}
          </Match>
          <Match when={routine()}>
            {(current) => (
              <RoutinePreview
                review={current()}
                triggerLines={props.triggerLines}
              />
            )}
          </Match>
          <Match when={review.isError}>
            <BuildError
              onRetry={() =>
                fireAndForget(review.refetch(), 'marketplace-review-preview')
              }
            />
          </Match>
        </Switch>
      </div>
    </div>
  );
}

/** A titled part of the preview. */
function Section(props: { title: string; children: JSX.Element }) {
  return (
    <section class="flex flex-col gap-2 px-4 py-3 mobile:px-3">
      <h3 class="text-xs font-semibold text-ink-muted uppercase">
        {props.title}
      </h3>
      {props.children}
    </section>
  );
}

/** Code or instructions, as text to read. */
function CodeBlock(props: { label: string; text: string }) {
  return (
    <pre
      aria-label={props.label}
      class="max-h-96 overflow-auto rounded-md border border-edge-muted bg-inset p-3 font-mono text-xs whitespace-pre-wrap wrap-anywhere text-ink"
    >
      {props.text}
    </pre>
  );
}

/** A page under review: rendered with its programs refused, and its source. */
function PagePreview(props: { review: PageReview }) {
  const built = () =>
    props.review.page.status === 'built' ? props.review.page : undefined;
  const failed = () =>
    props.review.page.status === 'failed' ? props.review.page : undefined;

  return (
    <div class="flex flex-col">
      <p
        role="note"
        class="border-b border-edge-muted bg-inset px-4 py-2 text-xs text-ink mobile:px-3"
      >
        {t('marketplace.reviewProgramsOff')}
      </p>
      <Show when={built()}>
        {(page) => (
          <Section title={t('marketplace.reviewPrograms')}>
            <Show
              when={page().programs.length > 0}
              fallback={
                <p class="text-xs text-ink-muted">
                  {t('marketplace.reviewNoPrograms')}
                </p>
              }
            >
              <ul
                aria-label={t('marketplace.reviewPrograms')}
                class="flex flex-wrap gap-1.5"
              >
                <For each={page().programs}>
                  {(program) => (
                    <li>
                      <Badge variant="outline" size="xs" class="font-mono">
                        {program}
                      </Badge>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          </Section>
        )}
      </Show>
      <Section title={t('marketplace.reviewPage')}>
        <div class="h-[60dvh] overflow-hidden rounded-md border border-edge-muted bg-surface">
          <Switch>
            <Match when={failed()}>
              {(failure) => <BuildProblems problems={failure().diagnostics} />}
            </Match>
            <Match when={built()}>
              {(page) => (
                <ProgramFrame
                  documentId={props.review.pending.listingId}
                  html={page().html}
                  sourceHash={page().sourceHash}
                  run={refuseReviewRun}
                  onStale={() => {}}
                />
              )}
            </Match>
            <Match when={props.review.page.status === 'plain'}>
              <PlainFrame html={props.review.html} />
            </Match>
          </Switch>
        </div>
      </Section>
      <Section title={t('marketplace.reviewSource')}>
        <CodeBlock
          label={t('marketplace.reviewSource')}
          text={props.review.html}
        />
      </Section>
    </div>
  );
}

/** One step of a routine, as text. */
type StepText = { title: string; detail?: string; text: string };

function stepTexts(task: RoutineTask): StepText[] {
  if (!('steps' in task)) {
    return [
      { title: t('marketplace.reviewStepAgent'), text: task.user_prompt },
    ];
  }

  return task.steps.map((step) =>
    step.type === 'code'
      ? {
          title: step.title,
          detail: step.summary ?? undefined,
          text: step.source,
        }
      : { title: t('marketplace.reviewStepAgent'), text: step.instructions }
  );
}

function changeLabel(change: InterfaceChange): string {
  const key = `${change.change}_${change.part}` as const;
  switch (key) {
    case 'added_variable':
      return t('marketplace.reviewAddedVariable', { name: change.name });
    case 'removed_variable':
      return t('marketplace.reviewRemovedVariable', { name: change.name });
    case 'added_app':
      return t('marketplace.reviewAddedApp', { name: change.name });
    case 'removed_app':
      return t('marketplace.reviewRemovedApp', { name: change.name });
    case 'added_trigger':
      return t('marketplace.reviewAddedTrigger', { name: change.name });
    case 'removed_trigger':
      return t('marketplace.reviewRemovedTrigger', { name: change.name });
    case 'added_knob':
      return t('marketplace.reviewAddedKnob', { name: change.name });
    case 'removed_knob':
      return t('marketplace.reviewRemovedKnob', { name: change.name });
  }
}

/** A routine under review: what starts it, what it does and asks for. */
function RoutinePreview(props: {
  review: RoutineReview;
  triggerLines: (trigger: ActionTrigger) => string[];
}) {
  const routine = (): RoutineSnapshot => props.review.routine;
  const apps = () => routine().task.mcp_servers ?? [];
  const triggers = () => props.triggerLines(routine().trigger);

  return (
    <div class="flex flex-col">
      <Section title={t('marketplace.reviewRoutineName')}>
        <p class="text-sm font-medium text-ink">{routine().name}</p>
        <Show when={routine().description}>
          <p class="text-xs text-ink-muted">{routine().description}</p>
        </Show>
      </Section>
      <Section title={t('marketplace.reviewInterfaceChanges')}>
        <Show
          when={props.review.interfaceChanges.length > 0}
          fallback={
            <p class="text-xs text-ink-muted">
              {t('marketplace.reviewNoInterfaceChanges')}
            </p>
          }
        >
          <ul class="flex flex-col gap-1 text-xs text-ink">
            <For each={props.review.interfaceChanges}>
              {(change) => <li>{changeLabel(change)}</li>}
            </For>
          </ul>
        </Show>
      </Section>
      <Section title={t('marketplace.reviewTrigger')}>
        <Show
          when={triggers().length > 0}
          fallback={
            <CodeBlock
              label={t('marketplace.reviewTrigger')}
              text={JSON.stringify(routine().trigger, null, 2)}
            />
          }
        >
          <ul class="flex flex-col gap-1 text-xs text-ink">
            <For each={triggers()}>{(line) => <li>{line}</li>}</For>
          </ul>
        </Show>
      </Section>
      <Section title={t('marketplace.reviewApps')}>
        <Show
          when={apps().length > 0}
          fallback={
            <p class="text-xs text-ink-muted">
              {t('marketplace.reviewNoApps')}
            </p>
          }
        >
          <ul class="flex flex-wrap gap-1.5">
            <For each={apps()}>
              {(app) => (
                <li>
                  <Badge variant="outline" size="xs">
                    {app.server_name}
                  </Badge>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </Section>
      <Section title={t('marketplace.reviewSteps')}>
        <ol class="flex flex-col gap-3">
          <For each={stepTexts(routine().task)}>
            {(step, index) => (
              <li class="flex flex-col gap-1">
                <p class="text-xs font-medium text-ink">
                  {t('marketplace.reviewStepTitle', {
                    number: index() + 1,
                    title: step.title,
                  })}
                </p>
                <Show when={step.detail}>
                  {(detail) => <p class="text-xs text-ink-muted">{detail()}</p>}
                </Show>
                <CodeBlock label={step.title} text={step.text} />
              </li>
            )}
          </For>
        </ol>
      </Section>
    </div>
  );
}
