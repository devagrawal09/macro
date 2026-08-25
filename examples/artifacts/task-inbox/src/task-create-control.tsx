import { createSignal, type JSX } from 'solid-js';

type TaskCreateControlProps = {
  ready: () => boolean;
  create: (name: string) => Promise<void>;
};

export function TaskCreateControl(props: TaskCreateControlProps) {
  const [name, setName] = createSignal('');

  const createTask = async () => {
    const taskName = name().trim();
    if (!props.ready() || !taskName) return;
    await props.create(taskName);
    setName('');
  };

  const handleKeyDown: JSX.EventHandler<HTMLInputElement, KeyboardEvent> = (
    event
  ) => {
    if (event.key !== 'Enter' || event.isComposing) return;
    event.preventDefault();
    void createTask();
  };

  return (
    <div class="task-create">
      <input
        aria-label="Task name"
        value={name()}
        onInput={(event) => setName(event.currentTarget.value)}
        onKeyDown={handleKeyDown}
      />
      <button type="button" onClick={() => void createTask()}>
        Create
      </button>
    </div>
  );
}
