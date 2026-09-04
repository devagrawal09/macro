import { Sdk as AgentHarnessSdk } from '../../generated/agent-harness/sdk.gen';
import { Sdk as AuthSdk } from '../../generated/auth/sdk.gen';
import { Sdk as CognitionSdk } from '../../generated/cognition/sdk.gen';
import { Sdk as ContactsSdk } from '../../generated/contacts/sdk.gen';
import { Sdk as EmailSdk } from '../../generated/email/sdk.gen';
import { Sdk as NotificationSdk } from '../../generated/notification/sdk.gen';
import { Sdk as PropertiesSdk } from '../../generated/properties/sdk.gen';
import { Sdk as SearchSdk } from '../../generated/search/sdk.gen';
import { createClient } from '../../generated/storage/client';
import { Sdk as StorageSdk } from '../../generated/storage/sdk.gen';
import type { Bot } from '../../generated/storage/types.gen';
import {
  type Env,
  HOSTS,
  type MacroAuth,
  type MacroOpts,
  type ServiceName,
  WEB_APP_URLS,
} from '../config';
import { BotsNamespace } from '../entities/bots/namespace';
import { User } from '../entities/users/user';
import { MacroEvents } from '../events/receiver';
import type { LocalPortmap } from '../local-portmap';

export class MacroClientCore {
  readonly agentHarness: AgentHarnessSdk;
  readonly auth: AuthSdk;
  readonly cognition: CognitionSdk;
  readonly contacts: ContactsSdk;
  readonly email: EmailSdk;
  readonly notification: NotificationSdk;
  readonly properties: PropertiesSdk;
  readonly search: SearchSdk;
  readonly storage: StorageSdk;
  readonly webAppUrl: string;
  readonly wsVerify?: string;
  readonly events: MacroEvents;
  /** Resolved authentication config (distinct from `auth`, the auth-service SDK). */
  readonly authConfig: MacroAuth;
  /** Resolved service base urls: env defaults, then the local-stack portmap,
   * then `opts.hosts` overrides. */
  readonly hosts: Record<ServiceName, string>;
  /** The local stack's generated port map; only set when env is `local`. */
  readonly localPortmap?: LocalPortmap;
  private readonly requestedAs?: string;
  private selfBotRecord?: Promise<Bot>;
  private selfPrincipal?: Promise<string>;

  constructor(
    opts: MacroOpts,
    env: Env,
    authConfig: MacroAuth,
    runtime: {
      localPortmap?: LocalPortmap;
      webAppUrl?: string;
      webhookSecret?: string;
    } = {},
  ) {
    const hosts = {
      ...HOSTS[env],
      ...runtime.localPortmap?.hosts,
      ...opts.hosts,
    };
    this.hosts = hosts;
    this.localPortmap = runtime.localPortmap;
    this.webAppUrl =
      opts.webAppUrl ??
      runtime.webAppUrl ??
      runtime.localPortmap?.webAppUrl ??
      WEB_APP_URLS[env];
    this.authConfig = authConfig;
    this.requestedAs = opts.requestedAs;
    if (this.requestedAs && this.authConfig.type !== 'bot') {
      throw new Error(
        'requestedAs() requires bot auth — a user token always acts as its own user',
      );
    }
    this.wsVerify = opts.wsVerify;

    this.agentHarness = new AgentHarnessSdk({
      client: this.makeClient(hosts['agent-harness']),
    });
    this.auth = new AuthSdk({ client: this.makeClient(hosts.auth) });
    this.cognition = new CognitionSdk({
      client: this.makeClient(hosts.cognition),
    });
    this.contacts = new ContactsSdk({
      client: this.makeClient(hosts.contacts),
    });
    this.email = new EmailSdk({ client: this.makeClient(hosts.email) });
    this.notification = new NotificationSdk({
      client: this.makeClient(hosts.notification),
    });
    this.properties = new PropertiesSdk({
      client: this.makeClient(hosts.properties),
    });
    this.search = new SearchSdk({ client: this.makeClient(hosts.search) });
    this.storage = new StorageSdk({ client: this.makeClient(hosts.storage) });

    this.events = new MacroEvents(
      this,
      opts.webhookSecret ?? runtime.webhookSecret,
    );
  }

  /** Whether requests have a user identity accepted by acting-user endpoints. */
  hasActingUser(): boolean {
    return this.authConfig.type === 'user' || this.requestedAs !== undefined;
  }

  /**
   * The authenticated bot's own record, fetched once and cached. Bot auth
   * only. Failed lookups are not cached, so a later call retries.
   */
  selfBot(): Promise<Bot> {
    this.selfBotRecord ??= new BotsNamespace(this).me().catch((error) => {
      this.selfBotRecord = undefined;
      throw error;
    });
    return this.selfBotRecord;
  }

  /**
   * The authenticated caller's mentionable principal — `bot|<uuid>` for bot
   * auth, `macro|<email>` for user auth — fetched once and cached. Failed
   * lookups are not cached, so a later call retries.
   */
  myPrincipalId(): Promise<string> {
    this.selfPrincipal ??= (
      this.authConfig.type === 'bot'
        ? this.selfBot().then((bot) => `bot|${bot.id}`)
        : User.me(this).then((user) => user.id)
    ).catch((error) => {
      this.selfPrincipal = undefined;
      throw error;
    });
    return this.selfPrincipal;
  }

  private makeClient(baseUrl: string) {
    const c = createClient({ baseUrl });
    c.interceptors.request.use(async (request) => {
      const source = this.authConfig.token;
      const tok = typeof source === 'function' ? await source() : source;
      if (this.authConfig.type === 'bot') {
        request.headers.set('x-macro-bot-token', tok);
        // A per-call scope wins: the channel webhook fallback pins `user`,
        // the only scope a user-owned bot can present (a team scope with no
        // owning team is rejected outright).
        if (!request.headers.has('x-macro-bot-scope')) {
          request.headers.set(
            'x-macro-bot-scope',
            this.authConfig.scope ?? (this.requestedAs ? 'user' : 'team'),
          );
        }
        if (this.requestedAs) {
          request.headers.set(
            'x-macro-bot-for-macro-user-id',
            this.requestedAs,
          );
        }
      } else {
        if (tok.startsWith('mbot_')) {
          throw new Error(
            "bot API key passed as a user token — use auth: { type: 'bot', token }",
          );
        }
        request.headers.set('Authorization', `Bearer ${tok}`);
      }
      return request;
    });
    return c;
  }
}

export type { ServiceName };
