# Macro Customization Research and Plan

Last updated: 2026-09-04

## Purpose

This file records research and planning prompted by **customizing-macro.md**. It focuses on whether Macro can avoid building a plugin control plane, how self-hosted JavaScript automations and browser extensions change the product boundary, and what the Macro SDK needs for one event-handler API over webhooks and SSE.

## Current Direction

The strongest one-day direction is not a managed plugin platform. It is:

1. **Macro SDK**: the stable, typed API for reading and mutating Macro data.
2. **Macro Events**: one typed handler API with webhook and SSE transports.
3. **Server customization**: ordinary user-hosted TypeScript/JavaScript using the SDK. No Macro-owned execution runtime.
4. **Client customization**: ordinary browser-extension code or standalone applications that bundle the browser SDK and optional sync package from npm. No Macro-owned bundle deployment or plugin control plane.
5. **Authoring helpers**: optional client contribution descriptors and framework adapters only where they encode a real host contract. No server-plugin wrapper is needed.

This vaporizes the **managed runtime and control plane** from the MVP. It does not vaporize the public extension contracts Macro must support.

## Preliminary Conclusion

### What can be removed

- Server bundle upload, build, deployment, process management, sandboxing, scheduling, retries, logs, versions, rollbacks, secrets, and billing.
- Client bundle upload, remote-module hosting, install state, enable/disable state, release management, and an in-product registry.
- A unified definePlugin() manifest whose main purpose is to describe artifacts Macro will deploy.
- The current demo's local executable runner and simulated in-Macro state bridge, if the new demo uses the real SDK and real event delivery.

### What remains necessary

- Stable API credentials suitable for unattended server code and browser-based user code.
- Event subscription registration, filtering, authorization, ordering semantics, reconnect behavior, and event schema evolution.
- Server-enforced scopes if capability restrictions are promised. Type narrowing alone is not security.
- A browser-safe SDK with token refresh and CORS support.
- A supported client integration contract. A browser extension can own installation and code delivery, but DOM scraping/injection is not a stable client plugin API.
- Documentation and examples that make external code feel cohesive without claiming Macro hosts it.

### Terminology recommendation

Avoid calling arbitrary self-hosted SDK code a "server plugin" for now. Call it a **Macro automation** or **SDK application**. Reserve **client extension** for UI integrated by a browser extension. "Plugin platform" should describe a future managed product, not today's SDK capability.

## Document Analysis

### Strong ideas

- The document correctly recognizes that server handlers are normal JavaScript plus credentials and do not require a Macro execution environment.
- Self-hosting lets developers use any libraries, databases, secrets, queues, and external services without Macro becoming a hosting platform.
- Treating the SDK as the customization ceiling is strategically sound. Dogfooding it in first-party features should expose missing APIs early.
- Deferring a first-party registry and managed hosting avoids a large operational and security project before demand is proven.
- A standalone Solid app is a useful proof that client code is portable and not intrinsically tied to Macro's deployment system.

### Ideas that need correction or sharper wording

- Browser extensions avoid a Macro **backend** control plane, but seamless page/sidebar insertion still requires either stable frontend extension points or brittle DOM manipulation.
- A browser extension is not automatically isolated. Content scripts have an isolated JavaScript world, but injected UI still shares page DOM/CSS unless it uses Shadow DOM or an iframe.
- createClientPlugin() returning only a Solid component is insufficient for browser-extension integration. The extension also needs placement/context contracts and a host-to-extension communication protocol.
- Capability-based TypeScript narrowing is useful ergonomics, but runtime enforcement requires credentials whose server-side authorization is actually scoped. A client-provided capability list cannot enforce itself.
- SSE does not replace webhooks in every deployment. It is excellent for local development, browser clients, and continuously running workers; webhooks remain better for stateless/public server infrastructure.
- "Exact same event handler API" is achievable for handler registration and payloads, not for delivery guarantees. Webhook retries and SSE reconnect/replay have different acknowledgement semantics and must be documented.

## Existing SDK Findings

### Package shape

- **packages/sdk/package.json** exports a Node/server entrypoint as @macro/sdk and a browser entrypoint as @macro/sdk/browser.
- The browser entrypoint requires explicit token or auth; it never reads environment variables.
- The browser entrypoint currently exposes data namespaces, including webhooks management, but deliberately does not expose the inbound event receiver.
- MacroOpts already accepts dynamic token sources, explicit service hosts, environment selection, and a wsVerify field.

### Existing event API

