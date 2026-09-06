import { useSplitPanelOrThrow } from '@components/app/split-layout/layoutUtils';
import { openDocument } from '@core/component/LexicalMarkdown/component/core/BlockLink';
import { createCallback } from '@solid-primitives/rootless';
import { ErrorBoundary, onMount, Suspense } from 'solid-js';
import DocumentHealthPage from '../../../../../../examples/document-health-extension/src/DocumentHealthPage';
import { localExtensionEnvironment, localMacro } from './localMacroClient';

/**
 * Local-only full-page host for the directly imported demo. Publishes the
 * versioned `full-page` extension slot on the page container so a browser
 * extension can claim the same placement; the direct import is the fallback
 * content. Plugin failures stay inside this split.
 */
export default function LocalDocumentHealthFullPage(props: {
  documentId: string;
}) {
  const panel = useSplitPanelOrThrow();
  onMount(() => panel.handle.setDisplayName('Document Health'));

  const openSourceDocument = createCallback((info: { fileType?: string }) => {
    openDocument(info.fileType ?? 'md', props.documentId);
  });

  const context = () =>
    JSON.stringify({
      apiVersion: 1,
      environment: localExtensionEnvironment,
      placement: 'full-page',
      entity: { id: props.documentId, type: 'document' },
    });

  return (
    <div
      data-macro-extension-slot="full-page"
      data-macro-extension-context={context()}
      class="h-full min-h-0"
    >
      <div data-macro-extension-fallback class="h-full min-h-0">
        <ErrorBoundary
          fallback={
            <div class="flex h-full items-center justify-center p-6 text-xs text-ink-muted">
              Document Health failed to render. Reopen the page to retry.
            </div>
          }
        >
          {/* Keep resource suspension inside the plugin instead of blanking
              the split to the layout-level Suspense boundary. */}
          <Suspense
            fallback={
              <div class="flex h-full items-center justify-center p-6 text-xs text-ink-muted">
                Loading Document Health...
              </div>
            }
          >
            <DocumentHealthPage
              macro={localMacro}
              documentId={props.documentId}
              onOpenDocument={openSourceDocument}
            />
          </Suspense>
        </ErrorBoundary>
      </div>
    </div>
  );
}
