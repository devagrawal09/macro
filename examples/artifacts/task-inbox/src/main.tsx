import { Macro } from '@macro/sdk/browser';
import { createSignal, For, Show } from 'solid-js';
import { render } from 'solid-js/web';
import { receiveTaskInboxToken } from './protocol';
import { TaskCreateControl } from './task-create-control';
import { createPrivateTask } from './task-client';
import './style.css';

const tokenPromise = receiveTaskInboxToken();

type TaskRow = { id: string; name: string };

function App() {
  const [tasks, setTasks] = createSignal<TaskRow[]>([]);
  const [error, setError] = createSignal<string>();
  let macro: Macro | undefined;

  const upsert = (task: TaskRow) =>
    setTasks((current) => {
      const next = current.filter((item) => item.id !== task.id);
      return [...next, task].sort((a, b) => a.name.localeCompare(b.name));
    });

  void tokenPromise
    .then(async (token) => {
      const hosts = __MACRO_STORAGE_HOST__
        ? { storage: __MACRO_STORAGE_HOST__ }
        : undefined;
      macro = new Macro({
        token,
        env: __MACRO_ENV__,
        hosts,
        webAppUrl: __MACRO_WEB_APP_URL__,
      });
      const project = macro.projects.byId(__MACRO_PROJECT_ID__);
      const items = await project.items();
      for (const item of items) {
        if (item.type === 'document' && item.subType?.type === 'task') {
          upsert({ id: item.id, name: item.name });
        }
      }
      for await (const event of macro.tasks.subscribe({
        projectId: __MACRO_PROJECT_ID__,
      })) {
        if (event.project_id !== __MACRO_PROJECT_ID__) continue;
        const task = macro.tasks.byId(event.document_id);
        upsert({ id: task.id, name: await task.name() });
      }
    })
    .catch((cause) =>
      setError(cause instanceof Error ? cause.message : String(cause))
    );

  const createTask = async (taskName: string) => {
    if (!macro) return;
    await createPrivateTask(macro, __MACRO_PROJECT_ID__, taskName);
  };

  return (
    <main>
      <h1>Task inbox</h1>
      <TaskCreateControl
        ready={() => macro !== undefined}
        create={createTask}
      />
      <Show when={error()}>
        {(message) => <p class="error">{message()}</p>}
      </Show>
      <ul>
        <For each={tasks()}>{(task) => <li>{task.name}</li>}</For>
      </ul>
    </main>
  );
}

render(() => <App />, document.getElementById('root')!);
