import type { Env } from './config';

/** DOM attribute that marks a supported Macro client-extension mount point. */
export const MACRO_EXTENSION_SLOT_ATTRIBUTE = 'data-macro-extension-slot';

/** DOM attribute containing the versioned JSON context for a mount point. */
export const MACRO_EXTENSION_CONTEXT_ATTRIBUTE = 'data-macro-extension-context';

/**
 * A placement currently exposed by the Macro web client.
 *
 * - `entity-sidebar`: a section in an open entity's right sidebar.
 * - `full-page`: a directly navigable page dedicated to one entity.
 */
export type MacroExtensionPlacement = 'entity-sidebar' | 'full-page';

/** Entity context supplied to an entity-scoped extension placement. */
export interface MacroExtensionEntity {
  /** Canonical Macro entity id. */
  id: string;
  /** Public entity type name. */
  type: 'document';
}

/** Version 1 context published by a Macro extension slot. */
export interface MacroExtensionContext {
  /** Host contract version. */
  apiVersion: 1;
  /** API environment used by this host. */
  environment: Env;
  /** Placement represented by the host element. */
  placement: MacroExtensionPlacement;
  /** Entity currently represented by the placement. */
  entity: MacroExtensionEntity;
}

/** Options for observing supported extension slots in the Macro page. */
export interface ObserveMacroExtensionSlotsOptions {
  /** Placement to mount into. */
  placement: MacroExtensionPlacement;
  /**
   * Mount extension UI into a host. Return a disposer for subscriptions and
   * rendered UI. Use Shadow DOM or an iframe to isolate the contribution.
   */
  mount(
    host: HTMLElement,
    context: MacroExtensionContext,
  ): undefined | (() => void);
  /** Receives malformed-context, mount, and cleanup errors. */
  onError?: (error: unknown) => void;
  /** Stop observing when aborted. */
  signal?: AbortSignal;
  /** Observation root. Defaults to the current document. */
  root?: ParentNode;
}

interface MountedSlot {
  readonly contextValue: string;
  readonly dispose?: () => void;
}

/**
 * Observe Macro's versioned DOM extension slots and keep one contribution
 * mounted for each matching host. Context changes dispose and remount the
 * contribution; removed hosts are disposed automatically.
 *
 * Returns an idempotent stop function.
 */
export function observeMacroExtensionSlots(
  options: ObserveMacroExtensionSlotsOptions,
): () => void {
  if (options.signal?.aborted) return () => {};

  const root = options.root ?? document;
  const selector = `[${MACRO_EXTENSION_SLOT_ATTRIBUTE}="${options.placement}"]`;
  const mounted = new Map<HTMLElement, MountedSlot>();
  let stopped = false;

  const report = (error: unknown) => {
    try {
      options.onError?.(error);
    } catch {
      // Error observers must not interrupt slot lifecycle cleanup.
    }
  };
  const dispose = (host: HTMLElement, slot: MountedSlot) => {
    mounted.delete(host);
    try {
      slot.dispose?.();
    } catch (error) {
      report(error);
    }
  };
  const scan = () => {
    if (stopped) return;

    for (const [host, slot] of mounted) {
      if (!host.isConnected || !host.matches(selector)) dispose(host, slot);
    }

    for (const host of Array.from(
      root.querySelectorAll<HTMLElement>(selector),
    )) {
      const contextValue =
        host.getAttribute(MACRO_EXTENSION_CONTEXT_ATTRIBUTE) ?? '';
      const previous = mounted.get(host);
      if (previous?.contextValue === contextValue) continue;
      if (previous) dispose(host, previous);

      let context: MacroExtensionContext;
      try {
        const value = JSON.parse(
          contextValue,
        ) as Partial<MacroExtensionContext>;
        if (
          value.apiVersion !== 1 ||
          value.placement !== options.placement ||
          (value.environment !== 'local' &&
            value.environment !== 'dev' &&
            value.environment !== 'prod') ||
          value.entity?.type !== 'document' ||
          typeof value.entity.id !== 'string' ||
          value.entity.id.length === 0
        ) {
          throw new Error('unsupported Macro extension context');
        }
        context = value as MacroExtensionContext;
      } catch (error) {
        mounted.set(host, { contextValue });
        report(error);
        continue;
      }

      try {
        const cleanup = options.mount(host, context);
        mounted.set(host, {
          contextValue,
          dispose: typeof cleanup === 'function' ? cleanup : undefined,
        });
      } catch (error) {
        mounted.set(host, { contextValue });
        report(error);
      }
    }
  };

  const observer = new MutationObserver(scan);
  observer.observe(root, {
    attributeFilter: [
      MACRO_EXTENSION_SLOT_ATTRIBUTE,
      MACRO_EXTENSION_CONTEXT_ATTRIBUTE,
    ],
    attributes: true,
    childList: true,
    subtree: true,
  });
  scan();

  const stop = () => {
    if (stopped) return;
    stopped = true;
    options.signal?.removeEventListener('abort', stop);
    observer.disconnect();
    for (const [host, slot] of mounted) dispose(host, slot);
  };
  options.signal?.addEventListener('abort', stop, { once: true });
  if (options.signal?.aborted) stop();
  return stop;
}
