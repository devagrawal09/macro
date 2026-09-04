import { rm } from 'node:fs/promises';
import { Glob } from 'bun';
import { build } from 'esbuild';

const generatedEntries = [...new Glob('generated/*/index.ts').scanSync('.')];

await rm('dist', { recursive: true, force: true });
await build({
  entryPoints: ['src/macro.ts', ...generatedEntries],
  bundle: true,
  format: 'esm',
  splitting: true,
  outdir: 'dist',
  outbase: '.',
  platform: 'neutral',
  target: 'node18',
  external: ['node:*'],
});
await build({
  entryPoints: ['src/macro.browser.ts'],
  bundle: true,
  format: 'esm',
  outfile: 'dist/src/macro.browser.js',
  platform: 'browser',
  target: 'es2022',
});

const browserOutput = await Bun.file('dist/src/macro.browser.js').text();
for (const forbidden of [
  'MACRO_API_KEY',
  'MACRO_BOT_TOKEN',
  'process.env',
  'node:',
]) {
  if (browserOutput.includes(forbidden)) {
    throw new Error(
      `browser SDK output contains forbidden value: ${forbidden}`,
    );
  }
}
