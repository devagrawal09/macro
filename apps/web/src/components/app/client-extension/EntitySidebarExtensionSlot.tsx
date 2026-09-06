import { SidePanel } from '@components/app/side-panel';
import { useSplitLayout } from '@components/app/split-layout/layout';
import { LOCAL_ONLY } from '@core/constant/featureFlags';
import { lazy, Show, Suspense } from 'solid-js';
import { documentHealthComponentId } from './documentHealthRoute';
import { localExtensionEnvironment } from './localMacroClient';

const LocalDocumentHealthPlugin = lazy(
  () => import('./LocalDocumentHealthPlugin')
);

interface EntitySidebarExtensionSlotProps {
  entityId: string;
  entityType: 'document';
  order?: number;
}

/**
 * Local-only host for the directly imported demo and its browser-extension slot.
 * The DOM contract still publishes context only; extensions own their credentials.
 */
export function EntitySidebarExtensionSlot(
  props: EntitySidebarExtensionSlotProps
) {
  const { openWithSplit } = useSplitLayout();
  const context = () =>
    JSON.stringify({
      apiVersion: 1,
      environment: localExtensionEnvironment,
      placement: 'entity-sidebar',
      entity: { id: props.entityId, type: props.entityType },
    });

  return (
    <Show when={LOCAL_ONLY}>
      <SidePanel.Section
        id="local-client-extension"
        title="Document Health"
        defaultOpen
        order={props.order}
      >
        <div
          data-macro-extension-slot="entity-sidebar"
          data-macro-extension-context={context()}
          class="min-h-20"
        >
          <div data-macro-extension-fallback>
            <Suspense
              fallback={
                <span class="text-xs text-ink-muted">Loading plugin...</span>
              }
            >
              <LocalDocumentHealthPlugin documentId={props.entityId} />
            </Suspense>
          </div>
        </div>
        <button
          type="button"
          class="mt-2 rounded-md px-1 py-1 text-xs text-ink-muted underline decoration-current/20 decoration-[max(1px,0.1em)] underline-offset-2 hover:bg-hover hover:text-ink"
          onClick={() =>
            openWithSplit(
              {
                type: 'component',
                id: documentHealthComponentId(props.entityId),
              },
              { referredFrom: null }
            )
          }
        >
          Open full page
        </button>
      </SidePanel.Section>
    </Show>
  );
}
