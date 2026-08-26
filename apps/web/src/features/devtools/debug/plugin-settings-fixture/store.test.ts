import { describe, expect, it } from 'vitest';
import {
  createPluginSettingsStore,
  MOCK_INSTALLATION,
  MOCK_RUNS,
} from './store';
import type { PluginInstallationMock } from './types';

describe('createPluginSettingsStore', () => {
  it('defaults to the mock installation and run history', () => {
    const store = createPluginSettingsStore();

    expect(store.installation()).toEqual(MOCK_INSTALLATION);
    expect(store.runs()).toEqual(MOCK_RUNS);
  });

  it('accepts seeded initial state', () => {
    const installation: PluginInstallationMock = {
      ...MOCK_INSTALLATION,
      pluginId: 'dev.local.other',
      enabled: false,
    };
    const store = createPluginSettingsStore({ installation, runs: [] });

    expect(store.installation().pluginId).toBe('dev.local.other');
    expect(store.installation().enabled).toBe(false);
    expect(store.runs()).toEqual([]);
  });

  it('setEnabled toggles enabled without mutating the shared mock', () => {
    const store = createPluginSettingsStore();

    store.setEnabled(false);
    expect(store.installation().enabled).toBe(false);

    store.setEnabled(true);
    expect(store.installation().enabled).toBe(true);

    expect(MOCK_INSTALLATION.enabled).toBe(true);
  });

  it('setHandlerPaused pauses only the matching handler', () => {
    const store = createPluginSettingsStore();

    store.setHandlerPaused('normalize-created-task', true);

    const handlers = store.installation().handlers;
    expect(handlers).toHaveLength(1);
    expect(handlers[0].paused).toBe(true);
    expect(MOCK_INSTALLATION.handlers[0].paused).toBe(false);
  });

  it('setHandlerPaused ignores unknown handler ids', () => {
    const store = createPluginSettingsStore();

    store.setHandlerPaused('does-not-exist', true);

    expect(store.installation().handlers).toEqual(MOCK_INSTALLATION.handlers);
  });

  it('mock run history is newest first and started-only outcomes are covered', () => {
    const startedAts = MOCK_RUNS.map((run) => run.startedAt);
    const sorted = [...startedAts].sort((a, b) => b - a);

    expect(startedAts).toEqual(sorted);
    expect(new Set(MOCK_RUNS.map((run) => run.outcome))).toEqual(
      new Set(['completed', 'failed', 'timeout'])
    );
  });
});
