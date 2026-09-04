import { match, P } from 'ts-pattern';
import type {
  WebhookFilter,
  WebhookScope,
} from '../../generated/storage/types.gen';
import { MacroError } from '../utils';
import type { MacroClient } from '../utils/client';
import { hydrateChannelEvent } from './hydrate/channel';
import { hydrateDocumentEvent } from './hydrate/document';
import type {
  DeliveryHeaders,
  EventHandler,
  EventMap,
  EventName,
  MacroEvent,
} from './types';
import { verifySignature } from './verify';

type AnyHandler = (event: unknown) => void | Promise<void>;

/** Sent by Macro when validating a newly registered endpoint; acked, never dispatched. */
const VALIDATION_EVENT = 'webhook.validation.test';

/** Event/entity filter accepted by both SSE and persisted webhooks. */
export type EventFilter = Omit<WebhookFilter, 'events'> & {
  events: readonly EventName[];
};

/** Options for {@link MacroEvents.connect}. */
export interface ConnectEventsOptions {
  /**
   * Event/entity-id filters, identical to persisted webhook `filters`.
   * Defaults to one filter covering every event currently registered with
   * {@link MacroEvents.on}.
   */
  filters?: readonly EventFilter[];
  /**
   * Personal or team workspace whose webhook lifecycle events are delivered.
   * Defaults to `'user'`.
   */
  scope?: WebhookScope;
  /** Abort the stream. */
  signal?: AbortSignal;
  /** Called for connection, parsing, and event-handler errors. */
  onError?: (error: unknown) => void;
  /** Delay before reconnecting after a closed stream. Defaults to 1 second. */
  reconnectDelayMs?: number;
}

/** Options accepted by the backwards-compatible {@link MacroEvents.listen}. */
export type ListenOptions = ConnectEventsOptions;

/** A live, best-effort event stream owned by one Macro client. */
export interface EventConnection {
  /** Resolves when this connection handle closes. */
  readonly closed: Promise<void>;
  /** Release this handle. The underlying stream closes after its last handle. */
  close(): void;
}

interface SharedEventConnection {
  readonly controller: AbortController;
  readonly errorHandlers: Set<(error: unknown) => void>;
  closed: Promise<void>;
  references: number;
}

const DEFAULT_RECONNECT_DELAY_MS = 1_000;

function serializeFilters(filters: readonly EventFilter[]): string {
  return JSON.stringify(filters, (_key, value: unknown) => {
    if (value === null || typeof value !== 'object' || Array.isArray(value)) {
      return value;
    }
    return Object.fromEntries(
      Object.entries(value).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    );
  });
}

/** Attach the entity handles defined for each webhook event. */
function hydrate(
  client: MacroClient,
  event: MacroEvent,
): EventMap[EventName] | undefined {
  return match(event)
    .with({ event_type: P.string.startsWith('document.') }, (documentEvent) =>
      hydrateDocumentEvent(client, documentEvent),
    )
    .with({ event_type: P.string.startsWith('channel.') }, (channelEvent) =>
      hydrateChannelEvent(client, channelEvent),
    )
    .otherwise(() => undefined);
}

/**
 * Per-instance event receiver. Subscribe with {@link MacroEvents.on}, then
 * either {@link MacroEvents.connect} (SSE, the default) or mount
 * {@link MacroEvents.webhook} at a persisted webhook URL.
 *
 * Obtain via `macro.events` — do not construct directly.
 */
export class MacroEvents {
  private readonly handlers = new Map<EventName, Set<AnyHandler>>();
  private readonly connections = new Map<string, SharedEventConnection>();

  constructor(
    private readonly client: MacroClient,
    private readonly secret?: string,
  ) {}

  /**
   * Subscribe to an event across all entities.
   *
   * @returns An unsubscribe function.
   */
  on<E extends EventName>(event: E, handler: EventHandler<E>): () => void {
    const set = this.handlers.get(event) ?? new Set<AnyHandler>();
    set.add(handler as AnyHandler);
    this.handlers.set(event, set);
    return () => set.delete(handler as AnyHandler);
  }

