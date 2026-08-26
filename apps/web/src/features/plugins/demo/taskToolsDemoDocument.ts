import taskToolsClientBundle from './task-tools-client.bundle.js?raw';
import { serializePluginBootstrapSource } from './pluginBootstrap';

/** Global name the inlined bundle assigns its exports to. */
const PLUGIN_ENTRY_GLOBAL = '__MACRO_PLUGIN_ENTRY__';

/**
 * Rewrite the bundle's single trailing `export { ... }` statement into an
 * assignment on `globalThis` so the bootstrap glue (same module script) and
 * nothing else can reach the entry. The compiler emits exactly one trailing
 * export statement per client entry; anything else fails closed.
 */
export function inlineClientBundle(source: string): string {
	const exportMatch = source.match(/export\s*\{([^}]*)\};?\s*$/);
	if (!exportMatch) {
		throw new Error('client bundle has no trailing export statement');
	}
	const bindings = exportMatch[1]
		.split(',')
		.map((specifier) => specifier.trim())
		.filter((specifier) => specifier.length > 0)
		.map((specifier) => {
			const parts = specifier.split(/\s+as\s+/);
			const local = parts[0];
			const exported = parts[1] ?? local;
			return `${exported}: ${local}`;
		});
	const body = source.slice(0, exportMatch.index ?? source.length);
	return `${body}\nglobalThis.${PLUGIN_ENTRY_GLOBAL} = { ${bindings.join(', ')} };\n`;
}

/** Neutralize `</script>` breakouts when embedding JS into a srcdoc document. */
export function escapeForScriptTag(source: string): string {
	return source.replace(/<\/script/gi, '<\\/script');
}

/** Full opaque-origin document for one Task Tools client frame. */
export function buildTaskToolsDemoDocument(): string {
	const bootstrap = serializePluginBootstrapSource();
	const bundle = inlineClientBundle(taskToolsClientBundle);
	return [
		'<!doctype html>',
		'<html><head><meta charset="utf-8">',
		'<style>html,body,#root{margin:0;height:100%;}body{font-family:sans-serif;padding:12px;background:#ffffff;color:#111111}</style>',
		'</head><body><div id="root"></div>',
		`<script type="module">${escapeForScriptTag(bundle)}\n${escapeForScriptTag(bootstrap)}</script>`,
		'</body></html>',
	].join('');
}