- **packages/sdk/src/events/types.ts** derives EventName and payloads from the backend-generated WebhookEvent OpenAPI union.
- Handlers receive hydrated entity handles, not only raw IDs. Hydration is lazy and transport-independent in concept.
- **packages/sdk/src/events/receiver.ts** owns a handler map and provides macro.events.on(event, handler) and onSelfMention(handler).
- MacroEvents.handle(rawBody, headers) currently combines signature verification, JSON parsing, hydration, and dispatch.
- MacroEvents.webhook() is only a Fetch-style adapter around handle().
- The Node Macro class exposes events only when a webhook secret is configured at the type level.
- The browser Macro class has no events property.

### Immediate design implication

The event dispatcher should be extracted from webhook verification. Both webhook and SSE transports can feed the same typed dispatch method:

~~~ts
macro.events.on('channel.message_posted', async ({ message }) => {
  await message.reply('Received');
});

// Server with public ingress
app.post('/webhook', macro.events.webhook());

// Browser, local process, or long-running worker
const connection = macro.events.connect();
await connection.closed;
~~~

The on(...) call and hydrated payload stay identical. Only startup and delivery semantics differ.

## Completed Repository Research

### A managed Plugin Platform already exists in this checkout

The direction is not merely avoiding hypothetical work. The checkout contains an early managed-platform implementation:

- **crates/plugin_platform**: manifests, releases, installations, grants, capability intersection, settings, and runtime ports.
- **services/plugin_http_service**: a dev-only HTTP control surface for manifest admission, invocation, and settings.
- **services/plugin-runtime**: a Bun process that executes admitted server bundles with timeout and concurrency controls.
- **packages/plugin**: definePlugin(), project-page, entity-side-panel, and best-effort server-event authoring types.
- **packages/plugin-cli**: check, build, and release-oriented tooling.
- **packages/plugin-ui**: a small isolated UI component package.
- **apps/web/src/features/plugins**: iframe bootstrap, MessageChannel protocol, host session, and demo/settings UI.
- **crates/macro_db_client/migrations/20260826020912_create_plugin_platform_core**: immutable releases and installations.

Under the userspace direction, most of this is no longer on the critical path. No deletion should happen until the direction is approved, but the intended disposition is:

| Area | Recommendation | Reason |
| --- | --- | --- |
| Rust control plane and plugin HTTP service | Remove or park | Exists to admit, install, grant, and launch Macro-hosted artifacts. |
| Bun server runtime | Remove or park | Self-hosted SDK applications use their own runtime. |
| Release/install database tables | Remove if never shipped; otherwise deprecate with a migration | Browser stores and user infrastructure own distribution and lifecycle. |
| Server entrypoints in packages/plugin | Remove | macro.events.on() is already the better API. |
| Plugin CLI release/deploy flow | Remove or reduce to examples/scaffolding | There is no Macro deployment target. |
| Client contribution descriptors | Keep only if backed by a stable browser-extension host bridge | Page/sidebar placement still needs a contract. |
| Iframe host protocol | Optional | Useful for strong UI isolation, but not required if an extension uses Shadow DOM. |
| packages/plugin-ui | Keep only as an explicitly supported, dogfooded design system | It has value independent of hosting. |
| packages/sdk | Keep and expand | This becomes the actual customization platform. |

### Webhook event pipeline

- Webhook event bodies originate in the shared Kafka event broker. Event envelopes contain event_id, schema_version, event_type, and metadata.
- The webhook ingestion service currently handles document and channel events, resolves users with entity access, maps them to personal/team workspaces, matches active webhook filters, and enqueues one delivery per matching webhook.
- Existing filters are an array of event-name lists plus optional entity-id lists. This is a useful starting contract for SSE.
- HTTP deliveries are signed and include X-Macro-Event, X-Macro-Event-Id, X-Macro-Timestamp, and X-Macro-Signature.
- Deliveries are persisted and retryable, with up to five HTTP attempts. Consumers must therefore tolerate duplicates.
- Runtime webhook bodies contain the flattened broker envelope, but the SDK's generated WebhookEvent type currently models only event_type and metadata. The SDK therefore drops event_id and schema_version from its public handler type even though they are present on the wire. Fix this before declaring a transport-neutral event contract.

### Existing SSE support

There is already a browser-compatible SSE path for tasks:

- **crates/projects/src/inbound/axum_router/task_events.rs** exposes authenticated text/event-stream responses.
- **crates/documents/src/domain/task_events.rs** describes this as process-local, best-effort, and non-replaying.
- **packages/sdk/src/entities/tasks/namespace.ts** exposes TaskNamespace.subscribe() as an AsyncIterable.
- The generated HeyAPI SSE client uses fetch, supports request interceptors and Authorization headers, parses event streams, and has Last-Event-ID/retry machinery.
- Both @macro/sdk and @macro/sdk/browser include TaskNamespace, so this path already runs in browsers.

