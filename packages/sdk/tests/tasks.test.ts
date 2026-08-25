import { afterEach, describe, expect, test } from 'bun:test';
import { Macro } from '../src/macro';
import { parseTaskEvent } from '../src/entities/tasks/namespace';

const originalFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = originalFetch; });

const event = { event_id: 'event-1', event_type: 'task.created' as const, document_id: 'doc-1', project_id: 'project-1' };
const sse = (...values: unknown[]) => new Response(values.map((value) => `data: ${typeof value === 'string' ? value : JSON.stringify(value)}\n\n`).join(''), { status: 200, headers: { 'content-type': 'text/event-stream' } });

function macro() { return new Macro({ token: 'user-token', env: 'local', hosts: { storage: 'https://storage.example.test' } }); }

async function collect(source: AsyncIterable<unknown>) { const values = []; for await (const value of source) values.push(value); return values; }

describe('TaskNamespace.subscribe', () => {
  test('authenticates one fetch and yields multiple strict frames without recovery header', async () => {
    let request: Request | undefined; let count = 0;
    globalThis.fetch = (async (input: RequestInfo | URL) => { count++; request = input instanceof Request ? input : new Request(input); return sse(event, { ...event, event_id: 'event-2', event_type: 'task.updated' }); }) as unknown as typeof fetch;
    const values = await collect(macro().tasks.subscribe({ projectId: 'project-1' }));
    expect(values).toHaveLength(2); expect(count).toBe(1);
    expect(request?.headers.get('authorization')).toBe('Bearer user-token');
    expect(request?.headers.has('Last-Event-ID')).toBe(false);
  });

  test.each([
    ['string', 'plain string'], ['missing', { event_id: 'e', event_type: 'task.created', document_id: 'd' }],
    ['extra', { ...event, extra: true }], ['array', [event]], ['bad type', { ...event, event_type: 'task.deleted' }],
  ])('rejects malformed %s payload after one fetch', async (_label, payload) => {
    let count = 0; globalThis.fetch = (async () => { count++; return sse(payload); }) as unknown as typeof fetch;
    await expect(collect(macro().tasks.subscribe({ projectId: 'project-1' }))).rejects.toBeInstanceOf(TypeError);
    expect(count).toBe(1);
  });

  test('surfaces non-OK and network failures without retry', async () => {
    let count = 0; globalThis.fetch = (async () => { count++; return new Response('no', { status: 503 }); }) as unknown as typeof fetch;
    await expect(collect(macro().tasks.subscribe({ projectId: 'project-1' }))).rejects.toThrow('SSE failed'); expect(count).toBe(1);
    count = 0; globalThis.fetch = (async () => { count++; throw new Error('network down'); }) as unknown as typeof fetch;
    await expect(collect(macro().tasks.subscribe({ projectId: 'project-1' }))).rejects.toThrow('network down'); expect(count).toBe(1);
  });

  test('consumer break aborts the only request', async () => {
    let signal: AbortSignal | undefined; let count = 0;
    globalThis.fetch = (async (input: RequestInfo | URL) => { count++; const request = input instanceof Request ? input : new Request(input); signal = request.signal; return sse(event, { ...event, event_id: 'two' }); }) as unknown as typeof fetch;
    for await (const _value of macro().tasks.subscribe({ projectId: 'project-1' })) break;
    expect(count).toBe(1); expect(signal?.aborted).toBe(true);
  });
});

describe('task request sharing behavior', () => {
  test.each([[false, false], [undefined, true]] as const)('sends shareWithTeam %p as %p', async (provided, expected) => {
    let body: any; globalThis.fetch = (async (input: RequestInfo | URL) => { const request = input instanceof Request ? input : new Request(input); body = await request.json(); return new Response(JSON.stringify({ error: false, data: { documentId: 'doc' } }), { status: 200, headers: { 'content-type': 'application/json' } }); }) as unknown as typeof fetch;
    await macro().tasks.create({ name: 'task', ...(provided === undefined ? {} : { shareWithTeam: provided }) });
    expect(body.shareWithTeam).toBe(expected);
  });
});

test('runtime parser rejects nonobjects and accepts exact events', () => {
  expect(parseTaskEvent(event)).toEqual(event); expect(() => parseTaskEvent(null)).toThrow();
});
