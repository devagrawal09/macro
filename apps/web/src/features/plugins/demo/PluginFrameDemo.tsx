import {
	DEV_MODE_ENV,
	LOCAL_ONLY,
} from '@core/constant/featureFlags';
import { Show, type JSX } from 'solid-js';
import { PluginFrame } from '../host/PluginFrame';
import type { PluginClientContext, PluginGrant } from '../contract';
import { buildTaskToolsDemoDocument } from './taskToolsDemoDocument';

const DEMO_CONTEXT: PluginClientContext = {
	projectId: 'demo-project',
	capabilities: ['tasks.read', 'tasks.create'],
};

/**
 * FAKE grant data. This checkpoint never calls the backend: the real
 * delegated-grant minting plugs in behind `fetchGrant` later.
 */
const fetchFakeGrant = async (
	context: PluginClientContext,
): Promise<PluginGrant> => ({
	token: 'fake-local-dev-grant-token',
	expiresAt: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
	capabilities: [...context.capabilities],
});

/**
 * Dev-only demo page mounting the built Task Tools client bundle through
 * the generic PluginFrame host. Fails closed outside local/dev.
 */
export default function PluginFrameDemo(): JSX.Element {
	return (
		<Show when={LOCAL_ONLY || DEV_MODE_ENV}>
			<div class="flex h-dvh flex-col">
				<header class="border-b border-secondary px-4 py-2 text-sm">
					Plugin frame demo — UNVERIFIED LOCAL/DEV PLUGIN (fake grants)
				</header>
				<main class="min-h-0 flex-1">
					<PluginFrame
						title="Task Tools plugin"
						srcdoc={buildTaskToolsDemoDocument()}
						context={DEMO_CONTEXT}
						fetchGrant={fetchFakeGrant}
					/>
				</main>
			</div>
		</Show>
	);
}
