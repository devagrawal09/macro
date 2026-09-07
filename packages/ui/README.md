# @macro/ui

A small SolidJS component library for Macro client extensions. It targets the
same Solid 1.9 line as the Macro web app, so one component tree can render
inside the app, inside a browser-extension iframe, or in a standalone page.

## Components

- `Badge`
- `Button`
- `Card`
- `Progress`
- `Stack`
- `Text`

Styles are included by the package entry point. Components use scoped
`macro-ui-*` classes and readable standalone defaults. A consumer can override
semantic values such as `--macro-ui-accent`, or set
`data-macro-ui-theme="light|dark"` on an ancestor.

## Usage

```tsx
import { Badge, Button, Card, Progress, Stack, Text } from "@macro/ui";

<Card title="Document health">
  <Stack gap="md">
    <Progress value={72} label="Document health score" />
    <Text>72 / 100</Text>
    <Badge>3 open TODOs</Badge>
    <Button>Refresh</Button>
  </Stack>
</Card>
```

`examples/document-health-extension` renders its sidebar card, full page, and
extension panel from these components.

## Development

The package builds with Vite and `vite-plugin-solid`; tests run under Vitest
with jsdom. Dependencies come from the repository root `bun install`.

```sh
bun run check
bun run test
bun run build
bun run demo
```

The demo is a small host shell and an isolated Document Health iframe. Host
buttons send document updates and light/dark theme changes across
`postMessage`, modeling the browser SDK boundary without coupling this package
to the Macro app runtime.
