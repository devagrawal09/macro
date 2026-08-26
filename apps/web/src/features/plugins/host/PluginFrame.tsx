import { Button } from '@ui';
import {
  createEffect,
  createSignal,
  type JSX,
  on,
  onCleanup,
  Show,
} from 'solid-js';
import type {
  PluginClientContext,
  PluginGrant,
  PluginHostIntent,
} from '../contract';
import { createPluginSession, type PluginSessionState } from './pluginSession';

export interface PluginFrameProps {
  /** Accessible frame title; the plugin cannot change it. */
  title: string;
  /** Full srcdoc document: bootstrap glue plus the verified client bundle. */
  srcdoc: string;
  /** Mount context delivered to the plugin over the claimed port. */
  context: PluginClientContext;
  /**
   * Grant seam. This checkpoint passes fake local/dev data from props;
   * real delegated-grant minting plugs in here later.
   */
  fetchGrant: (context: PluginClientContext) => Promise<PluginGrant>;
  /**
   * Typed host intent dispatched by the plugin over the claimed port.
   * Malformed or unknown port messages never reach this callback and never
   * tear the session down.
   */
  onIntent?: (intent: PluginHostIntent) => void;
  timeoutMs?: number;
  class?: string;
}

interface RetryableSession {
  retry: () => void;
}

function FrameInstance(
  props: PluginFrameProps & {
    sessionVersion: number;
    onStateChange: (state: PluginSessionState) => void;
    bindSession: (session: RetryableSession) => void;
  }
): JSX.Element {
  let iframe: HTMLIFrameElement | undefined;
  const session = createPluginSession({
    getFrame: () => iframe,
    context: props.context,
    fetchGrant: props.fetchGrant,
    onIntent: props.onIntent,
    onStateChange: props.onStateChange,
    timeoutMs: props.timeoutMs,
  });
  props.bindSession(session);

  // Covers both explicit unmount and keyed replacement on context change.
  onCleanup(() => session.dispose());

  return (
    <iframe
      ref={iframe}
      title={props.title}
      class="size-full border-0"
      sandbox="allow-scripts"
      // Opaque-origin frames report origin "null"; never leak referrers.
      referrerPolicy="no-referrer"
      srcdoc={props.srcdoc}
      onLoad={() => session.handleLoad()}
    />
  );
}

const errorCopy = (state: Extract<PluginSessionState, { phase: 'error' }>) =>
  state.reason === 'handshake-timeout'
    ? 'The plugin did not respond in time.'
    : state.reason === 'disposed'
      ? ''
      : 'The plugin failed to load.';

function ErrorCard(props: {
  state: () => PluginSessionState;
  onRetry: () => void;
}): JSX.Element {
  const copy = () =>
    props.state().phase === 'error' ? errorCopy(props.state()) : '';
  return (
    <Show when={copy().length > 0}>
      <div class="flex flex-col items-center gap-3 p-6">
        <p class="text-sm text-secondary" role="alert">
          {copy()}
        </p>
        <Button variant="base" onClick={() => props.onRetry()}>
          Retry
        </Button>
      </div>
    </Show>
  );
}

/**
 * Macro-owned host for any compiled client plugin bundle. Mounts the bundle
 * inside a sandboxed opaque-origin iframe and runs the hardened handshake
 * (one-time nonce, generation token, typed MessageChannel port transfer)
 * with Macro-owned loading and error fallback UI. Changing `context` tears
 * the frame down and remounts it with a fresh session.
 */
export function PluginFrame(props: PluginFrameProps): JSX.Element {
  const [sessionVersion, setSessionVersion] = createSignal(1);
  const [state, setState] = createSignal<PluginSessionState>({
    phase: 'loading',
  });
  let liveSession: RetryableSession | undefined;

  // Unmount the frame and start a fresh session whenever the mount
  // context changes.
  createEffect(
    on(
      () => props.context,
      () => setSessionVersion((version) => version + 1),
      { defer: true }
    )
  );

  return (
    <div
      class={`relative size-full overflow-hidden bg-surface ${props.class ?? ''}`}
    >
      <Show when={sessionVersion()} keyed>
        {(version) => (
          <FrameInstance
            // Keyed by sessionVersion above: a version bump tears the
            // frame down fully (ports, listeners, timer) and mounts a
            // brand-new opaque-origin frame with a fresh session.
            sessionVersion={version}
            {...props}
            onStateChange={setState}
            bindSession={(session) => {
              liveSession = session;
            }}
          />
        )}
      </Show>
      <Show when={state().phase !== 'ready'}>
        <div class="absolute inset-0 flex items-center justify-center bg-surface">
          <Show
            when={state().phase === 'error'}
            fallback={<p class="text-sm text-secondary">Loading plugin…</p>}
          >
            <ErrorCard
              state={state}
              onRetry={() => {
                setState({ phase: 'loading' });
                liveSession?.retry();
              }}
            />
          </Show>
        </div>
      </Show>
    </div>
  );
}