This precedent proves the SDK and transport mechanics. It is not a generic solution because it covers only task.created/task.updated, filters only one project, uses an in-memory bus, has no replay, and cannot fan out channel/document Kafka events across service replicas.

### Connection gateway

Macro already has an authenticated WebSocket gateway used by the frontend. It has reconnect, heartbeat, and entity tracking behavior, but its messages are frontend-specific and are not the WebhookEvent contract. Reusing it is possible only after adding a domain-event-to-authorized-principal fan-out path. It is not a zero-work substitute for the requested SSE endpoint.

### Browser package and authentication

- @macro/sdk/browser already exists.
- Its build targets browsers, rejects process.env, node: imports, and MACRO_API_KEY leakage, and requires an explicit token source.
- MacroClientCore already applies a dynamic token source to generated clients. This is suitable for refreshable browser credentials and authenticated fetch-based SSE.
- There is no discovered public OAuth 2.1 Authorization Code + PKCE flow for third-party Macro applications.
- Current user Macro API tokens are broad bearer credentials. Bot keys support user/team access context, not the method-level capabilities described in the customization document.
- The current production CORS allowlist includes Macro web origins, Tauri origins, and selected development origins. It does not generally allow arbitrary hosted Solid applications or chrome-extension origins.

## Can The Plugin Platform Be Vaporized?

### Short answer

**The managed platform can be vaporized from the near-term architecture. The extension surface cannot.**

Server-side code needs no plugin abstraction or Macro runtime. It is an ordinary SDK application:

~~~ts
const macro = new Macro({ auth: { type: 'bot', token } });

macro.events.on('channel.message_posted', async ({ message }) => {
  await macro.tasks.create({ name: await message.content() });
});

await macro.events.connect({
  filters: [{ events: ['channel.message_posted'] }],
});
~~~

That code can run in Bun, Node, a container, a function with suitable connection lifetime, or any other JavaScript environment. A createServerPlugin() wrapper adds vocabulary but no capability. It should not exist until Macro offers a runtime with a concrete lifecycle contract.

Client code can be distributed as a browser extension or hosted as an independent web application. That removes Macro-owned upload, release, install, and execution infrastructure. It does not eliminate the need for:

- Secure user authorization and consent.
- Stable browser SDK behavior and CORS.
- Realtime event transport.
- Stable page/sidebar placement and context contracts.
- UI isolation rules.
- Compatibility/versioning rules.

The result is better described as **an SDK plus an extension protocol**, not a Plugin Platform.

### What browser extensions really replace

Browser extensions can own:

- Code packaging and distribution.
- Installation, updates, and uninstall.
- Local enable/disable state.
- Execution of client code.
- Their own settings UI and storage.
- UI isolation through an extension iframe or Shadow DOM.

Browser extensions do not own:

- Authorization to Macro data.
- API operation scopes.
- Event filtering and access checks.
- A stable way to appear in Macro navigation or an entity sidebar.
- Compatibility when Macro's DOM and routes change.
- Cross-browser support.

If the extension locates arbitrary CSS selectors and patches Macro's DOM, there is no platform work but there is also no supported product. A small stable host contract is the minimum responsible alternative.

### Chrome-specific constraints

Research against current Chrome extension documentation found:

- Content-script JavaScript runs in an isolated world, so its variables are hidden from the page. That does not isolate injected DOM or CSS by itself; use Shadow DOM or an iframe.
- Cross-origin requests made directly by a content script are still treated as page-origin requests. Extension pages/service workers can use declared host_permissions.
- Manifest V3 forbids remotely hosted executable JavaScript/Wasm. This fits the proposed architecture: each extension bundles its client code, Macro SDK, and optional sync client at build time. It does rule out turning one generic extension into a remote plugin-bundle loader.
- Manifest V3 service workers are not reliable owners for SSE. Chrome can terminate them after inactivity and specifically times out fetch responses that take more than 30 seconds. Keep a UI SSE connection in the active content script/extension page, or reconnect and resync whenever a view opens. Do not claim always-on browser automation.

This means browser extensions are a strong distribution answer for client customizations. The restriction is not a blocker when client and sync code are normal npm dependencies bundled into each extension. Browser extensions are still not a replacement for managed server automation.

## Recommended Product Model

### 1. Macro SDK applications

Any server-side integration is an SDK application or automation. It owns hosting, logs, retries outside event transport, secrets, data stores, dependencies, and operational support.

Use webhooks when reliable push to public infrastructure matters. Use SSE when local development, a continuously running worker, or an interactive client benefits from outbound-only connectivity.

