import { createSignal, onCleanup } from "solid-js";
import { render } from "solid-js/web";
import { Badge, Button, Card, Progress, Stack, Text } from "../src";

type HealthState = {
  score: number;
  todos: string[];
  revision: number;
};

type HealthMessage = {
  type: "macro.document-health";
  state: HealthState;
  theme: "light" | "dark";
};

const initialState: HealthState = {
  score: 72,
  todos: ["Confirm analytics owner", "Add rollback criteria", "Review support handoff"],
  revision: 12
};

function DocumentHealthPlugin() {
  const [health, setHealth] = createSignal(initialState);
  const [refreshing, setRefreshing] = createSignal(false);

  const onMessage = (event: MessageEvent<HealthMessage>) => {
    if (event.source !== window.parent || event.data?.type !== "macro.document-health") return;
    setHealth(event.data.state);
    setRefreshing(false);
    document.documentElement.dataset.macroUiTheme = event.data.theme;
  };

  window.addEventListener("message", onMessage);
  onCleanup(() => window.removeEventListener("message", onMessage));
  window.parent.postMessage({ type: "macro.document-health.ready" }, "*");

  const refresh = () => {
    setRefreshing(true);
    window.parent.postMessage({ type: "macro.document-health.refresh" }, "*");
  };

  const tone = () => health().score >= 90 ? "success" as const : health().score >= 75 ? "warning" as const : "failure" as const;

  return (
    <Card title="Document health" description={"Revision " + String(health().revision)}>
      <Stack gap="lg">
        <Stack gap="sm">
          <Progress value={health().score} tone={tone()} label="Document health score" />
          <Stack direction="row" justify="between" align="baseline">
            <Text role="display">{health().score}</Text>
            <Text role="caption" tone="subtle">/ 100</Text>
          </Stack>
        </Stack>

        <Stack gap="sm">
          <Badge tone={health().todos.length === 0 ? "success" : "warning"}>
            {health().todos.length} open TODO{health().todos.length === 1 ? "" : "s"}
          </Badge>
          {health().todos.length > 0 ? (
            <ol class="health-todos">
              {health().todos.map((todo) => <li>{todo}</li>)}
            </ol>
          ) : (
            <Text tone="success" weight="medium">Ready for launch review.</Text>
          )}
        </Stack>

        <Button onClick={refresh} loading={refreshing()} fullWidth>
          Refresh analysis
        </Button>
      </Stack>
    </Card>
  );
}

const root = document.querySelector<HTMLElement>("#plugin");
if (!root) throw new Error("Document Health plugin root is missing");
render(() => <DocumentHealthPlugin />, root);
