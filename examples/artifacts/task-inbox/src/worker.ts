import { Macro, type MacroOpts } from '@macro/sdk';
type TaskEvent = { event_id: string; event_type: 'task.created' | 'task.updated'; document_id: string; project_id: string };

export async function processCreatedEvent(macro: Macro, projectId: string, event: TaskEvent): Promise<void> {
  if (event.event_type !== 'task.created' || event.project_id !== projectId) return;
  const task = macro.tasks.byId(event.document_id);
  const name = await task.name();
  if (!name.startsWith('[processed] ')) await task.rename(`[processed] ${name}`);
}

async function main() {
  const token = process.env.MACRO_API_KEY?.trim();
  const projectId = process.env.MACRO_PROJECT_ID?.trim();
  const env = process.env.MACRO_ENV;
  if (!token || !projectId || (env !== 'local' && env !== 'dev')) throw new Error('MACRO_API_KEY, MACRO_PROJECT_ID, and MACRO_ENV=local|dev are required');
  const hosts: MacroOpts['hosts'] = process.env.MACRO_STORAGE_HOST ? { storage: process.env.MACRO_STORAGE_HOST } : undefined;
  const macro = new Macro({ token, env, hosts, webAppUrl: process.env.MACRO_WEB_APP_URL });
  for await (const event of macro.tasks.subscribe({ projectId })) await processCreatedEvent(macro, projectId, event);
}

if (import.meta.main) await main();
