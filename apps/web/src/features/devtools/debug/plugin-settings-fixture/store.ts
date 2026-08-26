import { type Accessor, createSignal } from 'solid-js';
import type { PluginInstallationMock, PluginRunRecord } from './types';

/**
 * The single seam between the fixture UI and its state. Today it is backed by
 * mock data and local signals; wiring the real backend later means replacing
 * this factory (or passing an alternative implementation as the component's
 * `store` prop) without touching the UI code.
 */
export interface PluginSettingsStore {
  installation: Accessor<PluginInstallationMock>;
  /** Started runs only, newest first. */
  runs: Accessor<readonly PluginRunRecord[]>;
  setEnabled(enabled: boolean): void;
  setHandlerPaused(handlerId: string, paused: boolean): void;
}

/** Mock installation matching the Task Tools trial plugin. */
export const MOCK_INSTALLATION: PluginInstallationMock = {
  pluginId: 'dev.local.task-tools',
  name: 'Task Tools',
  version: '0.1.0',
  verified: false,
  enabled: true,
  project: { id: 'proj-demo-1', name: 'Demo Project' },
  clientCapabilities: ['tasks.read', 'tasks.create', 'task-events.read'],
  serverCapabilities: ['tasks.read', 'tasks.rename'],
  handlers: [
    {
      id: 'normalize-created-task',
      label: 'Normalize created task',
      event: 'task.created',
      paused: false,
    },
  ],
};

/** Mock started-run history: one of each outcome, newest first. */
export const MOCK_RUNS: readonly PluginRunRecord[] = [
  {
    id: 'run-3',
    startedAt: Date.UTC(2026, 7, 26, 9, 14, 3),
    handlerId: 'normalize-created-task',
    outcome: 'completed',
    durationMs: 812,
  },
  {
    id: 'run-2',
    startedAt: Date.UTC(2026, 7, 26, 9, 2, 41),
    handlerId: 'normalize-created-task',
    outcome: 'failed',
    durationMs: 240,
    errorSummary: 'Task not found (404)',
  },
  {
    id: 'run-1',
    startedAt: Date.UTC(2026, 7, 26, 8, 47, 12),
    handlerId: 'normalize-created-task',
    outcome: 'timeout',
    durationMs: 30_000,
  },
];

/** Optional seed data for a mock-backed store. */
export interface PluginSettingsStoreSeed {
  installation?: PluginInstallationMock;
  runs?: readonly PluginRunRecord[];
}

/**
 * Create the default mock implementation of `PluginSettingsStore`.
 *
 * State is held in local Solid signals seeded from the mock constants, so
 * mutations are reactive and never leak back into `MOCK_INSTALLATION` /
 * `MOCK_RUNS`. Swap this factory for a real backend adapter later without
 * touching the fixture UI.
 */
export function createPluginSettingsStore(
  seed: PluginSettingsStoreSeed = {}
): PluginSettingsStore {
  const [installation, setInstallation] = createSignal<PluginInstallationMock>(
    seed.installation ?? MOCK_INSTALLATION
  );
  const [runs] = createSignal<readonly PluginRunRecord[]>(
    seed.runs ?? MOCK_RUNS
  );

  return {
    installation,
    runs,
    setEnabled(enabled) {
      setInstallation((current) => ({ ...current, enabled }));
    },
    setHandlerPaused(handlerId, paused) {
      setInstallation((current) => ({
        ...current,
        handlers: current.handlers.map((handler) =>
          handler.id === handlerId ? { ...handler, paused } : handler
        ),
      }));
    },
  };
}
