import { Glob } from 'bun';
import { build } from 'esbuild';
import { rm } from 'node:fs/promises';

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
});
await build({
  entryPoints: ['src/macro.browser.ts'],
  bundle: true,
  format: 'esm',
  outfile: 'dist/src/macro.browser.js',
  platform: 'browser',
  target: 'es2022',
});

const browserGraph = new Map<string, string>();
async function collectBrowserGraph(path: string): Promise<void> {
  const outputPath = new URL(path, `file://${process.cwd()}/`).pathname;
  if (browserGraph.has(outputPath)) return;
  const output = await Bun.file(outputPath).text();
  browserGraph.set(outputPath, output);
  const imports = output.matchAll(
    /(?:from\s*|import\s*)["'](\.[^"']+\.js)["']/g,
  );
  for (const match of imports) {
    await collectBrowserGraph(
      new URL(match[1], `file://${outputPath}`).pathname,
    );
  }
}
await collectBrowserGraph('dist/src/macro.browser.js');
for (const [path, output] of browserGraph) {
  for (const forbidden of ['MACRO_API_KEY', 'process.env', 'node:']) {
    if (output.includes(forbidden)) {
      throw new Error(
        `browser SDK output ${path} contains forbidden value: ${forbidden}`,
      );
    }
  }
}
