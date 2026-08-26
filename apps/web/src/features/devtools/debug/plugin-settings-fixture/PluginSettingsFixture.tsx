import { InlineCheckbox } from '@ui';
import { For, Show } from 'solid-js';
import { createPluginSettingsStore, type PluginSettingsStore } from './store';
import type { PluginRunOutcome } from './types';

/**
 * Fallback store used when the fixture renders without an explicit `store`
 * prop. Module-level is fine: the mock factory holds plain signals.
 */
const DEFAULT_STORE = createPluginSettingsStore();

const OUTCOME_LABELS: Record<PluginRunOutcome, string> = {
  completed: 'completed',
  failed: 'failed',
  timeout: 'timeout',
};

/**
 * Dev fixture for the plugin platform settings surface (POC/demo quality).
 *
 * Renders one mock installation of the Task Tools trial plugin: metadata and
 * capabilities, an enable toggle, per-handler pause controls, and the
 * started-run history. Everything reads through `PluginSettingsStore`, so a
 * real backend adapter can later be passed as the `store` prop without
 * changing this UI.
 */
export default function PluginSettingsFixture(props: {
  store?: PluginSettingsStore;
}) {
  const store = () => props.store ?? DEFAULT_STORE;

  const formatTime = (startedAt: number) =>
    new Date(startedAt).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });

  return (
    <div class="mx-auto max-w-xl space-y-4 p-4 text-sm">
      <header class="space-y-1">
        <h1 class="text-base font-medium">Plugin settings</h1>
        <p class="text-ink-secondary text-xs">
          Dev fixture backed by mock data; not wired to the plugin backend yet.
        </p>
      </header>

      <section class="space-y-2 rounded-sm border border-edge-muted bg-surface p-3">
        <div class="flex items-baseline gap-2">
          <span class="font-medium">{store().installation().name}</span>
          <span class="text-ink-secondary text-xs">
            {store().installation().pluginId} v{store().installation().version}
          </span>
        </div>
        <Show when={store().installation().project}>
          {(project) => (
            <div class="text-ink-secondary text-xs">
              Project: {project().name}
            </div>
          )}
        </Show>
        <div class="flex flex-wrap gap-3 text-xs">
          <span class="text-ink-secondary">
            Client caps: {store().installation().clientCapabilities.join(', ')}
          </span>
          <span class="text-ink-secondary">
            Server caps: {store().installation().serverCapabilities.join(', ')}
          </span>
        </div>
        <Show
          when={store().installation().verified}
          fallback={
            <span class="text-xs text-amber-600">
              Unverified local sideload
            </span>
          }
        >
          <span class="text-xs text-green-700">Verified</span>
        </Show>
        <button
          type="button"
          class="flex items-center gap-2 rounded-sm border border-edge-muted p-2 text-left hover:bg-hover"
          onClick={() => store().setEnabled(!store().installation().enabled)}
        >
          <InlineCheckbox checked={store().installation().enabled} />
          Enabled
        </button>
      </section>

      <section class="space-y-2 rounded-sm border border-edge-muted bg-surface p-3">
        <h2 class="font-medium">Server handlers</h2>
        <For each={store().installation().handlers}>
          {(handler) => (
            <button
              type="button"
              class="flex w-full items-center gap-2 rounded-sm border border-edge-muted p-2 text-left hover:bg-hover"
              onClick={() =>
                store().setHandlerPaused(handler.id, !handler.paused)
              }
            >
              <InlineCheckbox checked={!handler.paused} />
              <span class="min-w-0 flex-1">
                <span class="block">{handler.label}</span>
                <span class="text-ink-secondary block text-xs">
                  {handler.event}
                </span>
              </span>
              <span class="text-ink-secondary shrink-0 text-xs">
                {handler.paused ? 'paused' : 'active'}
              </span>
            </button>
          )}
        </For>
      </section>

      <section class="space-y-2 rounded-sm border border-edge-muted bg-surface p-3">
        <h2 class="font-medium">Started runs</h2>
        <p class="text-ink-secondary text-xs">
          Only runs that actually started are listed; unadmitted events never
          appear here by contract.
        </p>
        <ul class="space-y-1">
          <For each={store().runs()}>
            {(run) => (
              <li class="flex items-center gap-2 rounded-sm border border-edge-muted px-2 py-1 text-xs">
                <span>{formatTime(run.startedAt)}</span>
                <span
                  classList={{
                    'font-medium': true,
                    'text-green-700': run.outcome === 'completed',
                    'text-red-600': run.outcome === 'failed',
                    'text-amber-600': run.outcome === 'timeout',
                  }}
                >
                  {OUTCOME_LABELS[run.outcome]}
                </span>
                <span>{run.durationMs}ms</span>
                <Show when={run.errorSummary}>
                  {(summary) => (
                    <span class="text-ink-secondary truncate">{summary()}</span>
                  )}
                </Show>
              </li>
            )}
          </For>
        </ul>
      </section>
    </div>
  );
}
