import type { Macro } from '@macro/sdk/browser';

/** Create an artifact task without inheriting the SDK's team-sharing default. */
export async function createPrivateTask(macro: Macro, projectId: string, name: string) {
  return macro.tasks.create({
    name,
    project: macro.projects.byId(projectId),
    shareWithTeam: false,
  });
}
