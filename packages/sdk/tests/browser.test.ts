import { afterEach, describe, expect, test } from 'bun:test';
import { Macro } from '../src/macro.browser';

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

describe('browser SDK', () => {
  test('requires explicit authentication', () => {
    expect(() => new Macro({})).toThrow(
      'browser clients must pass token or auth',
    );
  });

  test('dispatches the same hydrated events over authenticated SSE', async () => {
    let request: Request | undefined;
    let tokenCalls = 0;
    const delivered = Promise.withResolvers<{ documentId: string }>();
    const event = {
      event_id: '01990f1d-a222-7000-8000-000000000002',
      schema_version: 1,
      event_type: 'document.updated' as const,
      metadata: {
        document_id: 'doc_browser',
        document_name: 'Browser document',
        owner: 'macro|owner@example.com',
      },
    };

    globalThis.fetch = (async (input) => {
      request = input instanceof Request ? input : new Request(input);
      return new Response(
        `event: document.updated\ndata: ${JSON.stringify(event)}\n\n`,
        {
          status: 200,
          headers: { 'content-type': 'text/event-stream' },
        },
      );
    }) as typeof fetch;

    const macro = new Macro({
      auth: {
        type: 'user',
        token: () => {
          tokenCalls += 1;
          return 'browser-token';
        },
      },
      hosts: { storage: 'https://storage.example.test' },
    });
    macro.events.on('document.updated', (received) => {
      delivered.resolve({
        documentId: received.document.id,
      });
    });

    const stop = await macro.events.listen();
    await expect(delivered.promise).resolves.toEqual({
      documentId: event.metadata.document_id,
    });
    stop();

    expect(tokenCalls).toBe(1);
    expect(request?.headers.get('authorization')).toBe('Bearer browser-token');
  });
});