### 2. Macro client extensions

A client extension is browser code that uses @macro/sdk/browser and optionally contributes UI to Macro through a versioned host bridge. It can also render in an ordinary Solid application without the host bridge.

Do not require createClientPlugin() merely to wrap a Solid component. A helper earns its existence only if it describes real host behavior such as placement, identity, supported entity context, and lifecycle:

~~~ts
defineMacroExtension({
  apiVersion: 1,
  contributions: [
    page({ id: 'inbox', label: 'Inbox', render: InboxPage }),
    entitySidebar({
      id: 'health',
      entityTypes: ['document'],
      render: DocumentHealth,
    }),
  ],
});
~~~

This manifest lives inside the browser-extension bundle. Macro does not upload, store, install, or execute it on a server.

### Bundled sync infrastructure

Client extensions do not need to fetch executable bundles or a sync engine at runtime. The extension can bundle all of these through its normal npm/build pipeline:

- Its own UI and business logic.
- @macro/sdk/browser.
- A supported Macro sync package when instant collaborative/optimistic behavior is needed.
- Solid, React, Web Component, or other rendering dependencies.

The repository already contains **packages/collaboration** as the private @macro-inc/collaboration package, including the Loro sync engine, live source, WAL, snapshots, awareness, and WebSocket support. It is evidence that the core can be packaged, but publishing it as an extension dependency requires a deliberate public boundary:

- Publish built JavaScript and declarations rather than workspace TypeScript source exports.
- Remove or hide private workspace dependencies from the public surface.
- Separate browser-neutral sync core from Solid helpers and Tauri-specific adapters.
- Inject API hosts, authentication/token refresh, persistence, logging, and WebSocket construction instead of importing Macro app configuration.
- Version the sync wire/schema compatibility contract independently of the Macro web app.
- Test it in an actual Manifest V3 extension build.

This npm package replaces the document's proposed CDN-hosted sync client. Bundlers can still code-split local extension assets; no Macro client-artifact service is needed.

The sync engine and workspace event SSE solve different problems. The sync engine provides canonical/optimistic collaborative entity state. macro.events provides discrete workspace lifecycle events. Publishing one does not automatically provide the other.

### 3. Optional future managed platform

Managed hosting can later consume the same SDK and event contracts. It should be introduced only after demand proves that users want Macro to own execution, secrets, retries, logs, deployment, billing, and security review.

## Exact SDK Changes For One Handler API

### Desired public behavior

Keep handler registration transport-neutral:

~~~ts
import { Macro } from '@macro/sdk/browser';

const macro = new Macro({
  auth: { type: 'user', token: getFreshToken },
});

const off = macro.events.on('document.updated', async (event) => {
  console.log(event.event_id, await event.document.name());
});

const connection = macro.events.connect({
  filters: [{ events: ['document.updated'], ids: [documentId] }],
  signal: abortController.signal,
});

await connection.closed;
off();
~~~

Webhook usage remains:

~~~ts
const macro = new Macro({ auth, webhookSecret });

macro.events.on('document.updated', handler);
app.post('/webhook', macro.events.webhook());
~~~

The exact common API is events.on(name, handler), EventName, and the hydrated handler payload. webhook() and connect() are transport adapters and are expected to differ.

### Public types

1. Make macro.events available in both Node and browser clients, not conditionally undefined when webhookSecret is absent.
2. Correct MacroEvent to include event_id and schema_version from the actual broker envelope.
3. Keep event_type, metadata, event_id, schema_version, and hydrated entity handles identical for webhook and SSE dispatch.
4. Export EventName, EventMap, EventHandler, EventFilter, EventConnection, and connection error/status types from both entrypoints.
5. Type filter event names as EventName rather than string.
6. Keep onSelfMention() as a convenience layered on on(). It should work unchanged over both transports.

### Internal refactor

Split the current MacroEvents responsibilities:

1. **Dispatcher**: handler registry, event validation, hydration, and dispatch.
2. **Webhook adapter**: signature verification, validation-event handling, header extraction, and Fetch-style response.
3. **SSE adapter**: authenticated stream creation, parsing, reconnect policy, and feeding events into the dispatcher.

MacroEvents.handle() currently combines all of these. A shared dispatch(event) method is the key extraction. Do not make the SSE adapter fake webhook headers or signature verification.

### SSE implementation choice

Use fetch-based SSE, not native EventSource.

Native EventSource accepts only a URL and withCredentials. It has no standard way to attach Authorization or bot headers. Putting bearer tokens in query strings leaks them into URLs, logs, history, and monitoring.

The generated HeyAPI fetch-stream client already:

