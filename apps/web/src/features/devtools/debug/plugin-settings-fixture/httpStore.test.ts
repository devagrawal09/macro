import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createHttpPluginSettingsStore,
  type SettingsSnapshotDto,
} from './httpStore';

function snapshot(): SettingsSnapshotDto {
  return {
    installation: {
      pluginId: 'dev.local.task-tools',
      name: 'Task Tools',
      version: '0.1.0',
      verified: false,
      enabled: true,
      clientCapabilities: ['tasks.read'],
      serverCapabilities: ['tasks.rename'],
      project: { id: 'proj-demo-1', name: 'Demo Project' },
      handlers: [
        {
          id: 'normalize-created-task',
          label: 'Normalize created task',
          event: 'task.created',
          paused: false,
        },
      ],
    },
    runs: [
      {
        id: 'run-3',
        startedAt: 1_787_735_643_000,
        handlerId: 'normalize-created-task',
        outcome: 'completed',
        durationMs: 812,
      },
      {
        id: 'run-2',
        startedAt: 1_787_734_961_000,
        handlerId: 'normalize-created-task',
        outcome: 'failed',
        durationMs: 240,
        errorSummary: 'Task not found (404)',
      },
    ],
  };
}

type FetchMock = ReturnType<typeof vi.fn>;

let fetchMock: FetchMock;

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

describe('createHttpPluginSettingsStore', () => {
  beforeEach(() => {
    fetchMock = vi.fn();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('loads installation and runs from GET /plugins/settings on creation', async () => {
    fetchMock.mockImplementation(() =>
      Promise.resolve(jsonResponse(snapshot()))
    );
    const store = createHttpPluginSettingsStore(
      'http://127.0.0.1:8135',
      fetchMock as unknown as typeof fetch
    );
    await store.reload();

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:8135/plugins/settings'
    );
    expect(store.installation().pluginId).toBe('dev.local.task-tools');
    expect(store.installation().enabled).toBe(true);
    expect(store.runs().map((run) => run.id)).toEqual(['run-3', 'run-2']);
    expect(store.loadError()).toBeUndefined();
  });

  it('maps run outcomes and keeps errorSummary only when present', async () => {
    const dto = snapshot();
    dto.runs[1].outcome = 'weird-outcome' as never;
    delete (dto.runs[0] as { errorSummary?: string }).errorSummary;
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse(dto)));

    const store = createHttpPluginSettingsStore(
      '',
      fetchMock as unknown as typeof fetch
    );
    await store.reload();

    expect(store.runs()[0].outcome).toBe('completed');
    expect(store.runs()[0].errorSummary).toBeUndefined();
    expect(store.runs()[1].outcome).toBe('failed');
    expect(store.runs()[1].errorSummary).toBe('Task not found (404)');
  });

  it('serves a neutral placeholder before the first successful load', () => {
    let pendingFetch: Promise<Response> = new Promise(() => {});
    fetchMock.mockReturnValue(pendingFetch);
    const store = createHttpPluginSettingsStore(
      '',
      fetchMock as unknown as typeof fetch
    );

    expect(store.installation().name).toBe('');
    expect(store.installation().handlers).toEqual([]);
    expect(store.runs()).toEqual([]);
  });

  it('setEnabled updates the signal optimistically and posts the toggle', async () => {
    fetchMock.mockImplementation((input: RequestInfo | URL) =>
      String(input).endsWith('/plugins/settings')
        ? Promise.resolve(jsonResponse(snapshot()))
        : Promise.resolve(jsonResponse({ installation: {}, runs: [] }))
    );
    const store = createHttpPluginSettingsStore(
      '',
      fetchMock as unknown as typeof fetch
    );
    await store.reload();

    store.setEnabled(false);
    expect(store.installation().enabled).toBe(false);

    await vi.waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith('/plugins/settings/enabled', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ enabled: false }),
      });
    });
    expect(store.loadError()).toBeUndefined();
  });

  it('setHandlerPaused pauses only the matching handler and posts the pause', async () => {
    fetchMock.mockImplementation((input: RequestInfo | URL) =>
      String(input).endsWith('/plugins/settings')
        ? Promise.resolve(jsonResponse(snapshot()))
        : Promise.resolve(jsonResponse({ installation: {}, runs: [] }))
    );
    const store = createHttpPluginSettingsStore(
      '',
      fetchMock as unknown as typeof fetch
    );
    await store.reload();

    store.setHandlerPaused('normalize-created-task', true);
    expect(store.installation().handlers[0].paused).toBe(true);

    await vi.waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        '/plugins/settings/handlers/normalize-created-task/paused',
        {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ paused: true }),
        }
      );
    });
  });

  it('records failed loads on loadError instead of crashing', async () => {
    fetchMock.mockRejectedValue(new TypeError('network down'));
    const store = createHttpPluginSettingsStore(
      '',
      fetchMock as unknown as typeof fetch
    );
    await store.reload();

    expect(store.loadError()).toBe('network down');
    // The store stays usable with its last-known state.
    expect(store.installation()).toBeDefined();
    expect(store.runs()).toEqual([]);
  });

  it('records non-ok load responses on loadError', async () => {
    fetchMock.mockImplementation(() =>
      Promise.resolve(jsonResponse({ error: {} }, 503))
    );
    const store = createHttpPluginSettingsStore(
      '',
      fetchMock as unknown as typeof fetch
    );
    await store.reload();

    expect(store.loadError()).toContain('503');
  });

  it('records failed mutations on loadError without throwing', async () => {
    fetchMock.mockImplementation((input: RequestInfo | URL) =>
      String(input).endsWith('/plugins/settings')
        ? Promise.resolve(jsonResponse(snapshot()))
        : Promise.resolve(jsonResponse({ error: {} }, 500))
    );
    const store = createHttpPluginSettingsStore(
      '',
      fetchMock as unknown as typeof fetch
    );
    await store.reload();

    expect(() => store.setEnabled(true)).not.toThrow();
    expect(store.installation().enabled).toBe(true);

    await vi.waitFor(() => {
      expect(store.loadError()).toContain('500');
    });
  });
});
