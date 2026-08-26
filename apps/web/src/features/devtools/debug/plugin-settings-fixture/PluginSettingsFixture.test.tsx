/**
 * @vitest-environment jsdom
 */

import { fireEvent, render, screen } from '@solidjs/testing-library';
import { describe, expect, it } from 'vitest';
import PluginSettingsFixture from './PluginSettingsFixture';
import { createPluginSettingsStore } from './store';

describe('PluginSettingsFixture', () => {
  it('renders the mock installation, handlers, and runs', () => {
    render(() => <PluginSettingsFixture />);

    expect(screen.getByText('Task Tools')).toBeTruthy();
    expect(screen.getByText('Normalize created task')).toBeTruthy();
    expect(screen.getByText('Unverified local sideload')).toBeTruthy();
    expect(screen.getByText(/Task not found/)).toBeTruthy();
  });

  it('toggles enabled and handler pause through the store', () => {
    const store = createPluginSettingsStore();
    render(() => <PluginSettingsFixture store={store} />);

    const enabledButton = screen
      .getAllByRole('button')
      .find((button) => button.textContent?.includes('Enabled'));
    expect(enabledButton).toBeTruthy();

    fireEvent.click(enabledButton!);
    expect(store.installation().enabled).toBe(false);

    const handlerButton = screen
      .getAllByRole('button')
      .find((button) => button.textContent?.includes('Normalize created task'));
    fireEvent.click(handlerButton!);
    expect(store.installation().handlers[0].paused).toBe(true);
  });
});
