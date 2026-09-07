import { describe, expect, test } from 'bun:test';
import { HOSTS, WEB_APP_URLS } from '../src/config';
import { Macro as BrowserMacro } from '../src/macro.browser';

// The browser entrypoint must resolve hosts from fixed environment defaults
// only. The Node entrypoint is deliberately not asserted here: it discovers a
// running local stack's port map, so its hosts depend on the machine.
describe('browser-safe local resolution', () => {
  test('uses fixed local defaults without portmap discovery', () => {
    const macro = new BrowserMacro({ token: 'token', env: 'local' });
    expect(macro._client.hosts).toEqual(HOSTS.local);
    expect(macro.webAppUrl).toBe(WEB_APP_URLS.local);
  });
  test('explicit host and web app overrides win', () => {
    const macro = new BrowserMacro({
      token: 'token',
      env: 'local',
      hosts: { storage: 'https://storage.test' },
      webAppUrl: 'https://web.test',
    });
    expect(macro._client.hosts.storage).toBe('https://storage.test');
    expect(macro.webAppUrl).toBe('https://web.test');
  });
});

describe('browser-only SDK entry', () => {
  test('requires explicit authentication and never falls back to process env', () => {
    expect(() => new BrowserMacro({ env: 'local' })).toThrow(
      'browser Macro requires an explicit token or auth option',
    );
    const macro = new BrowserMacro({ env: 'local', token: 'browser-token' });
    expect(macro._client.hosts.storage).toBe('http://localhost:8086');
  });
});
