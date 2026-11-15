/**
 * Doing an intent, where the shell decides (spec 103): where an agent's
 * change is shown — a site it opened is in the browser's own tab, so the
 * page the person is on stays (agent-12); a whole page already open goes
 * back to its top, nothing left over it (agent-13); the page the person is
 * on is not navigated to again — and a cleared history leaves the feed
 * showing none of it (agent-23).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const seams = vi.hoisted(() => ({
	goto: vi.fn(() => Promise.resolve()),
	// A route matches a route path; the packaged extension's files do not.
	match: vi.fn(async (path: string) => (path.includes('.html') ? null : { id: path }))
}));
vi.mock('$app/navigation', () => ({ goto: seams.goto }));
vi.mock('$app/paths', () => ({
	resolve: (route: string, params: { locale: string }) => route.replace('[locale]', params.locale),
	match: seams.match
}));
vi.mock('#lib/services/preferences.svelte.ts', async (original) => ({
	...(await original<typeof import('#lib/services/preferences.svelte.ts')>()),
	preferences: { setLanguage: vi.fn() }
}));
const cleared = vi.hoisted(() => ({ items: [] as string[], history: 0 }));
vi.mock('#lib/services/device-storage.ts', () => ({
	clearStorageItem: async (item: string) => {
		cleared.items.push(item);
		return [];
	},
	clearAllCaches: async () => []
}));
vi.mock('#lib/wallet/core/feed.svelte.ts', () => ({
	feed: {
		historyCleared: () => {
			cleared.history += 1;
		}
	}
}));

const { perform, show } = await import('./perform');
const { screenDesk } = await import('#lib/flows/screen-desk.svelte.ts');

function at(pathname: string, search = '') {
	vi.stubGlobal('location', { pathname, search, assign: vi.fn() });
}

beforeEach(() => {
	seams.goto.mockClear();
	vi.unstubAllGlobals();
});

describe('show', () => {
	it('opens nothing for a site the agent opened: the person stays where they are', async () => {
		at('/en/settings');
		const started = Date.now();
		expect(await show({ type: 'explore' })).toBeNull();
		expect(seams.goto).not.toHaveBeenCalled();
		expect(Date.now() - started).toBeLessThan(1000);
	});

	it('takes a page already open back to its top, without a new history step', async () => {
		at('/en/contacts');
		const present = vi.fn(() => null);
		const detach = screenDesk.attach(['contacts'], present);
		try {
			expect(await show({ type: 'contacts' })).toBeNull();
			expect(present).toHaveBeenCalledWith({ type: 'contacts' });
			expect(seams.goto).not.toHaveBeenCalled();
		} finally {
			detach();
		}
	});

	it('goes to the page when it is elsewhere, and waits for nothing it does not draw', async () => {
		at('/en/settings');
		expect(await show({ type: 'contacts' })).toBeNull();
		expect(seams.goto).toHaveBeenCalledWith('/en/contacts');
	});
});

describe('clearing the transaction history', () => {
	it('has the feed read its cleared store again, so nothing cleared still shows', async () => {
		const world = {} as Parameters<typeof perform>[1];
		expect(
			await perform({ type: 'clear_storage_item', item: 'transactions' }, world, 0, 'agent')
		).toBeNull();
		expect(cleared.items).toEqual(['transactions']);
		expect(cleared.history).toBe(1);
		await perform({ type: 'clear_storage_item', item: 'rates' }, world, 0, 'agent');
		expect(cleared.history).toBe(1);
	});
});

describe('choosing a language', () => {
	const choose = (value: string) =>
		perform(
			{ type: 'set_preference', preference: { kind: 'language', value } },
			{} as Parameters<typeof perform>[1],
			0,
			'agent'
		);

	it('goes to this very page in the chosen locale, through the router', async () => {
		at('/en/settings', '?tab=1');
		await choose('ja');
		expect(seams.goto).toHaveBeenCalledWith('/ja/settings?tab=1');
		expect(location.assign).not.toHaveBeenCalled();
	});

	it('loads the packaged extension’s file as a document, which no route matches', async () => {
		at('/en/settings.html');
		await choose('ja');
		expect(seams.goto).not.toHaveBeenCalled();
		expect(location.assign).toHaveBeenCalledWith('/ja/settings.html');
	});
});
