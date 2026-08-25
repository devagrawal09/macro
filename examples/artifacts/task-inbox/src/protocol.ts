type NonceMessage = { type: string; nonce: string };

const exactNonceMessage = (value: unknown, type: string): value is NonceMessage => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  return keys.length === 2 && keys[0] === 'nonce' && keys[1] === 'type' &&
    record.type === type && typeof record.nonce === 'string' && record.nonce.length > 0;
};

const exactTokenMessage = (value: unknown, nonce: string): value is { type: string; nonce: string; token: string } => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  return keys.length === 3 && keys[0] === 'nonce' && keys[1] === 'token' && keys[2] === 'type' &&
    record.type === 'macro-task-inbox-token' && record.nonce === nonce &&
    typeof record.token === 'string' && record.token.length > 0;
};

/** Install the nonce-bound parent protocol and resolve the in-memory API token once. */
export function receiveTaskInboxToken(): Promise<string> {
  return new Promise((resolve) => {
    let currentNonce: string | null = null;
    let currentPort: MessagePort | undefined;
    const listener = (event: MessageEvent) => {
      if (event.source !== window.parent) return;
      if (exactNonceMessage(event.data, 'macro-task-inbox-init')) {
        currentPort?.close();
        currentPort = undefined;
        currentNonce = event.data.nonce;
        window.parent.postMessage({ type: 'macro-task-inbox-ready', nonce: currentNonce }, '*');
        return;
      }
      if (!exactNonceMessage(event.data, 'macro-task-inbox-grant') ||
          event.data.nonce !== currentNonce || event.ports.length !== 1) return;
      const nonce = currentNonce;
      const port = event.ports[0];
      currentPort?.close();
      currentPort = port;
      port.onmessage = (portEvent) => {
        if (!nonce || currentNonce !== nonce || !exactTokenMessage(portEvent.data, nonce)) return;
        const token = portEvent.data.token;
        currentNonce = null;
        currentPort = undefined;
        port.close();
        window.removeEventListener('message', listener);
        resolve(token);
      };
      port.start();
    };
    window.addEventListener('message', listener);
  });
}

export const protocolValidation = { exactNonceMessage, exactTokenMessage };
