/** @vitest-environment jsdom */

import { fireEvent, render } from '@solidjs/testing-library';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  metadata: { documentId: 'fixed-doc' } as { documentId?: string },
  getToken: vi.fn<() => Promise<string>>(),
  local: true,
  develop: false,
}));

vi.mock('@core/constant/featureFlags', () => ({
  get LOCAL_ONLY() { return mocks.local; },
  get DEV_MODE_ENV() { return mocks.develop; },
}));
vi.mock('@core/signal/load', () => ({
  blockTextSignal: { get: () => '<p>artifact</p>' },
  blockMetadataSignal: () => mocks.metadata,
}));
vi.mock('@service-auth/fetch', () => ({ getMacroApiToken: mocks.getToken }));

import { HtmlPreview, taskInboxGate } from './HtmlPreview';

class FakePort {
  postMessage = vi.fn();
  close = vi.fn();
}
class FakeChannel {
  port1 = new FakePort() as unknown as MessagePort;
  port2 = new FakePort() as unknown as MessagePort;
}
let channels: FakeChannel[];

const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

function ready(source: Window, nonce: string, extra?: object) {
  window.dispatchEvent(new MessageEvent('message', {
    source,
    data: { type: 'macro-task-inbox-ready', nonce, ...extra },
  }));
}

function mountConfigured() {
  vi.stubEnv('VITE_TASK_INBOX_DOCUMENT_ID', 'fixed-doc');
  const view = render(() => <HtmlPreview />);
  const iframe = view.container.querySelector('iframe')!;
  const postMessage = vi.spyOn(iframe.contentWindow!, 'postMessage');
  fireEvent.load(iframe);
  const init = postMessage.mock.calls.find((call) => (call[0] as any).type === 'macro-task-inbox-init')![0] as { type: string; nonce: string };
  return { ...view, iframe, postMessage, init };
}

beforeEach(() => {
  mocks.metadata = { documentId: 'fixed-doc' };
  mocks.local = true;
  mocks.develop = false;
  mocks.getToken.mockReset().mockResolvedValue('secret-token');
  channels = [];
  vi.stubGlobal('MessageChannel', class {
    port1: MessagePort; port2: MessagePort;
    constructor() { const channel = new FakeChannel(); channels.push(channel); this.port1 = channel.port1; this.port2 = channel.port2; }
  });
  let nonceCounter = 0;
  vi.spyOn(crypto, 'randomUUID').mockImplementation(() => {
    nonceCounter += 1;
    return `00000000-0000-4000-8000-${String(nonceCounter).padStart(12, '0')}` as ReturnType<Crypto['randomUUID']>;
  });
});