  /**
   * Subscribe to @-mentions of the authenticated caller: `channel.mentioned`
   * deliveries whose mentioned entity is this bot (bot auth) or this user
   * (user auth).
   *
   * `channel.mentioned` deliveries cover every mention in channels the
   * stream's (or webhook's) workspace can access (its `ids` filter, like all
   * channel events, holds channel ids); picking out "me" happens here,
   * client-side. The caller's identity is resolved lazily (once) on the first
   * delivery.
   *
   * For SSE, register this handler before {@link connect} so the derived
   * filters include `channel.mentioned`. For persisted webhooks, register the
   * webhook separately, e.g. `macro.webhooks.create({ filters: [{ events:
   * ['channel.mentioned'] }], … })`.
   *
   * @returns An unsubscribe function.
   */
  onSelfMention(handler: EventHandler<'channel.mentioned'>): () => void {
    return this.on('channel.mentioned', async (event) => {
      // Principals embed emails for users; emails are case-insensitive.
      const mentioned = event.metadata.mentioned.entity_id.toLowerCase();
      if (mentioned !== (await this.client.myPrincipalId()).toLowerCase()) {
        return;
      }
      await handler(event);
    });
  }

  /**
   * Open a live Server-Sent Events stream of matching broker events. No public
   * URL or signing secret is required. Delivery is best-effort: events sent
   * before connection, while disconnected, or after overflow are not replayed.
   *
   * Equal subscriptions on one Macro instance share an underlying request.
   * Filters are fixed for the connection lifetime and default to event names
   * already registered with {@link on}.
   */
  connect(opts: ConnectEventsOptions = {}): EventConnection {
    const filters = opts.filters ?? this.filtersFromHandlers();
    if (
      filters.length === 0 ||
      filters.every((filter) => filter.events.length === 0)
    ) {
      throw new MacroError(
        'connect() needs filters - pass filters or register handlers with .on() first',
      );
    }

    const reconnectDelayMs =
      opts.reconnectDelayMs ?? DEFAULT_RECONNECT_DELAY_MS;
    if (!Number.isFinite(reconnectDelayMs) || reconnectDelayMs < 0) {
      throw new MacroError('reconnectDelayMs must be a non-negative number');
    }

    if (opts.signal?.aborted) {
      return { closed: Promise.resolve(), close() {} };
    }

    const scope = opts.scope ?? 'user';
    const serializedFilters = serializeFilters(filters);
    const key = JSON.stringify({
      filters: serializedFilters,
      reconnectDelayMs,
      scope,
    });
    let shared = this.connections.get(key);
    if (!shared || shared.controller.signal.aborted) {
      const controller = new AbortController();
      shared = {
        controller,
        errorHandlers: new Set(),
        references: 0,
        closed: Promise.resolve(),
      };
      const current = shared;
      shared.closed = this.consumeConnection(
        scope,
        serializedFilters,
        reconnectDelayMs,
        shared,
      ).finally(() => {
        if (this.connections.get(key) === current) {
          this.connections.delete(key);
        }
        current.errorHandlers.clear();
      });
      this.connections.set(key, shared);
    }

    shared.references += 1;
    const errorHandler = opts.onError
      ? (error: unknown) => opts.onError?.(error)
      : undefined;
    if (errorHandler) shared.errorHandlers.add(errorHandler);

    let isClosed = false;
    let resolveClosed: () => void = () => {};
    const closed = new Promise<void>((resolve) => {
      resolveClosed = resolve;
    });
    const connection = shared;
    const close = () => {
      if (isClosed) return;
      isClosed = true;
      opts.signal?.removeEventListener('abort', close);
      if (errorHandler) connection.errorHandlers.delete(errorHandler);
      connection.references -= 1;
      if (connection.references === 0) connection.controller.abort();
      resolveClosed();
    };
    opts.signal?.addEventListener('abort', close, { once: true });
    void connection.closed.then(close);

    return { closed, close };
  }

  /**
   * Open an event stream and return its close function.
   *
   * @deprecated Use {@link connect} to observe closure and connection errors.
   */
  async listen(opts: ListenOptions = {}): Promise<() => void> {
    return this.connect(opts).close;
  }

