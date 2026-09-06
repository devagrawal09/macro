# Document Health workflow and client extension

This local demo has two parts:

- A Bun server receives signed Macro document events, calculates health, and stores the snapshot in a `Document Health` custom property on the document.
- A SolidJS plugin reads that property through `@macro/sdk/browser` and renders in Macro's local frontend in two placements: an `entity-sidebar` card in the open document's right sidebar, and a `full-page` view at `/app/component/document-health~<documentId>`. The local frontend imports both directly for the shortest demo loop; the Chrome extension proves the isolated userspace path.

Both placements follow live changes with the upstream SSE transport: they register a `document.updated` handler and call `macro.events.listen({ filters: [{ events: ['document.updated'], ids: [documentId] }] })`, refetching the canonical property when a matching event arrives.

There is no extension database. The canonical document and all workflow state stay in Macro.

## Run the local demo

1. Start Macro from the repository root:

   ```sh
   bash .cursor/stack.sh
   ```

2. Open `http://localhost:3000/app`, log in, then create a user key under **Settings > API Keys**. The secret is shown once.

3. Install dependencies and start the workflow:

   ```sh
   cd examples/document-health-extension
   bun install
   MACRO_API_KEY=... bun run server
   ```

   `MACRO_ENV` defaults to `local`. The process listens on the instance-specific webhook receiver port, registers `http://sdk-webhook-relay:8787/macro-events`, verifies the webhook, and prints its Macro property and webhook IDs. Press Ctrl-C to remove the temporary webhook.

4. In another terminal, create a document that contains two open tasks:

   ```sh
   cd examples/document-health-extension
   MACRO_API_KEY=... bun run demo
   ```

   Open the printed local Macro URL. The workflow log should show a stored score of `88`.

5. Open the document in the local frontend. Vite imports `DocumentHealthPlugin.tsx` directly and renders it in the **Document Health** sidebar section. No extension build is required for this path.

6. Remove one unchecked task from the document. The workflow receives `document.updated` and stores the new snapshot; the sidebar card hears the same event over `macro.events.listen()` and refetches. Click **Refresh** if a refetch still races the server delivery.

## Full-page view

Click **Open full page** under the sidebar card, or navigate directly to `/app/component/document-health~<documentId>`. The page renders `DocumentHealthPage.tsx`: the document's name, the score with a verdict, per-signal metrics, outstanding work, a manual **Refresh**, and an **Open document** action back to the source document. It reads the same canonical property, follows the same `document.updated` SSE subscription, and works on desktop and mobile. The route is registered only in local mode; elsewhere it redirects to the inbox.

## Optional Chrome extension path

Build the extension:

```sh
bun run build
```

Open `chrome://extensions`, enable Developer mode, and load `dist/` as an unpacked extension. Open its popup and enter the same development API key. Then open the demo document with its side panel visible.

When loaded, the extension hides the directly imported local plugin and mounts its isolated iframe into the same versioned slot.

## Local webhook path

The local stack's storage service cannot call a process through the host's `localhost`. It sends the persisted webhook to `sdk-webhook-relay:8787`; the stack's SSH reverse tunnel forwards that request to the generated host receiver port used by `server.ts`.

The first registration-time validation request is acknowledged before Macro returns the one-time signing secret. Every later delivery, including the explicit validation in `server.ts`, is verified by `macro.events.webhook()`.

## Security boundary

The server reads the API key only from its environment. The extension keeps its development key in `chrome.storage.session`; it does not write it to the Macro DOM, page globals, URLs, or logs. The pasted token flow is local scaffolding, not a production authorization model.
