import { DocumentHealthCard } from '@macro-examples/document-health-extension/src/DocumentHealthCard';
import { localMacro } from './localMacroClient';

export default function LocalDocumentHealthPlugin(props: {
  documentId: string;
}) {
  return (
    <DocumentHealthCard macro={localMacro} documentId={props.documentId} />
  );
}
