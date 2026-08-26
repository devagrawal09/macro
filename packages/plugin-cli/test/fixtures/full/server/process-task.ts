export async function processTask(id: string, projectId: string) {
	globalThis.__taskResult = `MPC_SERVER_ONLY:${projectId}:${id}`;
}
declare global {
	var __taskResult: string | undefined;
}
