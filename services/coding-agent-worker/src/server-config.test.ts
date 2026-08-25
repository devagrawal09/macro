import { expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';
import { resolveWorkerServerPort } from './server-config';

test('uses explicit worker-owned webhook receiver port', () => {
  expect(resolveWorkerServerPort({ SDK_WEBHOOK_HOST_RECEIVER_PORT: 9123 })).toBe(9123);
});

test('defaults the worker-owned webhook receiver port to 8787', async () => {
  Object.assign(process.env, {
    DAYTONA_API_KEY: 'test-daytona',
    MACRO_BOT_TOKEN: 'mbot_test',
    MACRO_USER_ID: 'test-user',
    PUBLIC_URL: 'https://worker.example.com',
    GITHUB_TOKEN: 'test-github',
  });
  delete process.env.SDK_WEBHOOK_HOST_RECEIVER_PORT;
  const { env } = await import('./env');
  expect(env.SDK_WEBHOOK_HOST_RECEIVER_PORT).toBe(8787);
});

test('checked-in server consumer no longer reads SDK local portmap', async () => {
  const source = await readFile(new URL('../index.ts', import.meta.url), 'utf8');
  expect(source).toContain('resolveWorkerServerPort(env)');
  expect(source).not.toContain('localPortmap');
});
