import path from "node:path";
import { pathToFileURL } from "node:url";
import type {
	PluginDefinition,
	ProjectPage,
	EntitySidePanel,
	BestEffortEvent,
} from "@macro/plugin";
export interface LoadedEntry {
	id: string;
	target: "client" | "server";
	kind: string;
	index: number;
	capabilities: string[];
	entityTypes: string[];
	event?: string;
}
export interface LoadedPlugin {
	definition: PluginDefinition;
	path: string;
	entries: LoadedEntry[];
}
function diagnostic(code: string, message: string): never {
	throw new Error(`${code} ${message}`);
}
function strings(value: unknown, field: string): string[] {
	if (value === undefined) return [];
	if (!Array.isArray(value) || value.some((x) => typeof x !== "string"))
		diagnostic("MPC105", `${field} must contain only strings`);
	return [...new Set(value)].sort();
}
export async function loadPlugin(input: string): Promise<LoadedPlugin> {
	const file = path.resolve(input);
	let module: Record<string, unknown>;
	try {
		module = await import(
			pathToFileURL(file).href + `?macro-plugin-check=${Date.now()}`
		);
	} catch (error) {
		diagnostic(
			"MPC101",
			`could not load side-effect-free local plugin configuration ${file}: ${error instanceof Error ? error.message : String(error)}`,
		);
	}
	const value = module.default as PluginDefinition | undefined;
	if (!value || typeof value !== "object")
		diagnostic(
			"MPC102",
			"plugin module must default-export definePlugin({...})",
		);
	if (value.apiVersion !== "1") diagnostic("MPC103", 'apiVersion must be "1"');
	if (
		typeof value.id !== "string" ||
		!/^(?:[a-z0-9][a-z0-9-]*\.)+[a-z0-9][a-z0-9-]*$/.test(value.id)
	)
		diagnostic("MPC104", "plugin id must use reverse-domain lowercase form");
	if (
		typeof value.version !== "string" ||
		!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/.test(
			value.version,
		)
	)
		diagnostic("MPC106", "version must be semantic version syntax");
	const base = strings(value.capabilities, "capabilities"),
		entries: LoadedEntry[] = [];
	const seen = new Set<string>();
	for (const [index, item] of (value.contributions ?? []).entries()) {
		if (
			!item ||
			typeof item !== "object" ||
			typeof item.id !== "string" ||
			typeof item.render !== "function" ||
			!["project.page", "entity.side_panel"].includes(item.kind)
		)
			diagnostic("MPC107", `invalid client contribution at index ${index}`);
		if (seen.has(item.id))
			diagnostic("MPC108", `duplicate entry id ${item.id}`);
		seen.add(item.id);
		const entityTypes =
			item.kind === "entity.side_panel"
				? strings((item as EntitySidePanel).entityTypes, "entityTypes")
				: [];
		if (item.kind === "entity.side_panel" && !entityTypes.length)
			diagnostic("MPC109", `${item.id} needs an entity type`);
		entries.push({
			id: item.id,
			target: "client",
			kind: item.kind,
			index,
			entityTypes,
			capabilities: [
				...new Set([
					...base,
					...strings(item.capabilities, "entry capabilities"),
				]),
			].sort(),
		});
	}
	for (const [index, item] of (value.handlers ?? []).entries()) {
		if (
			!item ||
			typeof item !== "object" ||
			typeof item.id !== "string" ||
			item.kind !== "best_effort_event" ||
			typeof item.event !== "string" ||
			typeof item.handle !== "function"
		)
			diagnostic("MPC110", `invalid best-effort handler at index ${index}`);
		if (seen.has(item.id))
			diagnostic("MPC108", `duplicate entry id ${item.id}`);
		seen.add(item.id);
		entries.push({
			id: item.id,
			target: "server",
			kind: item.kind,
			index,
			event: item.event,
			entityTypes: [],
			capabilities: [
				...new Set([
					...base,
					...strings(item.capabilities, "entry capabilities"),
				]),
			].sort(),
		});
	}
	if (!entries.length)
		diagnostic("MPC111", "plugin must declare an entrypoint");
	return { definition: value, path: file, entries };
}
export function virtualEntry(plugin: LoadedPlugin, entry: LoadedEntry): string {
	const spec = JSON.stringify(pathToFileURL(plugin.path).href);
	return entry.target === "client"
		? `import plugin from ${spec};import {render as __solidRender} from "@solidjs/web";const __selected=plugin.contributions[${entry.index}].render;export function mount(root,context){return __solidRender(()=>__selected(context),root)}`
		: `import plugin from ${spec};const __selected=plugin.handlers[${entry.index}].handle;export default __selected;export {__selected as handle}`;
}
