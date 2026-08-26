/** @vitest-environment jsdom */

import { fireEvent, render } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  PluginClientContext,
  PluginGrant,
  PluginHostIntent,
} from '../contract';
import {
  PLUGIN_CONNECT_TYPE,
  PLUGIN_CONTEXT_TYPE,
  PLUGIN_INIT_TYPE,
  PLUGIN_INTENT_TYPE,
  PLUGIN_PROTOCOL_VERSION,
  PLUGIN_READY_TYPE,
} from '../contract';
import { PluginFrame } from './PluginFrame';

const CONTEXT: PluginClientContext = {
  projectId: 'project-1',
  capabilities: ['tasks.read'],
};

const GRANT: PluginGrant = {
  token: 'fake-grant',
  expiresAt: '2030-01-01T00:00:00.000Z',
  capabilities: ['tasks.read'],
};

class FakePort {
  postMessage = vi.fn();
  close = vi.fn();
  start = vi.fn();
}

class FakeChannel {
  port1 = new FakePort() as unknown as MessagePort;
  port2 = new FakePort() as unknown as MessagePort;
}

let channels: FakeChannel[];
let nonceCounter: number;

interface PostCapture {
  calls: Array<
    [message: unknown, targetOrigin: string, transfer?: Transferable[]]
  >;
  ofType: (type: string) => Array<Record<string, unknown>>;
}

/**
 * Capture postMessage calls by shadowing the jsdom window method.
 * jsdom fires one natural load event per srcdoc iframe; tests that need an
 * additional navigation call fireEvent.load themselves.
 */
function capturePostMessage(win: Window): PostCapture {
  const calls: PostCapture['calls'] = [];
  win.postMessage = ((
    message: unknown,
    targetOrigin: string,
    transfer?: Transferable[]
  ) => {
    calls.push([message, targetOrigin, transfer]);
  }) as Window['postMessage'];
  return {
    calls,
    ofType: (type: string) =>
      calls
        .filter((call) => (call[0] as { type?: string }).type === type)
        .map((call) => call[0] as Record<string, unknown>),
  };
}

async function mountFrame(overrides?: {
  fetchGrant?: () => Promise<PluginGrant> | Promise<PluginGrant>;
  onIntent?: (intent: PluginHostIntent) => void;
  timeoutMs?: number;
  context?: PluginClientContext;
}) {
  const rawFetchGrant =
    overrides?.fetchGrant ?? vi.fn().mockResolvedValue(GRANT);
  const fetchGrant =
    typeof rawFetchGrant === 'function' ? rawFetchGrant : () => rawFetchGrant;
  const view = render(() => (
    <PluginFrame
      title="Test plugin"
      srcdoc="<html><body>bootstrap</body></html>"
      context={overrides?.context ?? CONTEXT}
      fetchGrant={fetchGrant}
      onIntent={overrides?.onIntent}
      timeoutMs={overrides?.timeoutMs}
    />
  ));
  const iframe = view.container.querySelector('iframe')!;
  const posted = capturePostMessage(iframe.contentWindow!);
  await vi.waitFor(() =>
    expect(posted.ofType(PLUGIN_INIT_TYPE)).toHaveLength(1)
  );
  return {
    view,
    iframe,
    posted,
    fetchGrant,
    initNonces: () =>
      posted.ofType(PLUGIN_INIT_TYPE).map((message) => message.nonce as string),
  };
}

type FrameHandle = Awaited<ReturnType<typeof mountFrame>>;

function readyFrom(
  frameOrIframe: FrameHandle | HTMLIFrameElement,
  nonce: string,
  origin = 'null'
) {
  const iframe =
    frameOrIframe instanceof HTMLIFrameElement
      ? frameOrIframe
      : frameOrIframe.iframe;
  window.dispatchEvent(
    new MessageEvent('message', {
      source: iframe.contentWindow,
      origin,
      data: { type: PLUGIN_READY_TYPE, nonce },
    })
  );
}

