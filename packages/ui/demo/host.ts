type Theme = "light" | "dark";

type HealthState = {
  score: number;
  todos: string[];
  revision: number;
};

const states: HealthState[] = [
  { score: 72, todos: ["Confirm analytics owner", "Add rollback criteria", "Review support handoff"], revision: 12 },
  { score: 84, todos: ["Add rollback criteria", "Review support handoff"], revision: 13 },
  { score: 96, todos: [], revision: 14 }
];

function required<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error("Missing demo element: " + selector);
  return element;
}

const frame = required<HTMLIFrameElement>("#plugin-frame");
const updateButton = required<HTMLButtonElement>("#update");
const themeButton = required<HTMLButtonElement>("#theme");
const revision = required<HTMLElement>("#revision");
const statusText = required<HTMLElement>("#document-status");

let stateIndex = 0;
let theme: Theme = "light";

function sendState(): void {
  const state = states[stateIndex];
  if (!state) return;
  revision.textContent = "Revision " + String(state.revision);
  statusText.textContent = state.todos.length > 0 ? state.todos.join(". ") + "." : "All launch checks are complete.";
  frame.contentWindow?.postMessage({ type: "macro.document-health", state, theme }, "*");
}

function advance(): void {
  stateIndex = (stateIndex + 1) % states.length;
  sendState();
}

frame.addEventListener("load", sendState);
updateButton.addEventListener("click", advance);
themeButton.addEventListener("click", () => {
  theme = theme === "light" ? "dark" : "light";
  themeButton.textContent = theme === "light" ? "Use dark theme" : "Use light theme";
  sendState();
});

window.addEventListener("message", (event) => {
  if (event.source !== frame.contentWindow) return;
  if (event.data?.type === "macro.document-health.ready") sendState();
  if (event.data?.type === "macro.document-health.refresh") {
    updateButton.disabled = true;
    window.setTimeout(() => {
      advance();
      updateButton.disabled = false;
    }, 350);
  }
});
