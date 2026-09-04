import { Macro } from '../src/macro';

const token = process.env.MACRO_API_KEY;
const channelId = process.argv[2];
if (!token) {
  console.error(
    'usage: MACRO_API_KEY=... bun examples/task-from-message.ts [channel-id]',
  );
  process.exit(1);
}

const macro = new Macro({ token });
const handled = new Set<string>();

macro.events.on('channel.message_posted', async (event) => {
  const taskName = event.metadata.content.match(/^todo:\s*(.+)$/i)?.[1]?.trim();
  if (!taskName || handled.has(event.event_id)) return;

  handled.add(event.event_id);
  try {
    const task = await macro.tasks.create({
      name: taskName,
      markdown: ['Created from channel message ', event.message.id, '.'].join(
        '',
      ),
      shareWithTeam: false,
    });
    await event.message.reply(['Created task: ', task.webUrl()].join(''));
  } catch (error) {
    handled.delete(event.event_id);
    throw error;
  }
});

const connection = macro.events.connect({
  filters: [
    {
      events: ['channel.message_posted'],
      ids: channelId ? [channelId] : undefined,
    },
  ],
  onError: (error) => console.error('automation event error', error),
});

const shutdown = () => connection.close();
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
await connection.closed;
