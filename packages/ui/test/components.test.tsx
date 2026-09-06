import { afterEach, describe, expect, it } from "vitest";
import { fireEvent } from "@testing-library/dom";
import { Badge, Button, Card, Progress, Stack, Text } from "../src";
import { cleanup, render } from "./render";

afterEach(cleanup);

describe("blog component set", () => {
  it("renders the Document Health composition", () => {
    const view = render(() => (
      <Card title="Document health">
        <Stack gap="md">
          <Progress value={72} label="Document health score" />
          <Text>72 / 100</Text>
          <Badge>3 open TODOs</Badge>
          <Button>Refresh</Button>
        </Stack>
      </Card>
    ));

    expect(view.getByRole("heading", { name: "Document health" })).toBeTruthy();
    expect(view.getByRole("progressbar", { name: "Document health score" }).getAttribute("value")).toBe("72");
    expect(view.getByText("3 open TODOs")).toBeTruthy();
    expect(view.getByRole("button", { name: "Refresh" })).toBeTruthy();
  });

  it("clamps progress values and dispatches button clicks", () => {
    let clicks = 0;
    const view = render(() => (
      <Stack direction="row" gap="sm">
        <Progress value={140} max={100} />
        <Button onClick={() => clicks++}>Refresh</Button>
      </Stack>
    ));

    expect(view.getByRole("progressbar").getAttribute("value")).toBe("100");
    fireEvent.click(view.getByRole("button"));
    expect(clicks).toBe(1);
  });

  it("exposes semantic variants", () => {
    const view = render(() => (
      <Card title="State" description="Current document analysis">
        <Badge tone="warning">Needs attention</Badge>
        <Text tone="muted" role="caption">Updated now</Text>
      </Card>
    ));

    expect(view.getByText("Needs attention").getAttribute("data-tone")).toBe("warning");
    expect(view.getByText("Updated now").getAttribute("data-role")).toBe("caption");
  });
});
