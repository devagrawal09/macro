import { onMount } from 'solid-js';
import PluginSettingsFixture from './PluginSettingsFixture';
import { createPluginSettingsStore } from './store';
import {
  createHttpPluginSettingsStore,
  type HttpPluginSettingsStore,
} from './httpStore';

/**
 * Dev-only standalone route for the settings fixture (mirrors the
 * `/dev/plugin-frame` demo pattern).
 *
 * Backing store selection:
 * - `VITE_PLUGIN_SETTINGS_URL` set -> real HTTP-backed store against
 *   `plugin_http_service` (dev-only surface).
 * - unset -> existing mock store (safe default; no network dependency).
 *
 * Fails closed outside local/dev builds.
 */
export default function PluginSettingsFixtureRoute() {
  if (!import.meta.env.DEV) {
    return (
      <div class="p-4 text-sm text-muted-foreground">
        Plugin settings fixture is dev-only.
      </div>
    );
  }

  const url = import.meta.env.VITE_PLUGIN_SETTINGS_URL as string | undefined;
  const http = url ? createHttpPluginSettingsStore(url) : undefined;
  const store = http ?? createPluginSettingsStore();

  onMount(() => {
    if (http) (http as HttpPluginSettingsStore).reload();
  });

  return (
    <div>
      {http ? (
        <HttpBanner store={http} />
      ) : (
        <div class="mx-auto max-w-xl p-4 text-xs text-muted-foreground">
          mock store — set VITE_PLUGIN_SETTINGS_URL to use the HTTP surface
        </div>
      )}
      <PluginSettingsFixture store={store} />
    </div>
  );
}

function HttpBanner(props: { store: HttpPluginSettingsStore }) {
  const error = props.store.loadError;
  return (
    <>
      {error() ? (
        <div class="mx-auto max-w-xl p-4 text-xs text-red-600">
          HTTP store error: {error()}
        </div>
      ) : null}
    </>
  );
}
