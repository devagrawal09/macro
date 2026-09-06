import { describe, expect, it } from 'vitest';
import {
  DOCUMENT_HEALTH_DEMO_DOCUMENT_ID,
  documentHealthComponentId,
  documentIdFromDocumentHealthComponentId,
} from './documentHealthRoute';

describe('documentHealthRoute', () => {
  it('targets the checked-in local demo document', () => {
    expect(documentHealthComponentId(DOCUMENT_HEALTH_DEMO_DOCUMENT_ID)).toBe(
      'document-health~01a0780b-e51a-7c93-918d-335077bffb9a'
    );
  });

  it('round-trips a document id through the component id', () => {
    const componentId = documentHealthComponentId('doc_123');
    expect(componentId).toBe('document-health~doc_123');
    expect(documentIdFromDocumentHealthComponentId(componentId)).toBe(
      'doc_123'
    );
  });

  it('rejects other component ids and an empty document id', () => {
    expect(documentIdFromDocumentHealthComponentId('document-health')).toBe(
      undefined
    );
    expect(documentIdFromDocumentHealthComponentId('document-health~')).toBe(
      undefined
    );
    expect(documentIdFromDocumentHealthComponentId('reminder-view~r1')).toBe(
      undefined
    );
  });
});
