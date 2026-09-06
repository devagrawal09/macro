import { Macro, type MacroOpts } from "@macro/sdk";
import { analyzeDocument, DOCUMENT_HEALTH_PROPERTY } from "./score";

const LOCAL_WEBHOOK_URL = "http://sdk-webhook-relay:8787/macro-events";
const env = readEnvironment();
const token = process.env.MACRO_API_KEY;
if (!token) throw new Error("MACRO_API_KEY is required");

const macro = new Macro({ env, token });
const port = Number(
	process.env.PORT ??
		macro._client.localPortmap?.sdkWebhookHostReceiverPort ??
		8787,
);
const endpointUrl =
	process.env.MACRO_WEBHOOK_URL ??
	(env === "local" ? LOCAL_WEBHOOK_URL : undefined);
if (!endpointUrl) {
	throw new Error("MACRO_WEBHOOK_URL is required outside the local stack");
}

let receiver: ((request: Request) => Promise<Response>) | undefined;
let registeredWebhook:
	| Awaited<ReturnType<typeof macro.webhooks.create>>
	| undefined;
const server = Bun.serve({
	hostname: "0.0.0.0",
	port,
	async fetch(request) {
		const url = new URL(request.url);
		if (url.pathname === "/health") return new Response("ok");
		if (request.method !== "POST" || url.pathname !== "/macro-events") {
			return new Response("not found", { status: 404 });
		}
		if (!receiver) {
			return request.headers.get("x-macro-event") === "webhook.validation.test"
				? new Response("ok")
				: new Response("starting", { status: 503 });
		}
		try {
			return await receiver(request);
		} catch (error) {
			console.error("Webhook delivery failed", error);
			const message = error instanceof Error ? error.message : "";
			return new Response("delivery failed", {
				status: message.includes("signature") ? 401 : 500,
			});
		}
	},
});

try {
	const property = await findOrCreateHealthProperty();
	const webhook = await macro.webhooks.create({
		url: endpointUrl,
		namespace: `document-health-demo-${crypto.randomUUID()}`,
		name: "Document Health local demo",
		scope: "user",
		filters: [
			{
				events: [
					"document.created",
					"document.updated",
					"document.content_uploaded",
				],
			},
		],
	});
	registeredWebhook = webhook;
	if (!webhook.signingSecret) {
		throw new Error("Macro did not return a webhook signing secret");
	}

	const eventMacro = new Macro({
		env,
		token,
		webhookSecret: webhook.signingSecret,
	});
	type Document = ReturnType<typeof eventMacro.documents.byId>;

	const updateHealth = async (eventId: string, document: Document) => {
		const health = analyzeDocument(await document.content());
		const serialized = JSON.stringify(health);
		const properties = await document.properties();
		const current = properties.find(
			({ definition }) => definition.id === property.id,
		);
		if (
			current?.value?.type === "String" &&
			current.value.value === serialized
		) {
			console.log(`[${eventId}] ${document.id}: unchanged (${health.score})`);
			return;
		}

		await document.setProperty(property, { type: "string", value: serialized });
		console.log(`[${eventId}] ${document.id}: stored score ${health.score}`);
	};

	eventMacro.events.on("document.created", ({ document }) =>
		updateHealth("document.created", document),
	);
	eventMacro.events.on("document.updated", ({ document }) =>
		updateHealth("document.updated", document),
	);
	eventMacro.events.on("document.content_uploaded", ({ document }) =>
		updateHealth("document.content_uploaded", document),
	);
	receiver = eventMacro.events.webhook();

	const validation = await webhook.validate();
	if (!validation.is_valid) {
		throw new Error(
			validation.message ?? "Macro could not validate the webhook",
		);
	}

	console.log(`Document Health workflow listening on http://localhost:${port}`);
	console.log(`Macro property: ${property.id}`);
	console.log(`Macro webhook: ${webhook.id}`);

	let stopping = false;
	const stop = async () => {
		if (stopping) return;
		stopping = true;
		server.stop();
		await webhook
			.delete()
			.catch((error) => console.error("Could not delete demo webhook", error));
		process.exit(0);
	};
	process.once("SIGINT", () => void stop());
	process.once("SIGTERM", () => void stop());
} catch (error) {
	server.stop();
	await registeredWebhook
		?.delete()
		.catch((cleanupError) =>
			console.error("Could not delete demo webhook", cleanupError),
		);
	throw error;
}

async function findOrCreateHealthProperty() {
	const definitions = await macro.properties.list({ scope: "user" });
	for (const definition of definitions) {
		if ((await definition.displayName) !== DOCUMENT_HEALTH_PROPERTY) continue;
		if ((await definition.dataType) !== "STRING") {
			throw new Error(
				`${DOCUMENT_HEALTH_PROPERTY} exists but is not a string property`,
			);
		}
		return definition;
	}

	return macro.properties.create({
		displayName: DOCUMENT_HEALTH_PROPERTY,
		scope: "user",
		dataType: { type: "string" },
	});
}

function readEnvironment(): NonNullable<MacroOpts["env"]> {
	const value = process.env.MACRO_ENV ?? "local";
	if (value === "local" || value === "dev" || value === "prod") return value;
	throw new Error(`Invalid MACRO_ENV: ${value}`);
}
