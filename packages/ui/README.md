# @macro/ui

A small SolidJS 2 component library for interfaces rendered inside Macro plugin iframes.

## Components

The first slice matches the Document Health example in `macro-customization-blog.md`:

- `Badge`
- `Button`
- `Card`
- `Progress`
- `Stack`
- `Text`

Styles are included by the package entry point. Components use scoped `macro-ui-*` classes and readable standalone defaults. A plugin can override semantic values such as `--macro-ui-accent`, or set `data-macro-ui-theme="light|dark"` on an ancestor.

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

## Development

The Solid 2 RC toolchain is currently tested against source checkouts. Defaults are `/Users/devagr/solid` and `/Users/devagr/dom-expressions`; override them with `MACRO_SOLID_CHECKOUT` and `MACRO_DOM_EXPRESSIONS_CHECKOUT`.

```sh
bun run check
bun run test
bun run build
bun run build:demo
bun run demo
```

The demo is a small host shell and an isolated Document Health iframe. Host buttons send document updates and light/dark theme changes across `postMessage`, modeling the browser SDK boundary without coupling this package to the Macro app runtime.
