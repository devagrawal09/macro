import { type Accessor, createSignal } from 'solid-js';
import type { PluginSettingsStore } from './store';
import type {
  PluginHandlerState,
  PluginInstallationMock,
  PluginRunOutcome,
  PluginRunRecord,
} from './types';

/**
 * HTTP-backed implementation of `PluginSettingsStore` against the dev-only
 * `plugin_http_service` endpoints:
 *
 * - `GET /plugins/settings`
 * - `POST /plugins/settings/enabled { enabled }`
 * - `POST /plugins/settings/handlers/{handlerId}/paused { paused }`
 *
 * Graceful fallback contract (dev fixture, never crash the UI):
 *
 * - Network/HTTP failures during load or mutation are recorded on
 *   `loadError()` and do NOT throw; the store keeps serving its last-known
 *   state so the fixture UI stays usable.
 * - Before the first successful load, `installation()` serves a neutral
 *   placeholder and `runs()` an empty list.
 * - Mutations are optimistic: local signals update immediately and the POST
 *   fires in the background. A failed POST surfaces via `loadError()` but is
 *   not rolled back, which is acceptable for this dev-only surface.
 *
 * This adapter is intentionally NOT registered as the default store; it is
 * consumed through the `PluginSettingsFixture` `store` prop.
 */
export interface HttpPluginSettingsStore extends PluginSettingsStore {
  /** Re-fetch installation + runs from the backend. Never throws. */
  reload(): Promise<void>;
  /** Human-readable message for the last failed load/mutation, if any. */
  loadError: Accessor<string | undefined>;
}

/** Neutral pre-load installation so the UI has a stable shape to render. */
const PENDING_INSTALLATION: PluginInstallationMock = {
  pluginId: '',
  name: '',
  version: '',
  verified: false,
  enabled: false,
  clientCapabilities: [],
  serverCapabilities: [],
  handlers: [],
};

/** Wire-shape of GET /plugins/settings served by plugin_http_service. */
export interface SettingsSnapshotDto {
  installation: PluginInstallationMock;
  runs: PluginRunRecordDto[];
}

/** Wire-shape of one run record; mirrors the Rust camelCase DTOs. */
export interface PluginRunRecordDto {
  id: string;
  startedAt: number;
  handlerId: string;
  outcome: string;
  durationMs: number;
  errorSummary?: string;
}

const OUTCOMES: readonly PluginRunOutcome[] = [
  'completed',
  'failed',
  'timeout',
];

function mapOutcome(raw: string): PluginRunOutcome {
  return (OUTCOMES as readonly string[]).includes(raw)
    ? (raw as PluginRunOutcome)
    : 'failed';
}

function mapRun(dto: PluginRunRecordDto): PluginRunRecord {
  return {
    id: dto.id,
    startedAt: dto.startedAt,
    handlerId: dto.handlerId,
    outcome: mapOutcome(dto.outcome),
    durationMs: dto.durationMs,
    ...(dto.errorSummary === undefined
      ? {}
      : { errorSummary: dto.errorSummary }),
  };
}

function mapHandlers(
  handlers: readonly PluginHandlerState[]
): PluginHandlerState[] {
  return handlers.map((handler) => ({ ...handler }));
}

function mapInstallation(dto: PluginInstallationMock): PluginInstallationMock {
  return {
    ...dto,
    clientCapabilities: [...dto.clientCapabilities],
    serverCapabilities: [...dto.serverCapabilities],
    project: dto.project ? { ...dto.project } : undefined,
    handlers: mapHandlers(dto.handlers),
  };
}

function describeError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export function createHttpPluginSettingsStore(
  baseUrl: string,
  fetchFn: typeof fetch = (...args) => fetch(...args)
): HttpPluginSettingsStore {
  const [installation, setInstallation] =
    createSignal<PluginInstallationMock>(PENDING_INSTALLATION);
  const [runs, setRuns] = createSignal<readonly PluginRunRecord[]>([]);
  const [loadError, setLoadError] = createSignal<string | undefined>(undefined);

  async function reload(): Promise<void> {
    try {
      const response = await fetchFn(`${baseUrl}/plugins/settings`);
      if (!response.ok) {
        throw new Error(`GET /plugins/settings failed with ${response.status}`);
      }
      const snapshot = (await response.json()) as SettingsSnapshotDto;
      setInstallation(mapInstallation(snapshot.installation));
      // The backend already returns newest-first history.
      setRuns(snapshot.runs.map(mapRun));
      setLoadError(undefined);
    } catch (error) {
      // Failed loads surface as state, not crashes.
      setLoadError(describeError(error));
    }
  }

  void reload();

  function request(path: string, body: Record<string, boolean>): void {
    fetchFn(`${baseUrl}${path}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`POST ${path} failed with ${response.status}`);
        }
        setLoadError(undefined);
      })
      .catch((error: unknown) => {
        setLoadError(describeError(error));
      });
  }

  return {
    installation,
    runs,
    loadError,
    reload,
    setEnabled(enabled) {
      setInstallation((current) => ({ ...current, enabled }));
      request('/plugins/settings/enabled', { enabled });
    },
    setHandlerPaused(handlerId, paused) {
      setInstallation((current) => ({
        ...current,
        handlers: current.handlers.map((handler) =>
          handler.id === handlerId ? { ...handler, paused } : handler
        ),
      }));
      request(
        `/plugins/settings/handlers/${encodeURIComponent(handlerId)}/paused`,
        {
          paused,
        }
      );
    },
  };
}
