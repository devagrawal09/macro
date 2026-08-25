import { describe, expect, mock, test } from 'bun:test';
import { processCreatedEvent } from './worker';

const event = {
  event_id: 'event', event_type: 'task.created' as const,
  document_id: 'document', project_id: 'project',
};

function macroFor(name: string) {
  const rename = mock(async (_name: string) => {});
  const byId = mock((_id: string) => ({ name: async () => name, rename }));
  return { macro: { tasks: { byId } } as never, byId, rename };
}

describe('manual worker scope and idempotence', () => {
  test('ignores updates and other projects', async () => {
    const fake = macroFor('Task');
    await processCreatedEvent(fake.macro, 'project', { ...event, event_type: 'task.updated' });
    await processCreatedEvent(fake.macro, 'project', { ...event, project_id: 'other' });
    expect(fake.byId).not.toHaveBeenCalled();
  });

  test('prefixes a matching created task once', async () => {
    const fake = macroFor('Task');
    await processCreatedEvent(fake.macro, 'project', event);
    expect(fake.byId).toHaveBeenCalledWith('document');
    expect(fake.rename).toHaveBeenCalledWith('[processed] Task');
  });

  test('does not rename an already processed task', async () => {
    const fake = macroFor('[processed] Task');
    await processCreatedEvent(fake.macro, 'project', event);
    expect(fake.rename).not.toHaveBeenCalled();
  });
});