  /**
   * Feed a raw webhook delivery in: verifies the signature, parses, and
   * dispatches to matching handlers.
   *
   * @throws {MacroError} if no signing secret was configured, or if the
   *   signature is missing or invalid.
   */
  async handle(rawBody: string, headers: DeliveryHeaders): Promise<void> {
    if (!this.secret) {
      throw new MacroError(
        'webhookSecret is required to verify incoming webhook deliveries',
      );
    }
    const ok = await verifySignature({
      secret: this.secret,
      timestamp: headers.timestamp ?? '',
      rawBody,
      signature: headers.signature ?? '',
    });
    if (!ok) throw new MacroError('invalid webhook signature');

    if (headers.event === VALIDATION_EVENT) return;

    await this.dispatchEvent(JSON.parse(rawBody));
  }

  /**
   * A Fetch-style handler to mount at your persisted webhook route.
   *
   * Requires `webhookSecret` (or `MACRO_WEBHOOK_SECRET`).
   *
   * @example
   * app.post('/webhook', macro.events.webhook()); // Hono
   */
  webhook(): (req: Request) => Promise<Response> {
    if (!this.secret) {
      throw new MacroError(
        'webhookSecret is required to verify incoming webhook deliveries',
      );
    }
    return async (req: Request) => {
      await this.handle(await req.text(), {
        event: req.headers.get('x-macro-event') ?? undefined,
        eventId: req.headers.get('x-macro-event-id') ?? undefined,
        timestamp: req.headers.get('x-macro-timestamp') ?? undefined,
        signature: req.headers.get('x-macro-signature') ?? undefined,
      });
      return new Response('ok');
    };
  }

  private async consumeConnection(
    scope: WebhookScope,
    filters: string,
    reconnectDelayMs: number,
    connection: SharedEventConnection,
  ): Promise<void> {
    const { signal } = connection.controller;
    while (!signal.aborted) {
      try {
        const { stream } = await this.client.storage.streamEvents({
          query: { scope, filters },
          signal,
          sseMaxRetryAttempts: 1,
          onSseError: (error) => this.reportError(connection, error),
        });
        for await (const data of stream) {
          try {
            await this.dispatchEvent(data);
          } catch (error) {
            this.reportError(connection, error);
          }
        }
      } catch (error) {
        if (!signal.aborted) this.reportError(connection, error);
      }

      if (!signal.aborted) {
        await new Promise<void>((resolve) => {
          const finish = () => {
            clearTimeout(timeout);
            signal.removeEventListener('abort', finish);
            resolve();
          };
          const timeout = setTimeout(finish, reconnectDelayMs);
          signal.addEventListener('abort', finish, { once: true });
          if (signal.aborted) finish();
        });
      }
    }
  }

  private reportError(connection: SharedEventConnection, error: unknown): void {
    for (const handler of connection.errorHandlers) {
      try {
        handler(error);
      } catch {
        // Error observers must not terminate the shared stream.
      }
    }
  }

  private filtersFromHandlers(): EventFilter[] {
    const events = [...this.handlers.entries()]
      .filter(([, set]) => set.size > 0)
      .map(([name]) => name);
    return events.length > 0 ? [{ events }] : [];
  }

  private async dispatchEvent(data: unknown): Promise<void> {
    const event =
      typeof data === 'string' ? (JSON.parse(data) as unknown) : data;
    if (
      event === null ||
      typeof event !== 'object' ||
      !('event_type' in event) ||
      typeof event.event_type !== 'string' ||
      !('event_id' in event) ||
      typeof event.event_id !== 'string' ||
      event.event_id.length === 0 ||
      !('schema_version' in event) ||
      typeof event.schema_version !== 'number' ||
      !Number.isInteger(event.schema_version) ||
      !('metadata' in event) ||
      event.metadata === null ||
      typeof event.metadata !== 'object'
    )
      throw new MacroError('invalid event payload');
    const typed = event as MacroEvent;
    const handlers = this.handlers.get(typed.event_type);
    if (!handlers || handlers.size === 0) return;

    const payload = hydrate(this.client, typed);
    if (!payload) return;
    await Promise.all([...handlers].map((handler) => handler(payload)));
  }
}
