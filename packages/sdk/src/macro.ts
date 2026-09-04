import type { MacroOpts } from './config';
import { MacroBase } from './macro-base';
import { MacroClient } from './utils/client';

export type { MacroOpts } from './config';
export type {
  ConnectEventsOptions,
  EventConnection,
  EventFilter,
  ListenOptions,
  MacroEvents,
} from './events/receiver';
export type {
  DeliveryHeaders,
  EventHandler,
  EventMap,
  EventName,
  EventPayload,
  MacroEvent,
} from './events/types';
export {
  here,
  type Interpolation,
  type Mentionable,
  type MentionPart,
  msg,
  type RichMessage,
  type SimpleMention,
  toBody,
  wrapXml,
} from './mentions';

export class Macro<T extends MacroOpts = MacroOpts> extends MacroBase<
  MacroClient,
  Macro<T>
> {
  constructor(opts: T) {
    super(
      new MacroClient(opts),
      (requestedAs) => new Macro<T>({ ...opts, requestedAs }),
    );
  }
}
