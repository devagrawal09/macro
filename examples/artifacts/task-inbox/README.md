# Task inbox artifact

A fixed dev/local HTML artifact. It runs only inside Macro's sandboxed HTML preview. Configure `MACRO_PROJECT_ID` and `MACRO_ENV=local|dev` for builds. Optional non-secret overrides are `MACRO_STORAGE_HOST` and `MACRO_WEB_APP_URL`. The browser token arrives only through the nonce-bound `MessageChannel` protocol.

`bun dev` performs a watch build. It does not start a web server. `bun worker` runs the live, no-replay processor. `bun deploy` rebuilds and requires the exact fixed document ID as interactive confirmation before raw multipart `simple_save`. Never place `MACRO_API_KEY` or `MACRO_HTML_DOCUMENT_ID` in build configuration.