- Works in Node and browsers.
- Goes through SDK request interceptors.
- Supports Authorization and bot headers.
- Accepts AbortSignal.
- Parses SSE framing.
- Supports retry timing and Last-Event-ID.

Reuse or extract that implementation rather than adding an EventSource dependency.

### Connection lifecycle

Recommended shape:

~~~ts
interface EventConnection {
  readonly closed: Promise<void>;
  close(): void;
}

interface ConnectEventsOptions {
  filters: readonly EventFilter[];
  signal?: AbortSignal;
  onError?: (error: unknown) => void;
}
~~~

connect() should begin connecting immediately. close() and AbortSignal must be idempotent. Token sources should be called again for each reconnect so refreshed credentials are used.

For Solid views, connect on mount and abort on cleanup. For a server process, await connection.closed. Multiple handlers should share one underlying stream per Macro instance; do not open one SSE request per handler.

### Dispatch and failure semantics

- Preserve the current behavior of running handlers registered for one event type.
- Decide explicitly whether handlers run concurrently. Current webhook dispatch uses Promise.all.
- Handler failures over webhooks reject the HTTP request and cause the whole delivery to retry, so other handlers can run more than once.
- SSE has no per-event HTTP acknowledgement. For the first version, document it as best-effort and surface handler errors without pretending they trigger server retries.
- Expose event_id so applications can implement idempotency.
- On reconnect after a gap, client UIs should refetch canonical state before consuming more deltas.
- Do not promise exactly-once delivery.

## Backend Required For Generic SSE

This cannot be achieved only inside packages/sdk. The browser needs an authenticated endpoint that emits the same event envelope.

### Proposed endpoint

Prefer authenticated fetch streaming so filters do not have to fit in a URL:

~~~http
POST /events/stream
Authorization: Bearer ...
Accept: text/event-stream
Content-Type: application/json

{
  "filters": [
    { "events": ["document.updated"], "ids": ["doc-id"] }
  ]
}
~~~

Each SSE item should use:

~~~text
id: opaque-resume-cursor
event: document.updated
data: {"event_id":"...","schema_version":1,"event_type":"document.updated","metadata":{...}}
~~~

The decoded data object should be structurally identical to the decoded webhook body. The SSE id may be an opaque resume cursor; event_id remains the stable idempotency key in data.

### Authorization requirements

- Authenticate user and bot credentials using the same Macro authorization extractor as normal SDK calls.
- Validate every requested event name and filter before opening the stream.
- Apply entity access at event time, not only at connection time.
- Stop or reauthorize streams when credentials expire or permissions change. A bounded maximum connection age is a practical first step.
- Never trust client-declared capabilities as authorization.
- Rate-limit open connections, filters, reconnects, and events per principal.
- Bound buffering and disconnect slow consumers rather than retaining unbounded memory.

### Fan-out architecture

Do not copy the existing process-local task bus for a production generic stream. It loses events on restart and does not work correctly across replicas.

The source must consume the same normalized broker envelopes used by webhooks and fan them out to every service replica that owns live clients. Viable designs include:

- Kafka to a dedicated realtime fan-out service, then Redis/NATS pub-sub to SSE replicas.
- Extend connection_gateway with an authorized domain-event subscription path and add an SSE adapter beside WebSocket.
- A durable event-stream service that owns replay cursors and retention.

Using one ordinary Kafka consumer group across SSE replicas is incorrect because each event goes to only one group member while matching clients may be connected to every replica. A unique consumer group per replica broadcasts but has operational and scaling costs.

### Delivery tiers

Recommended staged contract:

**V1 interactive/best-effort SSE**

- Live events only.
- Heartbeats.
- Automatic reconnect with backoff.
- Explicit gap/reconnect callback and canonical refetch guidance.
- Appropriate for browser UI and local demos.

**V2 replayable SSE**

- Durable, opaque cursor.
- Defined retention window.
- Last-Event-ID resume.
- A clear cursor-expired response such as 410 Gone.
- At-least-once replay and duplicate documentation.
- Per-ordering-key ordering, not a false global-order promise.

Webhooks remain the recommended durable server integration until V2 exists.

## Browser Extension Host Contract

### Minimum supported contract

To avoid coupling extensions to private DOM structure, Macro should expose a tiny versioned contract in the web app:

- Stable mount slots for page navigation, page content, and entity sidebars.
- A versioned context payload containing route, entity type/id, theme, locale, and current user/workspace identifiers that are safe to expose.
- Lifecycle signals for mount, context change, and unmount.
- Supported host actions such as navigate and openEntity, preferably through SDK/deep links rather than arbitrary callbacks.
- Stable data attributes or a MessageChannel handshake, not imports from Macro's internal frontend modules.

