import { SidePanel } from '@components/app/side-panel';
import { LOCAL_ONLY } from '@core/constant/featureFlags';
import { Show } from 'solid-js';

interface EntitySidebarExtensionSlotProps {
  entityId: string;
  entityType: 'document';
  order?: number;
}

/**
 * Local-only proof of the versioned DOM boundary used by browser extensions.
 * The host publishes entity context only; credentials remain extension-owned.
 */
export function EntitySidebarExtensionSlot(
  props: EntitySidebarExtensionSlotProps
) {
  const context = () =>
    JSON.stringify({
      apiVersion: 1,
      environment: 'dev',
      placement: 'entity-sidebar',
      entity: { id: props.entityId, type: props.entityType },
    });

  return (
    <Show when={LOCAL_ONLY}>
      <SidePanel.Section
        id="local-client-extension"
        title="Client extension"
        defaultOpen
        order={props.order}
      >
        <div
          data-macro-extension-slot="entity-sidebar"
          data-macro-extension-context={context()}
          class="min-h-20 rounded-lg border border-dashed border-edge-muted p-3 text-xs text-ink-muted"
        >
          <span data-macro-extension-fallback>
            Load a local browser extension to fill this supported mount point.
          </span>
        </div>
      </SidePanel.Section>
    </Show>
  );
}