beforeEach(() => {
  channels = [];
  nonceCounter = 0;
  vi.stubGlobal(
    'MessageChannel',
    class {
      port1: MessagePort;
      port2: MessagePort;
      constructor() {
        const channel = new FakeChannel();
        channels.push(channel);
        this.port1 = channel.port1;
        this.port2 = channel.port2;
      }
    }
  );
  vi.spyOn(crypto, 'randomUUID').mockImplementation(() => {
    nonceCounter += 1;
    return `00000000-0000-4000-8000-${String(nonceCounter).padStart(
      12,
      '0'
    )}` as ReturnType<Crypto['randomUUID']>;
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('PluginFrame', () => {
  it('completes the happy handshake and delivers context plus grant over the port', async () => {
    const frame = await mountFrame();

    readyFrom(frame, frame.initNonces()[0]);
    await vi.waitFor(() => expect(channels).toHaveLength(1));

    const connect = frame.posted.ofType(PLUGIN_CONNECT_TYPE)[0];
    expect(connect).toEqual({
      type: PLUGIN_CONNECT_TYPE,
      nonce: frame.initNonces()[0],
    });
    const connectCall = frame.posted.calls.find(
      (call) => (call[0] as { type?: string }).type === PLUGIN_CONNECT_TYPE
    );
    expect(connectCall?.[2]).toEqual([channels[0].port2]);
    expect(channels[0].port1.postMessage).toHaveBeenCalledWith({
      version: PLUGIN_PROTOCOL_VERSION,
      type: PLUGIN_CONTEXT_TYPE,
      context: CONTEXT,
      grant: GRANT,
    });
    expect(frame.view.container.textContent).not.toContain('Loading plugin');
    frame.view.unmount();
  });

  it('rejects a ready message from a non-opaque origin', async () => {
    const frame = await mountFrame();
    readyFrom(frame, frame.initNonces()[0], 'http://localhost:3000');
    await Promise.resolve();
    expect(channels).toHaveLength(0);
    expect(frame.fetchGrant).not.toHaveBeenCalled();
    frame.view.unmount();
  });

  it('rejects a ready message with the wrong nonce', async () => {
    const frame = await mountFrame();
    readyFrom(frame, 'wrong-nonce');
    await Promise.resolve();
    expect(channels).toHaveLength(0);
    expect(frame.fetchGrant).not.toHaveBeenCalled();

    // The correct nonce still works afterwards: the bad one was not consumed.
    readyFrom(frame, frame.initNonces()[0]);
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    frame.view.unmount();
  });

  it('ignores stale-generation ready messages and honors only the fresh load', async () => {
    const frame = await mountFrame();
    const staleNonce = frame.initNonces()[0];

    // A second load models a fresh frame navigation with a new generation.
    fireEvent.load(frame.iframe);
    await vi.waitFor(() => expect(frame.initNonces()).toHaveLength(2));
    expect(frame.initNonces()[1]).not.toBe(staleNonce);

    readyFrom(frame, staleNonce);
    await Promise.resolve();
    expect(channels).toHaveLength(0);
    expect(frame.fetchGrant).not.toHaveBeenCalled();

    readyFrom(frame, frame.initNonces()[1]);
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    frame.view.unmount();
  });

  it('cleans up listeners and ignores delayed work after unmount', async () => {
    const removeSpy = vi.spyOn(window, 'removeEventListener');
    const grantGate = deferred<PluginGrant>();
    const frame = await mountFrame({
      fetchGrant: vi.fn().mockReturnValue(grantGate.promise),
    });
    readyFrom(frame, frame.initNonces()[0]);
    expect(frame.fetchGrant).toHaveBeenCalledTimes(1);
    frame.view.unmount();

    expect(removeSpy.mock.calls.some((call) => call[0] === 'message')).toBe(
      true
    );

    grantGate.resolve(GRANT);
    await Promise.resolve();
    await Promise.resolve();
    expect(channels).toHaveLength(0);
  });

  it('shows the error fallback on handshake timeout and retries into ready', async () => {
    const frame = await mountFrame({ timeoutMs: 20 });
    await vi.waitFor(() =>
      expect(frame.view.container.textContent).toContain(
        'did not respond in time'
      )
    );

    fireEvent.click(
      [...frame.view.container.querySelectorAll('button')].find(
        (button) => button.textContent === 'Retry'
      )!
    );
    await vi.waitFor(() =>
      expect(frame.initNonces().length).toBeGreaterThanOrEqual(2)
    );

    readyFrom(frame, frame.initNonces().at(-1)!);
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    expect(frame.view.container.textContent).not.toContain(
      'did not respond in time'
    );
    frame.view.unmount();
  });

  it('shows the error fallback when the grant fails', async () => {
    const frame = await mountFrame({
      fetchGrant: vi.fn().mockRejectedValue(new Error('grant boom')),
    });
    readyFrom(frame, frame.initNonces()[0]);
    await vi.waitFor(() =>
      expect(frame.view.container.textContent).toContain('failed to load')
    );
    frame.view.unmount();
  });

  it('remounts cleanly: a fresh instance completes its own handshake', async () => {
    const first = await mountFrame();
    first.view.unmount();
    channels = [];

    const second = await mountFrame();
    readyFrom(second, second.initNonces()[0]);
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    expect(second.fetchGrant).toHaveBeenCalledOnce();
    second.view.unmount();
  });

  it('tears down the old frame and remounts when the mount context changes', async () => {
    const [context, setContext] = createSignal<PluginClientContext>(CONTEXT);
    const fetchGrant = vi.fn().mockResolvedValue(GRANT);
    const view = render(() => (
      <PluginFrame
        title="Test plugin"
        srcdoc="<html><body>bootstrap</body></html>"
        context={context()}
        fetchGrant={fetchGrant}
      />
    ));
    const firstIframe = view.container.querySelector('iframe')!;
    const firstPost = capturePostMessage(firstIframe.contentWindow!);
    await vi.waitFor(() =>
      expect(firstPost.ofType(PLUGIN_INIT_TYPE)).toHaveLength(1)
    );
    readyFrom(
      firstIframe,
      firstPost.ofType(PLUGIN_INIT_TYPE)[0].nonce as string
    );
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    const firstChannel = channels[0];

    setContext({ projectId: 'project-2', capabilities: [] });
    await vi.waitFor(() => {
      const iframes = view.container.querySelectorAll('iframe');
      return iframes.length === 1 && iframes[0] !== firstIframe;
    });
    expect(firstChannel.port1.close).toHaveBeenCalled();

    const secondIframe = view.container.querySelector('iframe')!;
    const secondPost = capturePostMessage(secondIframe.contentWindow!);
    let secondNonce = '';
    await vi.waitFor(() => {
      secondNonce = secondPost.ofType(PLUGIN_INIT_TYPE)[0]?.nonce as string;
      expect(secondNonce).toBeTruthy();
    });
    readyFrom(secondIframe, secondNonce);
    await vi.waitFor(() => expect(channels).toHaveLength(2));
    expect(channels[1].port1.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        type: PLUGIN_CONTEXT_TYPE,
        context: { projectId: 'project-2', capabilities: [] },
      })
    );
    view.unmount();
  });
});

describe('PluginFrame host intents', () => {
  async function readyFrame(overrides?: Parameters<typeof mountFrame>[0]) {
    const frame = await mountFrame(overrides);
    readyFrom(frame, frame.initNonces()[0]);
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    return frame;
  }

  const postOnPort = (data: unknown) => {
    channels[0].port1.onmessage?.({ data } as MessageEvent);
  };

  it('delivers a valid typed intent to the onIntent callback', async () => {
    const onIntent = vi.fn();
    const frame = await readyFrame({ onIntent });

    postOnPort({
      version: PLUGIN_PROTOCOL_VERSION,
      type: PLUGIN_INTENT_TYPE,
      intent: { type: 'project.open', projectId: 'project-9' },
    });
    expect(onIntent).toHaveBeenCalledWith({
      type: 'project.open',
      projectId: 'project-9',
    });
    expect(frame.view.container.textContent).not.toContain('failed to load');
    frame.view.unmount();
  });

  it('delivers an entity.open intent with the entity reference intact', async () => {
    const onIntent = vi.fn();
    const frame = await readyFrame({ onIntent });

    postOnPort({
      version: PLUGIN_PROTOCOL_VERSION,
      type: PLUGIN_INTENT_TYPE,
      intent: {
        type: 'entity.open',
        entity: { type: 'document', id: 'doc-1' },
      },
    });
    expect(onIntent).toHaveBeenCalledWith({
      type: 'entity.open',
      entity: { type: 'document', id: 'doc-1' },
    });
    frame.view.unmount();
  });

  it('ignores malformed intents without tearing the session down', async () => {
    const onIntent = vi.fn();
    const frame = await readyFrame({ onIntent });
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});

    for (const data of [
      { version: PLUGIN_PROTOCOL_VERSION, type: PLUGIN_INTENT_TYPE },
      {
        version: PLUGIN_PROTOCOL_VERSION,
        type: PLUGIN_INTENT_TYPE,
        intent: { type: 'entity.open' },
      },
      {
        version: PLUGIN_PROTOCOL_VERSION,
        type: PLUGIN_INTENT_TYPE,
        intent: { type: 'window.close' },
      },
      { version: 0, type: PLUGIN_INTENT_TYPE, intent: null },
    ]) {
      postOnPort(data);
    }

    expect(onIntent).not.toHaveBeenCalled();
    expect(warnSpy).toHaveBeenCalledTimes(4);
    warnSpy.mockRestore();

    // The session is still healthy: a valid intent still arrives and the
    // frame never shows the error fallback.
    postOnPort({
      version: PLUGIN_PROTOCOL_VERSION,
      type: PLUGIN_INTENT_TYPE,
      intent: { type: 'project.open', projectId: 'project-2' },
    });
    expect(onIntent).toHaveBeenCalledWith({
      type: 'project.open',
      projectId: 'project-2',
    });
    expect(frame.view.container.textContent).not.toContain('failed to load');
    frame.view.unmount();
  });

  it('does not deliver intents after the generation is invalidated', async () => {
    const onIntent = vi.fn();
    const first = await readyFrame({ onIntent });
    first.view.unmount();

    postOnPort({
      version: PLUGIN_PROTOCOL_VERSION,
      type: PLUGIN_INTENT_TYPE,
      intent: { type: 'project.open', projectId: 'late' },
    });
    expect(onIntent).not.toHaveBeenCalled();
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
