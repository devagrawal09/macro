import type { MacroClient } from '../../utils/client';
import type { Project } from '../projects/project';
import type { SearchOpts } from '../search';
import type { Team } from '../teams/team';
import { Task } from './task';

/** A live task lifecycle event scoped to one authorized project. */
export type TaskEvent = {
  event_id: string;
  event_type: 'task.created' | 'task.updated';
  document_id: string;
  project_id: string;
};

const TASK_EVENT_KEYS = [
  'document_id',
  'event_id',
  'event_type',
  'project_id',
] as const;

/** Strictly validate one untrusted SSE item. */
export function parseTaskEvent(value: unknown): TaskEvent {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new TypeError('invalid task event: expected an object');
  }
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  if (
    keys.length !== TASK_EVENT_KEYS.length ||
    keys.some((key, index) => key !== TASK_EVENT_KEYS[index])
  ) {
    throw new TypeError('invalid task event: expected exactly four known keys');
  }
  if (
    typeof record.event_id !== 'string' ||
    record.event_id.length === 0 ||
    typeof record.document_id !== 'string' ||
    record.document_id.length === 0 ||
    typeof record.project_id !== 'string' ||
    record.project_id.length === 0 ||
    (record.event_type !== 'task.created' &&
      record.event_type !== 'task.updated')
  ) {
    throw new TypeError('invalid task event: malformed field');
  }
  return record as TaskEvent;
}

export class TaskNamespace {
  constructor(private readonly client: MacroClient) {}

  /** A handle to a task by document id. Details load on first access. */
  byId(id: string): Task {
    return Task.byId(this.client, id);
  }

  /** Create a task. */
  create(opts: {
    name: string;
    markdown?: string;
    project?: Project;
    team?: Team;
    shareWithTeam?: boolean;
  }): Promise<Task> {
    return Task.create(this.client, opts);
  }

  /**
   * Subscribe to the non-replaying live task feed for one authorized project.
   * Consumer cancellation aborts the one underlying authenticated fetch.
   */
  async *subscribe(opts: { projectId: string }): AsyncIterable<TaskEvent> {
    const controller = new AbortController();
    try {
      const result = await this.client.storage.taskEventsHandler({
        path: { id: opts.projectId },
        signal: controller.signal,
        sseMaxRetryAttempts: 1,
        onSseError(error: unknown) {
          throw error;
        },
      });
      for await (const item of result.stream as AsyncIterable<unknown>) {
        yield parseTaskEvent(item);
      }
    } finally {
      controller.abort();
    }
  }

  /** Search tasks by name and content, most relevant first, auto-paginated. */
  search(query: string, opts?: SearchOpts): AsyncGenerator<Task> {
    return Task.search(this.client, query, opts);
  }
}
