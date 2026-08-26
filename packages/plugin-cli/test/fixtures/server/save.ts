export async function save(id: string, project: string) {
	globalThis.__saved = `${project}:${id}`;
}
declare global {
	var __saved: string | undefined;
}
