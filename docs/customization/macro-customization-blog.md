Macro is the unified workspace that gives your team and your agents the context they need. But every team works differently. What if Macro could adapt to your workflows and your style of working?

This post shows how automations, external integrations, agents, and client extensions make Macro fit the way your team already works.

## One SDK for Your Workspace

At the center of this model is the Macro SDK. It gives developers typed access to the documents, tasks, channels, email, projects, and other entities in a Macro workspace.

_[Show the Macro SDK README]_

```ts
import { Macro } from "@macro/sdk";

const macro = new Macro({ token: process.env.MACRO_API_KEY });

const document = macro.documents.byId("doc_123");
const owner = await document.owner();

console.log(await document.name(), await owner.email());
await document.rename("Launch plan");
```

The SDK represents Macro entities as objects rather than a collection of unrelated endpoints. You can query them, follow their relationships, change them, and react to events involving them.

## Automating Workflows

Macro events make it possible to automate the parts of a process that are specific to a team. This workflow registers a handler for changes to a launch document and exposes a signature-verified webhook route:

```ts
const macro = new Macro({
  token: process.env.MACRO_API_KEY,
  webhookSecret: process.env.MACRO_WEBHOOK_SECRET,
});

macro.events.on("document.updated", reviewDocument);
app.post("/webhook", (c) => macro.events.webhook()(c.req.raw));
```

After deploying the server, its persisted webhook can be registered with a `document.updated` filter for the launch document. Macro will then deliver matching workspace events to the route.

The same primitives support workflows such as:

- **Custom review routing.** Changing a team-specific document property can notify the appropriate review channel. The message can include the document and the internal checklist that applies to it.
- **Stale-work reminders.** A scheduled workflow can find tasks that have not changed before a deadline. It can post a focused reminder with links to the relevant work.
- **Daily workspace briefings.** A workflow can gather recent documents, tasks, channels, and email into one summary. It can publish that cross-workspace overview where the team starts its day.

## Adding Agents to Workflows

An automation does not have to stop after moving data or creating a task. Starting an agent is another Macro SDK action. The document review handler can hand the open-ended work to a managed agent session with one API call:

```ts
async function reviewDocument({ document, event_id }) {
  await macro.agentSessions.createManaged({
    prompt: `Review ${document.webUrl()} against our launch checklist.
Create tasks for anything missing.`,
    instructions: `Use ${event_id} as the review-run ID.`,
  });
}
```

_[Show a document change starting an agent session and producing follow-up work]_

This workflow does not duplicate a generic feature already available in Macro. It adds the team's own launch standards, review instructions, and follow-up process. A production deployment can persist `event_id` as an idempotency key for each review run.

The same pattern supports agentic workflows such as:

- **Document drafting.** An agent can turn an approved outline into a first draft. The result stays connected to the source document in Macro.
- **Meeting preparation.** An agent can summarize the documents and messages linked to a customer. The account team receives a focused brief before the call.
- **Incident investigation.** An agent can inspect a failed deployment alongside related discussions and runbooks. It can combine those sources into a timeline and propose the next debugging steps.

Macro becomes the environment where people, software, and agents continue the workflow together with the same context.

## Sending Macro Events to External Systems

Macro workflows are not limited to actions inside the workspace. An event handler can call any external API, making Macro activity part of a broader business process.

For example, a document update can send the canonical document to an internal compliance service:

```ts
macro.events.on("document.updated", async ({ document, event_id }) => {
  await fetch("https://reviews.example.com/documents", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${externalToken}`,
      "Content-Type": "application/json",
      "Idempotency-Key": event_id,
    },
    body: JSON.stringify({
      id: document.id,
      url: document.webUrl(),
    }),
  });
});
```

_[Show the Macro event appearing in the external system]_

The same outbound pattern supports integrations such as:

- **CRM synchronization.** A project milestone in Macro can update the corresponding opportunity or account. Customer-facing teams see the latest delivery context without maintaining it twice.
- **Compliance and archiving.** Approval events can send final documents to a required system of record. The external archive receives a canonical link and a stable event ID for traceability.
- **Workspace analytics.** Macro activity can feed an organization-specific analytics pipeline. Teams can measure their own processes without forcing that reporting model into the core product.

## Bringing External Events into Macro

The integration can also run in the other direction. A service can receive an event from an external system and use the Macro SDK to create or update the workspace context people need.

For example, when a Granola meeting transcript becomes available, an integration can turn it into a Macro document and ask an agent to prepare the follow-up work:

```ts
app.post("/granola/transcripts", async (c) => {
  const transcript = await c.req.json<GranolaTranscriptEvent>();

  const notes = await macro.documents.create({
    name: transcript.title,
    markdown: transcript.markdown,
  });

  await macro.agentSessions.createManaged({
    prompt: `Review ${notes.webUrl()} and create the follow-up tasks.`,
  });

  return c.json({ document: notes.webUrl() }, 201);
});
```

_[Show a Granola transcript becoming a document and follow-up work in Macro]_

Other inbound integrations could include:

- **Issue tracking.** A new issue in an external tracker can create a linked Macro task. The team can work from Macro while retaining the original issue URL.
- **Deployment updates.** A completed deployment can post its status in a Macro channel. The message can link directly to the build.
- **Customer escalation.** A support event can create an incident document and start a Macro agent with the relevant account context. The agent can prepare a summary while the account team coordinates in the workspace.

Macro becomes both a source of context for other systems and the place where their events turn into coordinated work.

## Bringing Custom Workflows into the Interface

Automations are only one part of the opportunity. Macro is also extensible at the interface layer through client extensions: small pieces of UI that live next to, or directly inside, the workspace, shipped as ordinary browser code.

Macro publishes versioned extension slots in its web client. An `entity-sidebar` slot appears in the context of an open document. A `full-page` slot is a directly navigable page dedicated to one document. Both carry a small JSON context (API version, environment, placement, entity) on the host element, so an extension never has to scrape Macro's DOM.

_[Show the Document Health sidebar extension]_

_[Show the Document Health full page]_

The browser SDK exposes `observeMacroExtensionSlots`, which watches for matching slots and mounts your UI into each one. The extension owns its credentials and its rendering; here it renders a SolidJS component into a Shadow DOM root:

```tsx
import { Macro, observeMacroExtensionSlots } from "@macro/sdk/browser";
import { render } from "solid-js/web";

