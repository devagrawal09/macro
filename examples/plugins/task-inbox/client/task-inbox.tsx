import type { PluginClientContext, PluginTask } from "@macro/plugin";
import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import {
	createPrivateTask,
	followTaskEvents,
	loadProjectTasks,
	sortTasks,
	upsertTask,
} from "./task-logic";
import "./style.css";

export interface TaskInboxPageProps {
	readonly context: PluginClientContext;
}

/** Task Inbox project page authored against the host-supplied Macro facade. */
export function TaskInboxPage(props: TaskInboxPageProps) {
	const [tasks, setTasks] = createSignal<readonly PluginTask[]>([]);
	const [loading, setLoading] = createSignal(true);
	const [creating, setCreating] = createSignal(false);
	const [error, setError] = createSignal<string>();
	const [name, setName] = createSignal("");
	const lifetime = new AbortController();
	const projectId = () => props.context.project.id;

	const fail = (cause: unknown) => {
		if (lifetime.signal.aborted || props.context.signal.aborted) return;
		setError(cause instanceof Error ? cause.message : String(cause));
		setLoading(false);
	};

	const create = async () => {
		const taskName = name().trim();
		if (!taskName || creating()) return;
		setCreating(true);
		setError(undefined);
		try {
			const task = await createPrivateTask(
				props.context.macro,
				projectId(),
				taskName,
				lifetime.signal,
			);
			setTasks((current) => upsertTask(current, task));
			setName("");
		} catch (cause) {
			fail(cause);
		} finally {
			setCreating(false);
		}
	};

	createEffect(() => {
		const abort = () => lifetime.abort(props.context.signal.reason);
		if (props.context.signal.aborted) abort();
		else props.context.signal.addEventListener("abort", abort, { once: true });

		void loadProjectTasks(props.context.macro, projectId(), lifetime.signal)
			.then((items) => {
				if (lifetime.signal.aborted) return;
				setTasks(sortTasks(items));
				setLoading(false);
				return followTaskEvents(
					props.context.macro,
					projectId(),
					(task) => setTasks((current) => upsertTask(current, task)),
					lifetime.signal,
				);
			})
			.catch(fail);

		onCleanup(() => {
			props.context.signal.removeEventListener("abort", abort);
			lifetime.abort();
		});
	});

	return (
		<main class="task-inbox" aria-labelledby="task-inbox-title">
			<header>
				<h1 id="task-inbox-title">Task Inbox</h1>
				<p>Private tasks in this project.</p>
			</header>
			<div class="task-inbox__create">
				<label for="task-inbox-name">Task name</label>
				<div>
					<input
						id="task-inbox-name"
						value={name()}
						disabled={creating()}
						onInput={(event) => setName(event.currentTarget.value)}
						onKeyDown={(event) => {
							if (event.key !== "Enter" || event.isComposing) return;
							event.preventDefault();
							void create();
						}}
					/>
					<button
						type="button"
						disabled={creating()}
						onClick={() => void create()}
					>
						{creating() ? "Creating…" : "Create"}
					</button>
				</div>
			</div>
			<Show when={error()}>{(message) => <p role="alert">{message()}</p>}</Show>
			<Show when={loading()}>
				<p role="status">Loading tasks…</p>
			</Show>
			<Show when={!loading() && !error() && tasks().length === 0}>
				<p class="task-inbox__empty">No tasks yet.</p>
			</Show>
			<Show when={!loading() && tasks().length > 0}>
				<ul>
					<For each={tasks()}>{(task) => <li>{task.name}</li>}</For>
				</ul>
			</Show>
		</main>
	);
}
