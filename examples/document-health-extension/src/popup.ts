export {};

const TOKEN_KEY = "macroDevelopmentToken";
const form = document.querySelector<HTMLFormElement>("form");
const input = document.querySelector<HTMLInputElement>("#token");
const status = document.querySelector<HTMLElement>("#status");
const clear = document.querySelector<HTMLButtonElement>("#clear");

if (!form || !input || !status || !clear) {
	throw new Error("Document Health popup markup is incomplete");
}

void chrome.storage.session.get(TOKEN_KEY).then((stored) => {
	status.textContent = stored[TOKEN_KEY]
		? "A token is available for this browser session."
		: "No token configured.";
});

form.addEventListener("submit", (event) => {
	event.preventDefault();
	const token = input.value.trim();
	if (!token) return;
	void chrome.storage.session.set({ [TOKEN_KEY]: token }).then(() => {
		input.value = "";
		status.textContent = "Saved for this browser session.";
	});
});

clear.addEventListener("click", () => {
	void chrome.storage.session.remove(TOKEN_KEY).then(() => {
		input.value = "";
		status.textContent = "Token cleared.";
	});
});
