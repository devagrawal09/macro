import { readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const root = process.argv[2];
if (!root) throw new Error('generated output path is required');

async function format(path) {
  const metadata = await stat(path);
  if (metadata.isFile()) {
    if (!/\.(?:ts|js)$/.test(path)) return;
    const text = await readFile(path, 'utf8');
    const formatted = text.replace(/[ \t]+$/gm, '');
    if (formatted !== text) await writeFile(path, formatted);
    return;
  }
  for (const entry of await readdir(path)) await format(join(path, entry));
}

await format(root);