This is frontend work, but it is not a backend control plane. It can remain completely local to the installed extension and the current tab.

### Isolation options

**Shadow DOM**

- Lowest friction for a Solid extension.
- Good CSS isolation when the extension owns all styles.
- Shares the page DOM and security process.
- Best for the first demo.

**Extension iframe**

- Stronger DOM/CSS and origin boundary.
- Requires a postMessage/MessageChannel protocol, sizing, focus, theme, and context synchronization.
- The existing plugin iframe protocol may provide reusable ideas even if the managed platform is removed.

Do not inject credentials into page-main-world JavaScript or DOM attributes. Keep tokens in the extension's isolated context and pass only sanitized view data/context across any page bridge.

### Standalone browser applications

The browser SDK technically bundles today, but arbitrary production origins are not accepted by current CORS policy and there is no third-party authorization flow. To support real standalone apps, Macro needs:

- OAuth 2.1 Authorization Code with PKCE or an equivalent delegated authorization flow.
- Registered redirect origins/URIs and revocation.
- Short-lived access tokens with refresh/rotation.
- User consent and server-enforced scopes.
- CORS policy for registered applications using bearer auth without broad credentialed wildcard origins.

For a demo, a manually supplied short-lived token is acceptable only if the UI clearly labels it development-only.

## Capabilities And Type Narrowing

The document currently over-promises this area.

packages/plugin can narrow a facade from a declared capability tuple, and crates/plugin_platform contains a small closed capability/grant model. The normal Macro SDK and its backend services do not currently turn an arbitrary capability list into method-level enforcement for external applications.

There are three separate layers:

1. **Declaration**: the extension says it wants tasks.read and tasks.create.
2. **Type projection**: TypeScript exposes only corresponding namespaces/methods.
3. **Authorization**: a server-issued credential is rejected by every backend endpoint outside the grant.

Only layer 3 is a security boundary. Layers 1 and 2 improve consent and ergonomics but cannot stop hostile JavaScript from constructing a full Macro client.

Recommendation for the demo:

- Do not claim runtime capability enforcement.
- Either omit capabilities or label them compile-time authoring hints.
- Use a bot/user credential with its actual current access model.
- Put delegated, operation-scoped credentials on the SDK/auth roadmap independently of managed plugin hosting. This work benefits CLIs, browser extensions, agents, and integrations.

Once real scopes exist, a type helper can project them:

~~~ts
const macro = createScopedMacro(token, [
  'tasks.read',
  'tasks.create',
] as const);
~~~

The token's signed server-side grant must remain authoritative; the tuple should be checked against token claims rather than inventing authority.

## Revised Demo Recommendation

### Honest one-day demo

1. **Automation**: a plain TypeScript file imports @macro/sdk, registers macro.events.on('channel.message_posted', ...), and creates a real Macro task.
2. **Transport**: use the existing webhook receiver through a tunnel if reliable generic SSE is not already implemented. If demonstrating SSE, label the new stream best-effort and keep it outside the managed plugin runtime.
3. **Client extension**: a locally loaded Manifest V3 extension injects one Document Health panel into a stable Macro slot and renders with Shadow DOM.
4. **Browser SDK**: the panel uses @macro/sdk/browser for real reads/mutations and either generic SSE or a canonical refetch after the existing relevant event feed.
5. **Standalone proof**: optionally mount the same Solid component in a separate localhost app. Treat localhost CORS/token setup as development scaffolding, not proof of production third-party auth.
6. **No control plane**: no plugin upload, release, install row, server runtime, logs UI, or settings backend appears in the story.

### Demo statement

Use wording like:

> Macro is customizable from userspace. Automations are ordinary TypeScript programs using the Macro SDK and webhooks or live events. UI extensions are browser extensions using the browser SDK and stable Macro UI extension points. Macro does not need to host your code.

Avoid saying:

- Macro deploys this automation.
- The extension has runtime-enforced granular capabilities.
- Browser extensions are always-on automation runtimes.
- The UI is isolated unless Shadow DOM/iframe isolation is actually shown.
- SSE has webhook-equivalent retries unless replay/ack semantics exist.

## Migration Of The Current Document Health Prototype

The existing prototype in the plugin-platform-demo worktree proves remote client loading and dynamic server execution. Those are precisely the two platform responsibilities the new direction removes.

### Reuse

- The Document Health Solid component and visual design.
- The document-health scoring behavior as a recognizable client use case.
- The sample workflow that reacts to a real Macro event.
- Focused behavior tests that do not depend on the local runtime protocol.

### Replace

