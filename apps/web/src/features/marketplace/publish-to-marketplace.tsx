import { t } from '@app/lib/i18n';
import type { FileOperation } from '@components/app/split-layout/components/SplitFileMenu';
import { useUserId } from '@core/context/user';
import UploadIcon from '@icon/system/regular/upload-simple.svg';
import type { TemplateVariable } from '@service-agent-harness/generated/schemas';
import { openDialog } from '@ui';
import { type Accessor, getOwner } from 'solid-js';
import { useMarketplaceFlag } from './use-marketplace-flag';
import { PublishListingDialog } from './views/publish-listing-dialog';

/**
 * The “Publish to marketplace…” file operation for an HTML document its
 * viewer owns, or none while the marketplace is off or its rollout is still
 * loading. The host decides which documents offer it.
 */
export function usePublishToMarketplaceOperation(document: {
  id: string;
  /** Starts the listing's title. */
  name: Accessor<string>;
  /** The editor has changes whose save hasn't landed. */
  unsaved: Accessor<boolean>;
}): Accessor<FileOperation | undefined> {
  const flag = useMarketplaceFlag();
  const userId = useUserId();
  // The dialog closes with the document, and reads its queries' context.
  const owner = getOwner();

  return () => {
    if (!flag().enabled) return undefined;

    return {
      group: 'file',
      label: t('marketplace.publishAction'),
      icon: UploadIcon,
      action: () => {
        openDialog(
          PublishListingDialog,
          () => ({
            target: {
              kind: 'artifact' as const,
              id: document.id,
              unsaved: document.unsaved(),
            },
            name: document.name(),
            userId: userId(),
          }),
          { owner, key: `marketplace-publish:${document.id}` }
        );
      },
    };
  };
}

/**
 * Publishing a routine its viewer owns, or nothing while the marketplace is
 * off or its rollout is still loading. The host decides which routines offer
 * it and reads their references.
 */
export function usePublishRoutineToMarketplace(routine: {
  id: Accessor<string | undefined>;
  name: Accessor<string>;
  /** Its references, marked as when sharing it as a template. */
  variables(routineId: string): Promise<TemplateVariable[]>;
}): Accessor<(() => void) | undefined> {
  const flag = useMarketplaceFlag();
  const userId = useUserId();
  const owner = getOwner();

  return () => {
    const id = routine.id();
    if (!flag().enabled || !id) return undefined;

    return () => {
      openDialog(
        PublishListingDialog,
        () => ({
          target: {
            kind: 'routine' as const,
            id,
            variables: () => routine.variables(id),
          },
          name: routine.name(),
          userId: userId(),
        }),
        { owner, key: `marketplace-publish:${id}` }
      );
    };
  };
}
