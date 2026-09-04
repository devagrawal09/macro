import type { Env, MacroAuth, MacroOpts } from '../config';
import { HOSTS } from '../config';
import { resolveLocalPortmap } from '../local-portmap';
import { MacroClientCore } from './client-core';

/** Root SDK client with Bun/Node environment and local-stack fallbacks. */
export class MacroClient extends MacroClientCore {
  constructor(opts: MacroOpts) {
    const env = resolveEnv(opts);
    super(opts, env, resolveAuth(opts), {
      localPortmap: env === 'local' ? resolveLocalPortmap() : undefined,
      webAppUrl:
        typeof process !== 'undefined' ? process.env.MACRO_WEB_URL : undefined,
      webhookSecret:
        typeof process !== 'undefined'
          ? process.env.MACRO_WEBHOOK_SECRET
          : undefined,
    });
  }
}

function resolveEnv(opts: MacroOpts): Env {
  if (opts.env) return opts.env;
  const fromEnv =
    typeof process !== 'undefined' ? process.env.MACRO_ENV : undefined;
  if (!fromEnv) return 'dev';
  if (!(fromEnv in HOSTS)) {
    throw new Error(
      `invalid MACRO_ENV "${fromEnv}" - expected local, dev, or prod`,
    );
  }
  return fromEnv as Env;
}

function resolveAuth(opts: MacroOpts): MacroAuth {
  if (opts.auth) return opts.auth;
  if (opts.token) return { type: 'user', token: opts.token };
  const envApiKey =
    typeof process !== 'undefined' ? process.env.MACRO_API_KEY : undefined;
  const envBotToken =
    typeof process !== 'undefined' ? process.env.MACRO_BOT_TOKEN : undefined;
  if (envApiKey && envBotToken) {
    throw new Error(
      'both MACRO_API_KEY and MACRO_BOT_TOKEN are set - pass auth to new Macro() to pick one',
    );
  }
  if (envBotToken) return { type: 'bot', token: envBotToken };
  return {
    type: 'user',
    token:
      envApiKey ??
      (() => {
        throw new Error(
          'no Macro API token - set MACRO_API_KEY / MACRO_BOT_TOKEN or pass token/auth to new Macro()',
        );
      }),
  };
}

export type { ServiceName } from '../config';
