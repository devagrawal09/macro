import { describe, expect, test } from 'bun:test';
import { HOSTS, WEB_APP_URLS } from '../src/config';
import { Macro } from '../src/macro';

type InspectableMacro = { _client: { hosts: typeof HOSTS.local } };

describe('browser-safe local resolution', () => {
  test('uses fixed local defaults without portmap discovery', () => {
    const macro = new Macro({ token: 'token', env: 'local' });
    const client = (macro as unknown as InspectableMacro)._client;
    expect(client.hosts).toEqual(HOSTS.local);
    expect(macro.webAppUrl).toBe(WEB_APP_URLS.local);
  });
  test('explicit host and web app overrides win', () => {
    const macro = new Macro({ token: 'token', env: 'local', hosts: { storage: 'https://storage.test' }, webAppUrl: 'https://web.test' });
    const client = (macro as unknown as InspectableMacro)._client;
    expect(client.hosts.storage).toBe('https://storage.test');
    expect(macro.webAppUrl).toBe('https://web.test');
  });
});
