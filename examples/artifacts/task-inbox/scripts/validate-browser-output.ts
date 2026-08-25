import { readdir } from 'node:fs/promises';

const distUrl = new URL('../dist/', import.meta.url);
const files = await readdir(distUrl);
if (files.length !== 1 || files[0] !== 'index.html') {
  throw new Error(`expected only dist/index.html, found: ${files.join(', ')}`);
}
const html = await Bun.file(new URL('index.html', distUrl)).text();
const apiKeyName = ['MACRO', 'API_KEY'].join('_');
const forbidden = [
  apiKeyName,
  'MACRO_BOT_TOKEN',
  'MACRO_WEBHOOK_SECRET',
  'process.env',
  'node:',
  'MACRO_HTML_DOCUMENT_ID',
  'VITE_TASK_INBOX_DOCUMENT_ID',
  'deployment cancelled',
  '[processed] ',
  '<form',
];
const sentinel = process.env[apiKeyName];
if (sentinel) forbidden.push(sentinel);
for (const value of forbidden) {
  if (html.includes(value)) {
    throw new Error(`browser output contains forbidden value: ${value}`);
  }
}
const projectId = process.env.MACRO_PROJECT_ID;
if (!projectId || !html.includes(projectId)) {
  throw new Error('browser output does not contain the configured project ID');
}
