/** @vitest-environment jsdom */

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	installPluginBootstrap,
	serializePluginBootstrapSource,
	type BootstrapWindow,
} from './pluginBootstrap';

const INIT = { type: 'macro.plugin.init.v1', nonce: 'nonce-1' };
const CONNECT = { type: 'macro.plugin.connect.v1', nonce: 'nonce-1' };
const CONTEXT_MESSAGE = {
	version: 1,
	type: 'macro.plugin.context.v1',
	context: { projectId: 'project-1', capabilities: ['tasks.read'] },
	grant: {
		token: 'fake-grant',
		expiresAt: '2030-01-01T00:00:00.000Z',
		capabilities: ['tasks.read'],
	},
};

interface Harness {
	listener: (event: { data?: unknown; ports?: MessagePort[] }) => void;
	parentPost: ReturnType<typeof vi.fn>;
	root: HTMLElement;
	run: (source: string) => void;
}

function harness(): Harness {
	const listeners: Array<(event: { data?: unknown; ports?: MessagePort[] }) => void> = [];
	const parentPost = vi.fn();
	const root = document.createElement('div');
	root.id = 'root';
	document.body.append(root);
	const fakeWindow = {
		parent: { postMessage: parentPost },
		addEventListener: (
			_type: 'message',
			listener: (event: { data?: unknown; ports?: MessagePort[] }) => void,
		) => {
			listeners.push(listener);
		},
		document,
	} as unknown as BootstrapWindow;
	const run = (source: string) => {
		new Function('window', source)(fakeWindow);
	};
	return {
		listener: (event) => {
			for (const listener of listeners) listener(event);
		},
		parentPost,
		root,
		run,
	};
}

afterEach(() => {
	document.getElementById('root')?.remove();
	delete (
		globalThis as { __MACRO_PLUGIN_ENTRY__?: unknown }
	).__MACRO_PLUGIN_ENTRY__;
});

describe('plugin bootstrap serialization', () => {
	it('serialized source answers init and mounts the entry on the port context', () => {
		const h = harness();
		expect(() => h.run(serializePluginBootstrapSource())).not.toThrow();

		h.listener({ data: INIT });
		expect(h.parentPost).toHaveBeenCalledWith(
			{ type: 'macro.plugin.ready.v1', nonce: 'nonce-1' },
			'*',
		);

		const mount = vi.fn();
		(globalThis as { __MACRO_PLUGIN_ENTRY__?: unknown }).__MACRO_PLUGIN_ENTRY__ = {
			mount,
		};

		const port = {
			onmessage: null as ((event: { data: unknown }) => void) | null,
			postMessage: vi.fn(),
			close: vi.fn(),
			start: vi.fn(),
		} as unknown as MessagePort;
		h.listener({ data: CONNECT, ports: [port] });
		port.onmessage?.({ data: CONTEXT_MESSAGE });

		expect(mount).toHaveBeenCalledWith(h.root, CONTEXT_MESSAGE.context);

		// Fatal plugin errors are reported back over the port, sanitized.
		mount.mockImplementation(() => {
			throw new Error('boom');
		});
		port.onmessage?.({ data: CONTEXT_MESSAGE });
		expect(port.postMessage).toHaveBeenCalledWith({
			version: 1,
			type: 'macro.plugin.fatal.v1',
			message: 'boom',
		});
	});

	it('direct installation works against a fake window object', () => {
		const h = harness();
		const directListeners: Array<(event: unknown) => void> = [];
		const parentPost = vi.fn();
		installPluginBootstrap({
			parent: { postMessage: parentPost },
			addEventListener: ((_type: string, fn: (event: unknown) => void) => {
				directListeners.push(fn);
			}) as BootstrapWindow['addEventListener'],
			document,
		});
		directListeners[0]({ data: INIT });
		expect(parentPost).toHaveBeenCalledWith(
			{ type: 'macro.plugin.ready.v1', nonce: 'nonce-1' },
			'*',
		);
	});

	it('rejects connect messages with the wrong nonce or no port', () => {
		const h = harness();
		h.run(serializePluginBootstrapSource());
		h.listener({ data: INIT });
		const mount = vi.fn();
		(globalThis as { __MACRO_PLUGIN_ENTRY__?: unknown }).__MACRO_PLUGIN_ENTRY__ = {
			mount,
		};
		const port = {
			onmessage: null,
			close: vi.fn(),
			start: vi.fn(),
		} as unknown as MessagePort;

		h.listener({
			data: { type: 'macro.plugin.connect.v1', nonce: 'other' },
			ports: [port],
		});
		h.listener({ data: CONNECT });
		expect(mount).not.toHaveBeenCalled();

		h.listener({ data: CONNECT, ports: [port] });
		port.onmessage?.({ data: CONTEXT_MESSAGE });
		expect(mount).toHaveBeenCalledOnce();
	});

	it('ignores malformed init payloads with excess keys', () => {
		const h = harness();
		h.run(serializePluginBootstrapSource());
		h.listener({ data: { ...INIT, extra: true } });
		expect(h.parentPost).not.toHaveBeenCalled();
	});
});
