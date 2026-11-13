/**
 * Connecting with clientmcp.dev (spec 106): the tab leaves for the site only
 * when the site answers (set-11).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: true }));
vi.mock('$lib/core/client', () => ({
	loadCore: async () => {},
	clientmcpInstallationId: () => 'install-1',
	clientmcpLinkBegin: () =>
		JSON.stringify({ ok: { authorize_url: 'https://clientmcp.dev/authorize?state=s' } }),
	clientmcpLinkAccept: () => '{}',
	clientmcpLinkExchange: () => '{}',
	clientmcpLinkReturned: () => '{}'
}));

import { agentLink, reachable } from './link.svelte';

function memoryStorage() {
	const map = new Map<string, string>();
	return {
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		removeItem: (k: string) => void map.delete(k),
		has: (k: string) => map.has(k)
	};
}

let assign: ReturnType<typeof vi.fn>;
let session: ReturnType<typeof memoryStorage>;

beforeEach(() => {
	assign = vi.fn();
	session = memoryStorage();
	vi.stubGlobal('location', { origin: 'https://getvela.app', assign });
	vi.stubGlobal('sessionStorage', session);
	vi.stubGlobal('localStorage', memoryStorage());
	agentLink.problem = null;
	agentLink.phase = 'idle';
});

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('agentLink.start (set-11)', () => {
	it('stays in the wallet and says so when clientmcp.dev cannot be reached', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => Promise.reject(new TypeError('Failed to fetch')))
		);
		await agentLink.start('/en/settings', 'Web browser');
		expect(assign).not.toHaveBeenCalled();
		expect(agentLink.problem).toEqual({ kind: 'network' });
		expect(agentLink.phase).toBe('idle');
		// Nothing is left pending for a return that cannot come.
		expect(session.has('vela.agent.link')).toBe(false);
	});

	it('goes to clientmcp.dev when it answers', async () => {
		const fetch = vi.fn(async () => new Response(null, { status: 200 }));
		vi.stubGlobal('fetch', fetch);
		await agentLink.start('/en/settings', 'Web browser');
		expect(fetch).toHaveBeenCalledWith('https://clientmcp.dev/', expect.anything());
		expect(assign).toHaveBeenCalledWith('https://clientmcp.dev/authorize?state=s');
		expect(agentLink.problem).toBeNull();
	});
});

describe('reachable', () => {
	it('a probe that never answers is unreachable once the wait is up', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(
				(_url: string, init: RequestInit) =>
					new Promise((_resolve, reject) =>
						init.signal?.addEventListener('abort', () => reject(init.signal?.reason))
					)
			)
		);
		expect(await reachable('https://clientmcp.dev/authorize', 20)).toBe(false);
	});
});
