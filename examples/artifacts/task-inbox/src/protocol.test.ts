import { describe, expect, test } from 'bun:test';
import { protocolValidation } from './protocol';

describe('nonce protocol validation', () => {
  test('accepts exact init and rejects extra keys', () => {
    expect(protocolValidation.exactNonceMessage({ type: 'macro-task-inbox-init', nonce: 'n' }, 'macro-task-inbox-init')).toBe(true);
    expect(protocolValidation.exactNonceMessage({ type: 'macro-task-inbox-init', nonce: 'n', token: 'secret' }, 'macro-task-inbox-init')).toBe(false);
  });
  test('binds token to nonce and exact keys', () => {
    expect(protocolValidation.exactTokenMessage({ type: 'macro-task-inbox-token', nonce: 'n', token: 't' }, 'n')).toBe(true);
    expect(protocolValidation.exactTokenMessage({ type: 'macro-task-inbox-token', nonce: 'old', token: 't' }, 'n')).toBe(false);
  });
});


test('accepts only parent/current-nonce grant and resolves exact port token', async () => {
  const originalWindow = globalThis.window;
  const sent: unknown[] = [];
  const parentChannel = new MessageChannel();
  const parent = parentChannel.port1;
  parent.postMessage = ((message: unknown) => { sent.push(message); }) as typeof parent.postMessage;
  const fakeWindow = new EventTarget() as EventTarget & { parent: MessagePort };
  fakeWindow.parent = parent;
  Object.defineProperty(globalThis, 'window', { configurable: true, value: fakeWindow });
  const dispatch = (source: unknown, data: unknown, ports: MessagePort[] = []) => {
    fakeWindow.dispatchEvent(new MessageEvent('message', {
      source: source as MessageEventSource,
      data,
      ports,
    }));
  };

  try {
    const tokenPromise = (await import('./protocol')).receiveTaskInboxToken();
    const wrongSource = new MessageChannel();
    dispatch(wrongSource.port1, { type: 'macro-task-inbox-init', nonce: 'wrong-source' });
    wrongSource.port1.close(); wrongSource.port2.close();
    dispatch(parent, { type: 'macro-task-inbox-init', nonce: 'old', extra: true });
    expect(sent).toHaveLength(0);

    dispatch(parent, { type: 'macro-task-inbox-init', nonce: 'current' });
    expect(sent).toEqual([{ type: 'macro-task-inbox-ready', nonce: 'current' }]);

    const wrong = new MessageChannel();
    dispatch(parent, { type: 'macro-task-inbox-grant', nonce: 'old' }, [wrong.port2]);
    wrong.port1.postMessage({ type: 'macro-task-inbox-token', nonce: 'old', token: 'wrong' });

    const channel = new MessageChannel();
    dispatch(parent, { type: 'macro-task-inbox-grant', nonce: 'current' }, [channel.port2]);
    channel.port1.postMessage({ type: 'macro-task-inbox-token', nonce: 'old', token: 'wrong' });
    channel.port1.postMessage({ type: 'macro-task-inbox-token', nonce: 'current', token: 'secret' });
    expect(await tokenPromise).toBe('secret');
    wrong.port1.close();
    channel.port1.close();
    parentChannel.port1.close(); parentChannel.port2.close();
  } finally {
    Object.defineProperty(globalThis, 'window', { configurable: true, value: originalWindow });
  }
});
