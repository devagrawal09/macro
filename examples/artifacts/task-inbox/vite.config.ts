import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import { viteSingleFile } from 'vite-plugin-singlefile';

function required(name: 'MACRO_PROJECT_ID' | 'MACRO_ENV'): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

export default defineConfig(() => {
  const projectId = required('MACRO_PROJECT_ID');
  const env = required('MACRO_ENV');
  if (env !== 'local' && env !== 'dev') {
    throw new Error('MACRO_ENV must be local or dev');
  }
  const storageHost = process.env.MACRO_STORAGE_HOST?.trim() || undefined;
  const webAppUrl = process.env.MACRO_WEB_APP_URL?.trim() || undefined;
  return {
    plugins: [solid(), viteSingleFile()],
    define: {
      __MACRO_PROJECT_ID__: JSON.stringify(projectId),
      __MACRO_ENV__: JSON.stringify(env),
      __MACRO_STORAGE_HOST__: JSON.stringify(storageHost),
      __MACRO_WEB_APP_URL__: JSON.stringify(webAppUrl),
    },
    build: { outDir: 'dist', emptyOutDir: true },
  };
});
