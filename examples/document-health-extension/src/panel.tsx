import { Macro, type MacroExtensionContext } from "@macro/sdk/browser";
import { Card } from "@macro/ui";
import { render } from "solid-js/web";
import { DocumentHealthCard } from "./DocumentHealthCard";

interface TokenResponse {
	macroDevelopmentToken?: unknown;
}

async function getToken(): Promise<string> {
	const stored = (await chrome.storage.session.get(
		"macroDevelopmentToken",
	)) as TokenResponse;
	if (
		typeof stored.macroDevelopmentToken !== "string" ||
		stored.macroDevelopmentToken.length === 0
	) {
		throw new Error("Open the extension popup and add a short-lived dev token.");
	}
	return stored.macroDevelopmentToken;
}

function readContext(): MacroExtensionContext {
	const encoded = new URL(location.href).searchParams.get("context");
	const value = encoded ? (JSON.parse(encoded) as MacroExtensionContext) : null;
	if (
		value?.apiVersion !== 1 ||
		!["local", "dev", "prod"].includes(value.environment) ||
		value.placement !== "entity-sidebar" ||
		value.entity?.type !== "document" ||
		typeof value.entity.id !== "string"
	) {
		throw new Error("Missing or unsupported Macro extension context");
	}
	return value;
}

const context = readContext();
const macro = new Macro({
	env: context.environment,
	auth: { type: "user", token: getToken },
});

const root = document.getElementById("root");
if (!root) throw new Error("Document Health panel root is missing");

// The same component the local Macro sidebar renders directly, now inside the
// extension's isolated iframe with credentials that never touch the page.
const dispose = render(
	() => (
		<Card title="Document Health" description="Bundled client extension">
			<DocumentHealthCard macro={macro} documentId={context.entity.id} />
		</Card>
	),
	root,
);
addEventListener("pagehide", dispose, { once: true });
