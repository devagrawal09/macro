/**
 * Mock domain shapes for the plugin settings fixture.
 *
 * These intentionally mirror the planned plugin platform contracts
 * (installation, per-handler pause state, started-only run records) without
 * depending on any backend or generated schema yet. When the real API lands,
 * only `store.ts` and these types should need to change.
 */

/** Outcome of one server-handler invocation that actually started. */
export type PluginRunOutcome = 'completed' | 'failed' | 'timeout';

/** One server handler declared by the installed release. */
export interface PluginHandlerState {
  id: string;
  label: string;
  /** Event type this handler reacts to, e.g. `task.created`. */
  event: string;
  paused: boolean;
}

/** A project-scoped installation of one plugin release (mock shape). */
export interface PluginInstallationMock {
  pluginId: string;
  name: string;
  version: string;
  /** Local/dev sideloads are unverified; production installs would be true. */
  verified: boolean;
  enabled: boolean;
  /** Capabilities granted to client contributions (browser side). */
  clientCapabilities: readonly string[];
  /** Capabilities granted to server handlers (server side). */
  serverCapabilities: readonly string[];
  project?: { id: string; name: string };
  handlers: readonly PluginHandlerState[];
}

/**
 * One admitted/started handler run. Events that never start a run are
 * discarded by contract and never appear here, so there are no retry or
 * replay controls for them.
 */
export interface PluginRunRecord {
  id: string;
  startedAt: number;
  handlerId: string;
  outcome: PluginRunOutcome;
  durationMs: number;
  errorSummary?: string;
}
