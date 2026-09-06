import { SERVER_HOSTS } from '@core/constant/servers';
import { Macro } from '@macro/sdk/browser';
import { getAccessToken } from '@service-auth/client';

/** Environment string published to extension slot contexts. */
export const localExtensionEnvironment = import.meta.env.VITE_LOCAL_SERVERS
  ? 'local'
  : 'dev';

const storageHost = SERVER_HOSTS['document-storage-service'];
const getSessionToken = async () => {
  const token = await getAccessToken();
  if (!token) throw new Error('No Macro session token available');
  return token;
};

/**
 * A browser Macro client authenticated with the current refreshable session
 * token. Shared by every directly imported local extension demo so the app
 * holds one SDK client, not one per placement.
 */
export const localMacro = new Macro({
  env: localExtensionEnvironment,
  token: getSessionToken,
  hosts: {
    properties: storageHost,
    storage: storageHost,
  },
  webAppUrl: globalThis.location.origin,
});
