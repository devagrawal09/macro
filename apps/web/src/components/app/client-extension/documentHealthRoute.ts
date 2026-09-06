/**
 * The local-only Document Health full page lives in the split layout as a
 * `component` split. Like `reminder-view~<id>`, the component id carries the
 * document id — component params are dropped on URL restore, so the id is the
 * only durable slot. The resulting URL is
 * `/app/component/document-health~<documentId>`.
 */
export const DOCUMENT_HEALTH_COMPONENT = 'document-health';

/** Document created by the checked-in local Document Health demo. */
export const DOCUMENT_HEALTH_DEMO_DOCUMENT_ID =
  '01a0780b-e51a-7c93-918d-335077bffb9a';

const DOCUMENT_HEALTH_COMPONENT_PREFIX = `${DOCUMENT_HEALTH_COMPONENT}~`;

/** The split-layout component id for one document's health page. */
export function documentHealthComponentId(documentId: string): string {
  return `${DOCUMENT_HEALTH_COMPONENT_PREFIX}${documentId}`;
}

/** The document id embedded in a health-page component id, if any. */
export function documentIdFromDocumentHealthComponentId(
  componentId: string
): string | undefined {
  if (!componentId.startsWith(DOCUMENT_HEALTH_COMPONENT_PREFIX)) {
    return undefined;
  }
  const documentId = componentId.slice(DOCUMENT_HEALTH_COMPONENT_PREFIX.length);
  return documentId.length > 0 ? documentId : undefined;
}
