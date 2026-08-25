import { expect, test } from 'bun:test';
import { rm } from 'node:fs/promises';

const browserSdkOutput = new URL(
  '../../../../packages/sdk/dist/src/macro.browser.js',
  import.meta.url,
);

test('SDK prebuild recreates a missing browser entrypoint', async () => {
  await rm(browserSdkOutput, { force: true });
  const child = Bun.spawn([process.execPath, 'run', 'prebuild:sdk'], {
    cwd: new URL('..', import.meta.url).pathname,
    stdout: 'pipe',
    stderr: 'pipe',
  });
  const [exitCode, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ]);
  expect(exitCode, `${stdout}
${stderr}`).toBe(0);
  expect(await Bun.file(browserSdkOutput).exists()).toBe(true);
});