- Replace the custom plugin-platform-demo SDK with @macro/sdk and @macro/sdk/browser.
- Replace the generated server.js artifact and Bun launcher with a normal automation entry file run directly by Bun/Node.
- Replace runtime-owned in-memory state commands with canonical Macro entities/properties, or keep state explicitly local to the extension if it is presentation-only.
- Replace direct Macro sidebar/registry source imports with a browser-extension content script using a stable local-only host slot.
- Bundle the client component into the extension instead of fetching client.js from port 3100.

### Remove from the story

- definePlugin() as a client/server artifact compiler.
- build.ts as a two-artifact producer.
- runtime.ts as a code server/process launcher.
- pluginRuntimeClient.ts and the host-returned changedStateKeys protocol.
- Claims that Macro installed, hosted, sandboxed, or managed either artifact.

The standalone localhost Solid app can consume the same component, SDK, and sync packages through npm/workspace dependencies. The extension bundles those same sources at build time; neither target needs runtime executable-code loading.

## Framework-Neutral Client Contract

The public host boundary should not expose Solid component types. Otherwise React, Web Components, and framework-free extensions require Macro-specific adapters in the host itself.

A durable low-level contribution contract is closer to:

~~~ts
interface ClientContribution<Context> {
  mount(root: HTMLElement, context: Context): void | (() => void);
}
~~~

The Solid helper can implement mount() by rendering a component and returning Solid's disposer. React and Web Component adapters can do the same. The extension manifest may offer projectPage() and entitySidebar() conveniences, but the browser/Macro handshake should be DOM and structured-clone based.

## Implementation Sequence

### Phase 0: approve the boundary

1. Decide that managed artifact hosting is out of scope.
2. Decide whether the existing unshipped platform code is deleted or parked on a branch.
3. Rename the product concepts to SDK applications/automations and client extensions.
4. Remove unsupported capability and deployment claims from the demo script.

### Phase 1: transport-neutral SDK events

1. Fix the generated public event envelope to include event_id and schema_version.
2. Extract dispatch and hydration from webhook verification.
3. Instantiate macro.events in both root and browser Macro classes.
4. Preserve webhook() in the server entrypoint.
5. Add connect() and EventConnection to both entrypoints.
6. Reuse the generated fetch SSE parser and request interceptors.
7. Add unit tests showing webhook and SSE bytes produce the same typed hydrated payload.

### Phase 2: best-effort generic SSE backend

1. Add a domain port over normalized public events rather than coupling the route directly to Kafka.
2. Add authenticated filter validation and event-time entity authorization.
3. Add a shared fan-out adapter that works across replicas.
4. Add the POST events stream Axum adapter and OpenAPI schema.
5. Emit heartbeats and explicit overflow/gap behavior.
6. Add SDK generation and browser tests.

Operational response requirements:

- Content-Type: text/event-stream.
- Cache-Control: no-cache, no-transform.
- Disable proxy buffering and any compression behavior that delays event flushes.
- Send comment heartbeats frequently enough for load balancers.
- Configure load-balancer idle timeouts above the heartbeat interval.

The generated HeyAPI parser currently records an SSE id as soon as it parses the item, before user handlers complete. That is acceptable for best-effort V1. It must be revisited before claiming failed-handler replay or acknowledgement semantics in V2.

### Phase 3: browser-extension proof

1. Add stable LOCAL_ONLY page and entity-sidebar mount slots to Macro.
2. Build a Manifest V3 extension with the Document Health code bundled locally.
3. Render into Shadow DOM and clean up on route/context changes.
4. Keep SDK credentials in isolated extension code, never in the page main world.
5. Connect only while the relevant UI is mounted; refetch canonical state after reconnect.
6. Verify Chrome store-compatible packaging contains the UI, SDK, and sync client locally and no remote executable code.

### Phase 4: production developer platform primitives

1. Add OAuth/PKCE, app registration, consent, revocation, and short-lived tokens.
2. Add backend-enforced operation/resource scopes.
3. Publish and version the frontend host bridge.
4. Decide cross-browser support and extension-store distribution.
5. Add replayable events only if automation/browser demand justifies the durable stream cost.

This phase is a small developer-platform/auth control plane. Browser extensions eliminate artifact hosting, but they cannot safely eliminate app identity, consent, and revocation for third-party software.

## Acceptance Criteria

### SDK

- The same events.on() handler compiles in @macro/sdk and @macro/sdk/browser.
- A webhook and SSE copy of one broker envelope hydrate to the same event object shape.
- event_id and schema_version are visible to handlers.
- One Macro instance opens at most one shared SSE connection for a given subscription set.
- Abort, close, reconnect, malformed input, token refresh, and handler rejection are tested.
- Browser output contains no environment-variable fallback or Node-only dependency.

