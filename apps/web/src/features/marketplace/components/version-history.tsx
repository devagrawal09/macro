import { formatLocale, t } from '@app/lib/i18n';
import { Badge } from '@ui';
import { For, Show } from 'solid-js';
import { type ListingVersion, versionLabel } from '../core/listing';

function publishedOn(createdAt: string): string {
  return new Intl.DateTimeFormat(formatLocale(), {
    dateStyle: 'medium',
  }).format(new Date(createdAt));
}

/**
 * Versions as a list, newest first: each one's number, when it was
 * published, and what its publisher said changed.
 */
export function VersionHistory(props: {
  versions: readonly ListingVersion[];
  /** Names the list for assistive tech. */
  label: string;
}) {
  return (
    <ol aria-label={props.label} class="flex flex-col gap-3">
      <For each={props.versions}>
        {(entry) => (
          <li
            aria-label={versionLabel(entry.version)}
            class="flex flex-col gap-1"
          >
            <div class="flex flex-wrap items-center gap-1.5 text-xs">
              <span class="font-medium text-ink tabular-nums">
                {versionLabel(entry.version)}
              </span>
              <span class="text-ink-extra-muted">
                {publishedOn(entry.createdAt)}
              </span>
              <Show when={entry.forcedMajor}>
                <Badge variant="outline" size="xs">
                  {t('marketplace.historyForcedMajor')}
                </Badge>
              </Show>
            </div>
            <p class="text-sm whitespace-pre-line text-ink-muted">
              {entry.changelog ?? t('marketplace.historyNoChangelog')}
            </p>
          </li>
        )}
      </For>
    </ol>
  );
}
