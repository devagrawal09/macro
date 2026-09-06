import { describe, expect, it } from 'vitest';
import {
  documentHealthComponentId,
  documentIdFromDocumentHealthComponentId,
} from './documentHealthRoute';

describe('documentHealthRoute', () => {
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
