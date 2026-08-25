const required = (name: 'MACRO_API_KEY' | 'MACRO_HTML_DOCUMENT_ID' | 'MACRO_ENV') => {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
};

export function overwriteUrl(storageHost: string, documentId: string): string {
  return `${storageHost.replace(/\/$/, '')}/documents/${encodeURIComponent(documentId)}/simple_save`;
}

async function main() {
  const token = required('MACRO_API_KEY');
  const documentId = required('MACRO_HTML_DOCUMENT_ID');
  const env = required('MACRO_ENV');
  if (env !== 'local' && env !== 'dev') throw new Error('deploy is restricted to local or dev');
  const defaultHost = env === 'local' ? 'http://localhost:8090/api' : 'https://cloud-storage-dev.macro.com/api';
  const storageHost = process.env.MACRO_STORAGE_HOST?.trim() || defaultHost;
  const answer = prompt(`Overwrite fixed Macro HTML document ${documentId}? Type the exact document ID to continue:`);
  if (answer !== documentId) throw new Error('deployment cancelled');
  const html = Bun.file(new URL('../dist/index.html', import.meta.url));
  if (!(await html.exists())) throw new Error('dist/index.html is missing');
  const form = new FormData();
  form.append('file', html, 'index.html');
  const response = await fetch(overwriteUrl(storageHost, documentId), { method: 'PUT', headers: { Authorization: `Bearer ${token}` }, body: form });
  if (!response.ok) throw new Error(`simple_save failed: ${response.status} ${response.statusText}`);
  console.log(`Overwrote fixed document ${documentId}`);
}

if (import.meta.main) await main();
