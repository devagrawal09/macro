import DocumentHealthPlugin from '../../../../../../examples/document-health-extension/src/DocumentHealthPlugin';
import { localMacro } from './localMacroClient';

export default function LocalDocumentHealthPlugin(props: {
  documentId: string;
}) {
  return (
    <DocumentHealthPlugin macro={localMacro} documentId={props.documentId} />
  );
}