### Backend

- A principal never receives an event for an entity it cannot currently access.
- Invalid event names and malformed filters fail before stream establishment.
- Slow consumers are bounded and disconnected with observable metrics.
- Multiple service replicas deliver matching events to clients connected to each replica.
- Heartbeats traverse the production proxy/load-balancer path.
- V1 behavior is explicitly documented as non-replaying and best-effort.

### Client extension

- No private CSS selector is required to mount the page/panel.
- Route and entity context changes remount or update without leaks.
- Extension CSS cannot alter Macro UI and Macro CSS cannot alter the extension UI.
- Tokens are not exposed through DOM, window globals, logs, or page messages.
- The extension bundle works with network access to Macro APIs but no remote executable code.

## Recommended Rewrite Of customizing-macro.md

The current document reads like live ideation and contains mutually exclusive claims. In particular:

- definePlugin changes to createServerPlugin/createClientPlugin, then the addendum concludes there may be no server-plugin concept.
- It says client plugins need Macro deployment, then proposes standalone apps and browser extensions.
- It promises cloud deployment and runtime capability enforcement that the demo intentionally does not implement.
- It says the backend stays unchanged while generic SSE, browser authorization, scopes, and supported UI slots all require product work.

Rewrite the narrative around three nouns only:

1. **Macro SDK** for data and actions.
2. **Macro Events** for the same typed handlers over webhooks and SSE.
3. **Macro Client Extensions** for browser-distributed UI contributions.

Move managed hosting, a registry, remote client bundle deployment, and monetization into a clearly labeled future section. Keep publication of a supported npm sync package on the client-SDK roadmap rather than tying it to managed plugins.

## Decision Log

- **Decision:** No createServerPlugin() API in the near-term design.
- **Decision:** macro.events.on() is the server automation authoring API.
- **Decision:** Preserve webhooks; add SSE as a complementary transport, not a semantic replacement.
- **Decision:** Use fetch-based SSE so browser bearer/bot authentication works without URL tokens.
- **Decision:** Browser extensions can replace client deployment/control-plane work, subject to a stable frontend host bridge.
- **Decision:** No claim of fine-grained capability enforcement until credentials and backend authorization enforce it.
- **Decision:** packages/plugin-ui is valuable only if maintained and dogfooded beyond a demo.
- **Decision:** Managed hosting remains a future product, not an MVP prerequisite.

## Open Product Decisions

- Is the first supported client target Chrome only, or must Firefox/Safari be part of the contract?
- Is a Macro UI extension allowed to use Shadow DOM, or is an iframe security boundary required?
- Will third-party extensions receive broad user authority initially, or is delegated auth a launch blocker?
- Are SSE events intentionally best-effort for UI, or must the first generic endpoint provide replay?
- Which event families beyond document and channel are part of the public contract?
- Is the frontend host bridge a supported public API with compatibility guarantees, or merely demo scaffolding?
- Has any managed plugin migration shipped to an environment with persistent data? This determines deletion versus deprecation.

## Sources Consulted

Repository sources:

- **customizing-macro.md**
- **packages/sdk/README.md**
- **packages/sdk/src/events/receiver.ts**
- **packages/sdk/src/events/types.ts**
- **packages/sdk/src/macro.ts** and **macro.browser.ts**
- **packages/sdk/src/utils/client-core.ts**
- **packages/sdk/src/entities/tasks/namespace.ts**
- **packages/sdk/generated/storage/core/serverSentEvents.gen.ts**
- **crates/webhook/src/domain/ingestion.rs**
- **crates/webhook/src/domain/models.rs**
- **crates/webhook/src/outbound/http_delivery.rs**
- **crates/projects/src/inbound/axum_router/task_events.rs**
- **crates/documents/src/domain/task_events.rs**
- **crates/macro_cors/src/lib.rs**
- Current Plugin Platform crates, services, packages, migrations, and web host code listed above.

External references:

- Chrome Extensions, Content scripts: https://developer.chrome.com/docs/extensions/develop/concepts/content-scripts
- Chrome Extensions, Cross-origin network requests: https://developer.chrome.com/docs/extensions/develop/concepts/network-requests
- Chrome Extensions, Service worker lifecycle: https://developer.chrome.com/docs/extensions/develop/concepts/service-workers/lifecycle
- Chrome Extensions, Remote hosted code: https://developer.chrome.com/docs/extensions/develop/migrate/remote-hosted-code
- MDN EventSource constructor: https://developer.mozilla.org/en-US/docs/Web/API/EventSource/EventSource
- WHATWG Server-sent events: https://html.spec.whatwg.org/multipage/server-sent-events.html
