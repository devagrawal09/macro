import type { MacroAuth, MacroOpts } from '../config';
import { MacroClientCore } from './client-core';

/** Browser-only SDK client. It never reads process environment variables. */
export class MacroClient extends MacroClientCore {
  constructor(opts: MacroOpts) {
    super(opts, opts.env ?? 'dev', resolveBrowserAuth(opts));
  }
}

function resolveBrowserAuth(opts: MacroOpts): MacroAuth {
  if (opts.auth) return opts.auth;
  if (opts.token) return { type: 'user', token: opts.token };
  throw new Error('browser Macro requires an explicit token or auth option');
}

export type { ServiceName } from '../config';
