import {
	type EventConnection,
	Macro,
	type MacroExtensionContext,
} from "@macro/sdk/browser";
import { analyzeDocument, type DocumentHealth } from "./score";

const MARKUP =
	'<style>:root{color:#eff0e8;background:transparent;font-family:Inter,ui-sans-serif,system-ui,sans-serif}*{box-sizing:border-box}body{margin:0}.panel{overflow:hidden;border:1px solid #41483a;border-radius:10px;background:#171b15;box-shadow:0 18px 45px rgba(10,12,8,.2)}header{padding:16px 16px 13px;border-bottom:1px solid #343a2f;background:linear-gradient(145deg,#252b1e,#171b15)}.eyebrow{margin:0 0 7px;color:#c8f169;font:700 9px/1.2 ui-monospace,SFMono-Regular,monospace;letter-spacing:.13em}h2{margin:0;font:600 20px/1.15 Georgia,serif;letter-spacing:-.02em}.status{margin:7px 0 0;color:#9da396;font-size:11px;line-height:1.4}.status[data-state=error]{color:#ffc26d}.body{padding:16px}.score-row{display:flex;align-items:flex-end;gap:12px}.score{font:600 52px/.9 Georgia,serif;letter-spacing:-.05em}.score-label{padding-bottom:4px;color:#9da396;font-size:10px}.score-label strong{display:block;margin-bottom:2px;color:#c8f169;font-size:12px}.track{height:4px;margin:14px 0 16px;overflow:hidden;border-radius:99px;background:#343a2f}.fill{height:100%;width:0;border-radius:inherit;background:linear-gradient(90deg,#edbd55,#c8f169);transition:width .25s ease}.metrics{display:grid;grid-template-columns:repeat(3,1fr);gap:7px}.metric{padding:9px 7px;border:1px solid #343a2f;border-radius:7px;background:#1f241c;text-align:center}.metric strong{display:block;color:#f6f3e9;font:600 15px/1.2 Georgia,serif}.metric span{display:block;margin-top:3px;color:#858b7e;font-size:8px;letter-spacing:.08em;text-transform:uppercase}.todos{margin-top:14px;padding-top:13px;border-top:1px solid #343a2f}.todos-head{display:flex;align-items:center;justify-content:space-between}.todos h3{margin:0;font-size:11px}.count{display:grid;min-width:18px;height:18px;place-items:center;border-radius:99px;background:#30372a;color:#c8f169;font-size:9px}ul{display:grid;gap:7px;margin:10px 0 0;padding:0;list-style:none}li{display:flex;gap:7px;color:#b8bcb0;font-size:10px;line-height:1.45}li:before{content:"";width:4px;height:4px;flex:0 0 auto;margin-top:5px;border-radius:50%;background:#edbd55}.empty{margin:10px 0 0;color:#9eae81;font-size:10px}.footer{display:flex;align-items:center;justify-content:space-between;margin-top:14px;color:#72786d;font-size:9px}button{all:unset;cursor:pointer;border:1px solid #48513f;border-radius:99px;padding:5px 9px;color:#dce4ce;font:650 9px/1 system-ui,sans-serif}button:hover{border-color:#c8f169;color:#c8f169}</style><section class="panel"><header><p class="eyebrow">BUNDLED CLIENT EXTENSION</p><h2 id="title">Document Health</h2><p id="status" class="status">Reading the canonical document...</p></header><div class="body"><div class="score-row"><strong id="score" class="score">--</strong><div class="score-label"><strong id="grade">Waiting</strong>out of 100</div></div><div class="track"><div id="fill" class="fill"></div></div><div class="metrics"><div class="metric"><strong id="words">--</strong><span>Words</span></div><div class="metric"><strong id="read">--</strong><span>Read</span></div><div class="metric"><strong id="long">--</strong><span>Long sentences</span></div></div><div class="todos"><div class="todos-head"><h3>Open work</h3><span id="count" class="count">0</span></div><ul id="todo-list"></ul><p id="empty" class="empty">Nothing left behind.</p></div><div class="footer"><span id="updated">Not analyzed</span><button id="refresh" type="button">Refresh</button></div></div></section>';

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
		throw new Error(
			"Open the extension popup and add a short-lived dev token.",
		);
	}
	return stored.macroDevelopmentToken;
}

function required<T extends Element>(root: ParentNode, selector: string): T {
	const element = root.querySelector<T>(selector);
	if (!element) throw new Error("Document Health panel markup is incomplete");
	return element;
}

function grade(score: number): string {
	if (score >= 90) return "Healthy";
	if (score >= 70) return "Needs polish";
	return "Needs attention";
}

function renderHealth(root: ParentNode, health: DocumentHealth): void {
	required(root, "#score").textContent = String(health.score);
	required(root, "#grade").textContent = grade(health.score);
	required<HTMLElement>(root, "#fill").style.width = `${String(health.score)}%`;
	required(root, "#words").textContent = String(health.words);
	required(root, "#read").textContent = `${String(health.readingMinutes)}m`;
	required(root, "#long").textContent = String(health.longSentences);
	required(root, "#count").textContent = String(health.openTodos.length);

	const list = required<HTMLUListElement>(root, "#todo-list");
	const empty = required<HTMLElement>(root, "#empty");
	list.replaceChildren(
		...health.openTodos.map((todo) => {
			const item = document.createElement("li");
			item.textContent = todo;
			return item;
		}),
	);
	empty.hidden = health.openTodos.length > 0;
	required(root, "#updated").textContent =
		"Analyzed " +
		new Date().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

/** Mount the Document Health contribution into one Macro sidebar slot. */
export function mountDocumentHealth(
	root: HTMLElement,
	context: MacroExtensionContext,
): () => void {
	root.innerHTML = MARKUP;

	const status = required<HTMLElement>(root, "#status");
	const macro = new Macro({
		env: context.environment,
		auth: { type: "user", token: getToken },
	});
	let connection: EventConnection | undefined;
	let unsubscribe = () => {};
	let disposed = false;
	let refreshVersion = 0;

	const setStatus = (message: string, isError = false) => {
		status.textContent = message;
		status.dataset.state = isError ? "error" : "ready";
	};
	const connect = (macro: Macro) => {
		if (connection || disposed) return;
		unsubscribe = macro.events.on("document.updated", () => {
			void refresh();
		});
		connection = macro.events.connect({
			filters: [{ events: ["document.updated"], ids: [context.entity.id] }],
			onError: () => {
				setStatus("Live updates reconnecting. Refreshing canonical state...");
				void refresh();
			},
		});
	};
	const refresh = async () => {
		const version = ++refreshVersion;
		setStatus("Reading the canonical document...");
		try {
			const document = macro.documents.byId(context.entity.id);
			const [name, content] = await Promise.all([
				document.name(),
				document.content(),
			]);
			if (disposed || version !== refreshVersion) return;
			required(root, "#title").textContent = name || "Document Health";
			renderHealth(root, analyzeDocument(content));
			setStatus("Live from the Macro SDK and best-effort events.");
			connect(macro);
		} catch (error) {
			if (disposed || version !== refreshVersion) return;
			setStatus(
				error instanceof Error ? error.message : "Could not analyze document.",
				true,
			);
		}
	};

	required(root, "#refresh").addEventListener("click", () => void refresh());
	void refresh();

	return () => {
		disposed = true;
		refreshVersion += 1;
		unsubscribe();
		connection?.close();
		root.replaceChildren();
	};
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

const dispose = mountDocumentHealth(document.body, readContext());
addEventListener("pagehide", dispose, { once: true });
