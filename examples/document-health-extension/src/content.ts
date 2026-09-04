import {
	type MacroExtensionContext,
	observeMacroExtensionSlots,
} from "@macro/sdk/browser";

const FRAME_STYLES =
	"<style>:host{all:initial;display:block}iframe{display:block;width:100%;height:430px;border:0;background:transparent}</style>";

function mountPanel(
	host: HTMLElement,
	context: MacroExtensionContext,
): () => void {
	const fallback = host.querySelector<HTMLElement>(
		"[data-macro-extension-fallback]",
	);
	if (fallback) fallback.hidden = true;

	const container = document.createElement("div");
	container.setAttribute("data-macro-extension-instance", "document-health");
	host.append(container);
	const root = container.attachShadow({ mode: "open" });
	root.innerHTML = FRAME_STYLES;

	const frame = document.createElement("iframe");
	frame.title = "Document Health";
	frame.src = chrome.runtime.getURL(
		`panel.html?context=${encodeURIComponent(JSON.stringify(context))}`,
	);
	frame.addEventListener(
		"load",
		() => container.setAttribute("data-macro-extension-ready", "true"),
		{ once: true },
	);
	root.append(frame);

	return () => {
		container.remove();
		if (fallback && !host.querySelector("[data-macro-extension-instance]")) {
			fallback.hidden = false;
		}
	};
}

observeMacroExtensionSlots({
	placement: "entity-sidebar",
	mount: mountPanel,
});
