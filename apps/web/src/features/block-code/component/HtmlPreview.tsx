import { DEV_MODE_ENV, LOCAL_ONLY } from '@core/constant/featureFlags';
import { blockMetadataSignal, blockTextSignal } from '@core/signal/load';
import { getMacroApiToken } from '@service-auth/fetch';
import { createEffect, createMemo, onCleanup, onMount } from 'solid-js';

type GrantState = {
  generation: number;
  nonce: string | null;
  claimed: boolean;
  controller: AbortController;
  port1?: MessagePort;
  port2?: MessagePort;
};

const exactMessage = (
  value: unknown,
  type: string,
): value is { type: string; nonce: string } => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  return (
    keys.length === 2 &&
    keys[0] === 'nonce' &&
    keys[1] === 'type' &&
    record.type === type &&
    typeof record.nonce === 'string' &&
    record.nonce.length > 0
  );
};

export function taskInboxGate(args: {
  local: boolean;
  develop: boolean;
  configuredDocumentId: string;
  currentDocumentId: string | undefined;
}): boolean {
  return (
    (args.local || args.develop) &&
    args.configuredDocumentId.length > 0 &&
    args.currentDocumentId === args.configuredDocumentId
  );
}

export function HtmlPreview() {
  const blockText = createMemo(blockTextSignal.get);
  const blockMetadata = blockMetadataSignal.get;
  let iframe: HTMLIFrameElement | undefined;
  let active = true;
  let generation = 0;
  let grant: GrantState | undefined;

  const configuredDocumentId = () =>
    import.meta.env.VITE_TASK_INBOX_DOCUMENT_ID?.trim() ?? '';
  const gatesPass = (node: HTMLIFrameElement) => {
    const configuredId = configuredDocumentId();
    return (
      active &&
      taskInboxGate({
        local: LOCAL_ONLY,
        develop: DEV_MODE_ENV,
        configuredDocumentId: configuredId,
        currentDocumentId: blockMetadata()?.documentId,
      }) &&
      iframe === node
    );
  };

  const invalidate = () => {
    generation += 1;
    grant?.controller.abort();
    grant?.port1?.close();
    grant?.port2?.close();
    grant = undefined;
  };

  // A srcdoc replacement invalidates the old load before the next load event.
  createEffect(() => {
    blockText();
    invalidate();
  });

  const onLoad = () => {
    const node = iframe;
    invalidate();
    if (!node || !gatesPass(node) || !node.contentWindow) return;

    const currentGeneration = generation;
    const nonce = crypto.randomUUID();
    grant = {
      generation: currentGeneration,
      nonce,
      claimed: false,
      controller: new AbortController(),
    };
    node.contentWindow.postMessage(
      { type: 'macro-task-inbox-init', nonce },
      '*',
    );
  };

  const onReady = async (event: MessageEvent) => {
    const node = iframe;
    const state = grant;
    if (
      !node ||
      event.source !== node.contentWindow ||
      event.origin !== 'null' ||
      !exactMessage(event.data, 'macro-task-inbox-ready') ||
      !state ||
      state.claimed ||
      state.nonce !== event.data.nonce ||
      state.generation !== generation ||
      !gatesPass(node)
    ) {
      return;
    }

    // Claim and consume synchronously, before the first await.
    state.claimed = true;
    const captured = {
      generation: state.generation,
      nonce: state.nonce,
      iframe: node,
      controller: state.controller,
      state,
    } as const;
    state.nonce = null;

    let token: string;
    try {
      token = await getMacroApiToken();
    } catch {
      return;
    }

    if (
      captured.controller.signal.aborted ||
      !active ||
      iframe !== captured.iframe ||
      !captured.iframe.contentWindow ||
      generation !== captured.generation ||
      grant !== captured.state ||
      !captured.state.claimed ||
      !gatesPass(captured.iframe)
    ) {
      return;
    }

    const channel = new MessageChannel();
    captured.state.port1 = channel.port1;
    captured.state.port2 = channel.port2;
    try {
      captured.iframe.contentWindow.postMessage(
        { type: 'macro-task-inbox-grant', nonce: captured.nonce },
        '*',
        [channel.port2],
      );
      captured.state.port2 = undefined;
      channel.port1.postMessage({
        type: 'macro-task-inbox-token',
        nonce: captured.nonce,
        token,
      });
      channel.port1.close();
      captured.state.port1 = undefined;
    } catch {
      channel.port1.close();
      channel.port2.close();
      captured.state.port1 = undefined;
      captured.state.port2 = undefined;
    }
  };

  onMount(() => window.addEventListener('message', onReady));
  onCleanup(() => {
    active = false;
    window.removeEventListener('message', onReady);
    invalidate();
  });

  return (
    <div class="size-full bg-surface overflow-auto touch:pt-(--mobile-content-inset-top) touch:pb-(--mobile-content-inset-bottom)">
      <iframe
        title="HTML preview"
        class="size-full border-0"
        sandbox="allow-scripts"
        referrerPolicy="no-referrer"
        ref={iframe}
        onLoad={onLoad}
        srcdoc={blockText() ?? ''}
      />
    </div>
  );
}
