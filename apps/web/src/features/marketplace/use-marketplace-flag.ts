import { useFeatureFlag } from '@app/lib/analytics/posthog';
import {
  enableHtmlCodeMode,
  enableMarketplace,
} from '@core/constant/featureFlags';
import type { Accessor } from 'solid-js';

/**
 * Whether the marketplace page, its sidebar entry and the publish action
 * are available. Its pages run programs, so the server serves it only with
 * HTML code mode too. `loading` is not off: wait for it before deciding.
 */
export function useMarketplaceFlag(): Accessor<{
  enabled: boolean;
  loading: boolean;
}> {
  const marketplace = useFeatureFlag(enableMarketplace);
  const codeMode = useFeatureFlag(enableHtmlCodeMode);

  return () => ({
    enabled: marketplace().enabled && codeMode().enabled,
    loading: marketplace().loading || codeMode().loading,
  });
}
