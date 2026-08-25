import { expect, test } from 'bun:test';
import { createPrivateTask } from './task-client';

test('every artifact task create explicitly disables team sharing', async () => {
  const project = { id: 'project' };
  let options: Record<string, unknown> | undefined;
  const macro = {
    projects: { byId: (id: string) => ({ ...project, id }) },
    tasks: { create: async (value: Record<string, unknown>) => { options = value; return {}; } },
  };

  await createPrivateTask(macro as never, 'project', 'Task');

  expect(options).toEqual({ name: 'Task', project, shareWithTeam: false });
});
