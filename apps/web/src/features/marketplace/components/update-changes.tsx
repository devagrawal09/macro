import { t } from '@app/lib/i18n';
import { Show } from 'solid-js';
import {
  type Listing,
  type ListingVersion,
  versionLabel,
} from '../core/listing';
import { VersionHistory } from './version-history';

/**
 * What an update brings, for its confirmation: the changelogs of the
 * versions between the viewer's and the one they take. `changes` is absent
 * when they couldn't be read; the update can go ahead anyway.
 */
export function UpdateChanges(props: {
  changes: readonly ListingVersion[] | undefined;
  /** The viewer edited their routine, which the update replaces. */
  replacesEdits: boolean;
}) {
  return (
    <div class="flex flex-col gap-3">
      <Show when={props.replacesEdits}>
        <p>{t('marketplace.updateEditedBody')}</p>
      </Show>
      <Show
        when={props.changes}
        fallback={<p>{t('marketplace.updateChangesFailed')}</p>}
      >
        {(changes) => (
          <Show
            when={changes().length > 0}
            fallback={<p>{t('marketplace.updateNoChanges')}</p>}
          >
            <VersionHistory
              versions={changes()}
              label={t('marketplace.updateChangesLabel')}
            />
          </Show>
        )}
      </Show>
    </div>
  );
}

/**
 * The update button's label: the newer major waiting, or for a pinned
 * install the exact version it would take.
 */
export function updateLabel(listing: Listing): string {
  const target = listing.latestVersion;
  const newerMajor =
    typeof listing.installedMajor === 'number' &&
    listing.installedMajor < target.major;
  return newerMajor
    ? t('marketplace.updateTo', { major: target.major })
    : t('marketplace.updateToVersion', { version: versionLabel(target) });
}
