# Document Health client extension

This is a Chrome Manifest V3 proof that Macro client customizations can be
shipped entirely from userspace. The extension bundles its UI and
`@macro/sdk/browser`; it does not download executable code or use a
Macro-managed plugin runtime.

The content script observes the versioned `entity-sidebar` host slot exposed
only by a local Macro frontend and mounts a bundled extension-origin iframe in
Shadow DOM. The iframe reads the canonical document with the browser SDK and
refreshes on best-effort `document.updated` events. It disconnects when the
slot is removed.

## Build and load

1. Run `bun install` in this directory.
2. Run `bun run build`.
3. Open `chrome://extensions`, enable Developer mode, and load `dist/` as an unpacked extension.
4. Run the Macro web app locally and open a markdown document with its side panel visible.
5. Open the extension popup and enter a short-lived development API token.
6. Use the panel Refresh button after changing the token.

The build rejects common dynamic executable-code loaders. All JavaScript in
`dist/` comes from this package and its npm dependencies at build time.

## Security boundary

The token stays in `chrome.storage.session` and is read only by the trusted
extension-origin iframe. It is never written to the Macro DOM, a page global,
a URL, or a log. The iframe performs API requests under declared host
permissions, while Shadow DOM and the iframe boundary keep styles isolated.

The pasted token flow is development scaffolding, not a production auth model.
A released third-party extension still needs OAuth Authorization Code with
PKCE, user consent, short-lived credentials, server-enforced scopes, and
revocation.
