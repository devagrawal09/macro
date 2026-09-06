import { afterEach, describe, expect, test } from 'bun:test';
import {
  MACRO_EXTENSION_CONTEXT_ATTRIBUTE,
  observeMacroExtensionSlots,
} from '../src/extensions';

const originalMutationObserver = globalThis.MutationObserver;

afterEach(() => {
  globalThis.MutationObserver = originalMutationObserver;
});

describe('observeMacroExtensionSlots', () => {
  test('remounts changed context and disposes removed hosts', () => {
    let callback: MutationCallback = () => {};
    let observer = {} as MutationObserver;
    let disconnects = 0;
    class FakeMutationObserver {
      constructor(next: MutationCallback) {
        callback = next;
        observer = this as unknown as MutationObserver;
      }
      observe() {}
      disconnect() {
        disconnects += 1;
      }
      takeRecords(): MutationRecord[] {
        return [];
      }
    }
    globalThis.MutationObserver =
      FakeMutationObserver as unknown as typeof MutationObserver;

    let connected = true;
    let contextValue = JSON.stringify({
      apiVersion: 1,
      environment: 'dev',
      placement: 'entity-sidebar',
      entity: { id: 'doc_1', type: 'document' },
    });
    const host = {
      get isConnected() {
        return connected;
      },
      getAttribute: (name: string) =>
        name === MACRO_EXTENSION_CONTEXT_ATTRIBUTE ? contextValue : null,
      matches: () => true,
    } as unknown as HTMLElement;
    const root = {
      querySelectorAll: () => (connected ? [host] : []),
    } as unknown as ParentNode;
    const mounted: string[] = [];
    const disposed: string[] = [];
    const errors: unknown[] = [];

    const stop = observeMacroExtensionSlots({
      placement: 'entity-sidebar',
      root,
      onError: (error) => errors.push(error),
      mount: (_host, context) => {
        mounted.push(context.entity.id);
        return () => disposed.push(context.entity.id);
      },
    });
    expect(mounted).toEqual(['doc_1']);

    contextValue = contextValue.replace('doc_1', 'doc_2');
    callback([], observer);
    expect(mounted).toEqual(['doc_1', 'doc_2']);
    expect(disposed).toEqual(['doc_1']);

    connected = false;
    callback([], observer);
    expect(disposed).toEqual(['doc_1', 'doc_2']);

    connected = true;
    contextValue = contextValue.replace('dev', 'staging');
    callback([], observer);
    expect(mounted).toEqual(['doc_1', 'doc_2']);
    expect(errors).toHaveLength(1);

    stop();
    stop();
    expect(disconnects).toBe(1);
  });

  test('mounts full-page context and rejects placement mismatches', () => {
    class FakeMutationObserver {
      observe() {}
      disconnect() {}
      takeRecords(): MutationRecord[] {
        return [];
      }
    }
    globalThis.MutationObserver =
      FakeMutationObserver as unknown as typeof MutationObserver;

    let contextValue = JSON.stringify({
      apiVersion: 1,
      environment: 'local',
      placement: 'full-page',
      entity: { id: 'doc_9', type: 'document' },
    });
    const host = {
      isConnected: true,
      getAttribute: (name: string) =>
        name === MACRO_EXTENSION_CONTEXT_ATTRIBUTE ? contextValue : null,
      matches: () => true,
    } as unknown as HTMLElement;
    const root = {
      querySelectorAll: () => [host],
    } as unknown as ParentNode;
    const mounted: string[] = [];
    const errors: unknown[] = [];

    contextValue = contextValue.replace('full-page', 'entity-sidebar');
    const stopMismatched = observeMacroExtensionSlots({
      placement: 'full-page',
      root,
      onError: (error) => errors.push(error),
      mount: (_host, context) => {
        mounted.push(context.entity.id);
        return undefined;
      },
    });
    stopMismatched();
    // A slot publishing another placement's context must not mount.
    expect(mounted).toEqual([]);
    expect(errors).toHaveLength(1);

    contextValue = contextValue.replace('entity-sidebar', 'full-page');
    const stop = observeMacroExtensionSlots({
      placement: 'full-page',
      root,
      onError: (error) => errors.push(error),
      mount: (_host, context) => {
        mounted.push(`${context.placement}:${context.entity.id}`);
        return undefined;
      },
    });
    stop();
    expect(mounted).toEqual(['full-page:doc_9']);
    expect(errors).toHaveLength(1);
  });

  test('does not observe an already-aborted signal', () => {
    let observers = 0;
    class FakeMutationObserver {
      constructor() {
        observers += 1;
      }
    }
    globalThis.MutationObserver =
      FakeMutationObserver as unknown as typeof MutationObserver;

    const controller = new AbortController();
    controller.abort();
    const stop = observeMacroExtensionSlots({
      placement: 'entity-sidebar',
      root: { querySelectorAll: () => [] } as unknown as ParentNode,
      signal: controller.signal,
      mount: () => {
        throw new Error('must not mount');
      },
    });

    stop();
    expect(observers).toBe(0);
  });
});