observeMacroExtensionSlots({
  placement: "entity-sidebar",
  mount(host, context) {
    const macro = new Macro({
      env: context.environment,
      auth: { type: "user", token: getToken },
    });
    const root = host.attachShadow({ mode: "open" });
    return render(
      () => <DocumentHealthCard macro={macro} documentId={context.entity.id} />,
      root,
    );
  },
});
```

The component itself is plain Solid. It reads the workspace through the browser SDK and follows live events over the same `macro.events` API the server workflow uses:

```tsx
function DocumentHealthCard(props: { macro: Macro; documentId: string }) {
  const [health, { refetch }] = createResource(
    () => props.documentId,
    (id) => fetchDocumentHealthSnapshot(props.macro, id),
  );

  const off = props.macro.events.on("document.updated", ({ document }) => {
    if (document.id === props.documentId) void refetch();
  });
  const listening = props.macro.events.listen({
    filters: [{ events: ["document.updated"], ids: [props.documentId] }],
  });
  onCleanup(() => {
    off();
    void listening.then((stop) => stop());
  });

  return (
    <section>
      <h2>Document health</h2>
      <p>{health()?.score ?? 0} / 100</p>
      <p>{health()?.openTodos.length ?? 0} open TODOs</p>
      <button onClick={() => void refetch()}>Refresh</button>
    </section>
  );
}
```

_[Show the score and open TODO list updating as the document changes]_

## Making Extensions Feel Native

The component above deliberately uses plain HTML. Extensions can bring any visual style, but they can also use Macro's optional component library: a set of SolidJS components for building interfaces that feel native to the workspace. The same view can be rendered with Macro components without changing its data logic:

```tsx
import { Badge, Button, Card, Progress, Stack, Text } from "@macro/ui";

function DocumentHealthView(props: {
  score: number;
  openTodos: number;
  onRefresh: () => void;
}) {
  return (
    <Card title="Document health">
      <Stack gap="md">
        <Progress value={props.score} />
        <Text>{props.score} / 100</Text>
        <Badge>{props.openTodos} open TODOs</Badge>
        <Button onClick={props.onRefresh}>Refresh</Button>
      </Stack>
    </Card>
  );
}
```

The shared components handle familiar layout, typography, controls, and states while leaving extensions free to develop their own visual identity. Developers and agents can focus on the business logic instead of recreating frontend details, and the result still feels like it belongs inside Macro.

## Putting It All Together

The most useful integrations connect a few steps that normally require manual handoffs.

Imagine a customer meeting follow-up workflow:

1. Granola sends the meeting transcript to an integration endpoint.
2. The integration creates a Macro document and asks an agent to extract decisions and follow-up tasks.
3. A sidebar extension shows those follow-ups next to the meeting notes as the team completes them.
4. Macro sends the final summary and task status to the customer record in the CRM.

_[Show the end-to-end experience]_

The workflow moves information into Macro, gives an agent the open-ended analysis, keeps the result visible to the team, and updates the external system when the work is complete. Every step operates on the same workspace through the Macro SDK.

## Letting Agents Build the Customization

The final piece is a Macro Skill that gives agents concise instructions for building workflows and client extensions. It explains how to use the SDK, connect external systems, subscribe to events, build contextual interfaces, and package the result for a workspace.

_[Show the short Skill file alongside the workflow and extension it generates]_

With that skill available, someone could describe the behavior they want in natural language:

> When Granola publishes a customer meeting transcript, add it to Macro, ask an agent to extract the follow-up work, show its status beside the meeting notes, and send the final summary to our CRM.

An agent can turn that request into the workflow and interface described above.

The SDK lets developers turn workspace context into custom workflows. External integrations connect those workflows to the rest of the organization's systems. Agents take on open-ended work, client extensions make the process visible inside an adaptable Macro interface, and the Macro Skill gives agents the building blocks to create the whole system on a team's behalf.

Teams should not have to move their work into another tool to automate it. Their workflows, agents, and custom interfaces can live directly on top of the workspace that already contains the people, communication, documents, tasks, and context needed to get the work done.
