import { fireEvent, render } from '@solidjs/testing-library';
import { describe, expect, it, vi } from 'vitest';
import { TaskCreateControl } from '../src/task-create-control';

describe('TaskCreateControl', () => {
  it('creates exactly once from an explicit button click without a form submit', async () => {
    const create = vi.fn(async () => {});
    const view = render(() => (
      <TaskCreateControl ready={() => true} create={create} />
    ));
    const input = view.getByRole('textbox', { name: 'Task name' });
    const button = view.getByRole('button', { name: 'Create' });

    expect(view.container.querySelector('form')).toBeNull();
    expect(button.getAttribute('type')).toBe('button');
    fireEvent.input(input, { target: { value: ' Click task ' } });
    fireEvent.click(button);

    await vi.waitFor(() => expect(create).toHaveBeenCalledOnce());
    expect(create).toHaveBeenCalledWith('Click task');
    await vi.waitFor(() => expect((input as HTMLInputElement).value).toBe(''));
  });

  it('creates exactly once from an explicitly handled Enter key', async () => {
    const create = vi.fn(async () => {});
    const view = render(() => (
      <TaskCreateControl ready={() => true} create={create} />
    ));
    const input = view.getByRole('textbox', { name: 'Task name' });

    fireEvent.input(input, { target: { value: ' Enter task ' } });
    fireEvent.keyDown(input, { key: 'Enter', code: 'Enter' });

    await vi.waitFor(() => expect(create).toHaveBeenCalledOnce());
    expect(create).toHaveBeenCalledWith('Enter task');
    await vi.waitFor(() => expect((input as HTMLInputElement).value).toBe(''));
  });

  it('keeps the empty and not-ready guards', async () => {
    const create = vi.fn(async () => {});
    let ready = false;
    const view = render(() => (
      <TaskCreateControl ready={() => ready} create={create} />
    ));
    const input = view.getByRole('textbox', { name: 'Task name' });
    const button = view.getByRole('button', { name: 'Create' });

    fireEvent.input(input, { target: { value: 'Not ready' } });
    fireEvent.click(button);
    fireEvent.keyDown(input, { key: 'Enter', code: 'Enter' });
    ready = true;
    fireEvent.input(input, { target: { value: '   ' } });
    fireEvent.click(button);
    fireEvent.keyDown(input, { key: 'Enter', code: 'Enter' });

    await Promise.resolve();
    expect(create).not.toHaveBeenCalled();
  });
});