describe('task inbox host gate', () => {
  it('fails closed in production, without config, and for a different document', () => {
    expect(taskInboxGate({ local: false, develop: false, configuredDocumentId: 'fixed', currentDocumentId: 'fixed' })).toBe(false);
    expect(taskInboxGate({ local: true, develop: false, configuredDocumentId: '', currentDocumentId: '' })).toBe(false);
    expect(taskInboxGate({ local: false, develop: true, configuredDocumentId: 'fixed', currentDocumentId: 'other' })).toBe(false);
    expect(taskInboxGate({ local: false, develop: true, configuredDocumentId: 'fixed', currentDocumentId: 'fixed' })).toBe(true);
  });

  it('mounts fail-closed in Production and initializes in Develop', async () => {
    vi.stubEnv('VITE_TASK_INBOX_DOCUMENT_ID', 'fixed-doc');
    mocks.local = false;
    mocks.develop = false;
    const production = render(() => <HtmlPreview />);
    const productionIframe = production.container.querySelector('iframe')!;
    const productionPost = vi.spyOn(productionIframe.contentWindow!, 'postMessage');
    fireEvent.load(productionIframe);
    expect(productionPost.mock.calls.some((call) => (call[0] as any).type === 'macro-task-inbox-init')).toBe(false);
    expect(mocks.getToken).not.toHaveBeenCalled();
    production.unmount();

    mocks.develop = true;
    const develop = render(() => <HtmlPreview />);
    const developIframe = develop.container.querySelector('iframe')!;
    const developPost = vi.spyOn(developIframe.contentWindow!, 'postMessage');
    fireEvent.load(developIframe);
    const init = developPost.mock.calls.find((call) => (call[0] as any).type === 'macro-task-inbox-init')![0] as { nonce: string };
    ready(developIframe.contentWindow!, init.nonce);
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    expect(mocks.getToken).toHaveBeenCalledOnce();
    develop.unmount();
  });

  it('does not initialize when external config is absent or mismatched', () => {
    for (const configuredId of ['', 'other']) {
      vi.stubEnv('VITE_TASK_INBOX_DOCUMENT_ID', configuredId);
      const view = render(() => <HtmlPreview />);
      const iframe = view.container.querySelector('iframe')!;
      const postMessage = vi.spyOn(iframe.contentWindow!, 'postMessage');
      fireEvent.load(iframe);
      expect(postMessage.mock.calls.some((call) => (call[0] as any).type === 'macro-task-inbox-init')).toBe(false);
      expect(mocks.getToken).not.toHaveBeenCalled();
      view.unmount();
    }
  });

  it('uses exact token-free nonce-bound init, grant, and port token payloads', async () => {
    const view = mountConfigured();
    expect(view.init).toEqual({ type: 'macro-task-inbox-init', nonce: view.init.nonce });
    ready(view.iframe.contentWindow!, view.init.nonce);
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    const grant = view.postMessage.mock.calls.find((call) => (call[0] as any).type === 'macro-task-inbox-grant')!;
    expect(grant[0]).toEqual({ type: 'macro-task-inbox-grant', nonce: view.init.nonce });
    expect((grant as unknown as unknown[])[2]).toEqual([channels[0].port2]);
    expect(channels[0].port1.postMessage).toHaveBeenCalledWith({ type: 'macro-task-inbox-token', nonce: view.init.nonce, token: 'secret-token' });
    expect(channels[0].port1.close).toHaveBeenCalledOnce();
    expect(JSON.stringify(view.init)).not.toContain('secret-token'); expect(JSON.stringify(grant[0])).not.toContain('secret-token');
    view.unmount();
  });

  it('rejects malformed, excess-key, wrong-source, and wrong-nonce ready messages', async () => {
    const view = mountConfigured();
    ready(window, view.init.nonce); ready(view.iframe.contentWindow!, 'wrong'); ready(view.iframe.contentWindow!, view.init.nonce, { extra: true });
    window.dispatchEvent(new MessageEvent('message', { source: view.iframe.contentWindow, data: 'ready' }));
    window.dispatchEvent(new MessageEvent('message', { source: view.iframe.contentWindow, data: { type: 'macro-task-inbox-ready' } }));
    await Promise.resolve(); expect(mocks.getToken).not.toHaveBeenCalled(); expect(channels).toHaveLength(0); view.unmount();
  });

  it('claims duplicate ready synchronously before the token await', async () => {
    const token = deferred<string>(); mocks.getToken.mockReturnValue(token.promise);
    const view = mountConfigured(); ready(view.iframe.contentWindow!, view.init.nonce); ready(view.iframe.contentWindow!, view.init.nonce);
    expect(mocks.getToken).toHaveBeenCalledOnce(); token.resolve('secret-token');
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    expect(view.postMessage.mock.calls.filter((call) => (call[0] as any).type === 'macro-task-inbox-grant')).toHaveLength(1);
    view.unmount();
  });

  it('invalidates delayed old generation and grants only the fresh load', async () => {
    const oldToken = deferred<string>(); mocks.getToken.mockReturnValueOnce(oldToken.promise).mockResolvedValueOnce('fresh-token');
    const view = mountConfigured(); const oldNonce = view.init.nonce; ready(view.iframe.contentWindow!, oldNonce);
    fireEvent.load(view.iframe);
    oldToken.resolve('old-token');
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(channels).toHaveLength(0);
    const inits = view.postMessage.mock.calls.filter((call) => (call[0] as any).type === 'macro-task-inbox-init');
    expect(inits.length).toBeGreaterThan(1);
    const freshNonce = (inits.at(-1)![0] as any).nonce as string;
    expect(freshNonce).not.toBe(oldNonce);
    ready(view.iframe.contentWindow!, oldNonce);
    expect(mocks.getToken).toHaveBeenCalledOnce();
    ready(view.iframe.contentWindow!, freshNonce);
    await vi.waitFor(() => expect(mocks.getToken).toHaveBeenCalledTimes(2));
    await vi.waitFor(() => expect(channels).toHaveLength(1));
    expect(channels[0].port1.postMessage).toHaveBeenCalledWith({ type: 'macro-task-inbox-token', nonce: freshNonce, token: 'fresh-token' });
    view.unmount();
  });

  it('keeps token failure claimed and aborts on post-await gate change', async () => {
    mocks.getToken.mockRejectedValueOnce(new Error('failed'));
    const failed = mountConfigured(); ready(failed.iframe.contentWindow!, failed.init.nonce); await Promise.resolve(); ready(failed.iframe.contentWindow!, failed.init.nonce); expect(mocks.getToken).toHaveBeenCalledOnce(); failed.unmount();

    const token = deferred<string>(); mocks.getToken.mockReset().mockReturnValue(token.promise);
    const stale = mountConfigured(); ready(stale.iframe.contentWindow!, stale.init.nonce); mocks.metadata = { documentId: 'other' }; token.resolve('secret'); await Promise.resolve(); await Promise.resolve(); expect(channels).toHaveLength(0); stale.unmount();
  });

  it('aborts a delayed grant on cleanup', async () => {
    const token = deferred<string>(); mocks.getToken.mockReturnValue(token.promise);
    const view = mountConfigured(); ready(view.iframe.contentWindow!, view.init.nonce);
    view.unmount(); token.resolve('secret'); await Promise.resolve(); await Promise.resolve();
    expect(channels).toHaveLength(0);
  });

});
