/**
 * What the home shows, the same in every document (`wallet-sync.ts`).
 *
 * The owner's report: a wallet tab and the side panel, $1,376.87 and
 * $1,373.97 for one wallet — each document read its balances at its own
 * moment, and a round that settled in one never reached the other. Nor did a
 * display currency or a network chosen there: IndexedDB raises no `storage`
 * event. Driven here through a fake channel and the balance bridge with its
 * resident stubbed: a settled round is announced, a document showing the
 * same account reads again past its token cache, and that read is never
 * announced back (no two documents keep each other reading).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BalanceView } from '$lib/core/generated/BalanceView';

const resident = vi.hoisted(() => {
	let listener: ((view: unknown) => void) | null = null;
	const state = { view: null as unknown };
	return {
		state,
		dispatch: vi.fn(),
		emit(view: unknown) {
			state.view = view;
			listener?.(view);
		},
		subscribe(fn: (view: unknown) => void) {
			listener = fn;
			return () => {};
		}
	};
});

vi.mock('./balance-resident', async (original) => {
	const real = await original<typeof import('./balance-resident')>();
	return {
		INITIAL_VIEW: real.INITIAL_VIEW,
		ensureBalanceDashboard: () => {},
		dispatchBalance: resident.dispatch,
		balanceView: () => resident.state.view ?? real.INITIAL_VIEW,
		balanceSettledFor: () => true,
		rereadHoldings: async () => {},
		subscribeBalanceDashboard: resident.subscribe
	};
});
vi.mock('$lib/core/client', () => ({ loadCore: async () => {} }));
vi.mock('$lib/services/networks', () => ({
	ensureCustomNetworks: async () => {},
	ensureShownNetworks: async () => {},
	inSpace: () => true
}));
vi.mock('$lib/services/holdings-cache', () => ({
	loadHoldings: async () => [],
	saveHoldings: async () => {}
}));
vi.mock('$lib/network-space/network-space.svelte', () => ({
	networkSpace: { current: 'mainnet', subscribe: () => () => {} }
}));
vi.mock('./privacy-sync', () => ({ onPrivacyElsewhere: () => {} }));

import {
	announceWalletSync,
	onWalletSyncElsewhere,
	readSyncMessage,
	useWalletSyncChannel
} from './wallet-sync';

const ADDRESS = '0x4AFb65cbcccb9b6f64fd0962c5a55f86cae5bdc5';

/** A channel of the test's own: what this document posts, and a way to deliver another's. */
function fakeChannel(use = useWalletSyncChannel) {
	const posted: unknown[] = [];
	const listeners = new Set<(event: { data: unknown }) => void>();
	use({
		postMessage: (m) => posted.push(m),
		addEventListener: (_t, l) => listeners.add(l),
		removeEventListener: (_t, l) => listeners.delete(l)
	});
	return {
		posted,
		deliver: (data: unknown) => {
			for (const l of [...listeners]) l({ data });
		}
	};
}

function view(at: number | null, address: string | null = ADDRESS): BalanceView {
	return {
		...(resident.state.view as BalanceView | null),
		address,
		last_refreshed_at_ms: at,
		updated_at_ms: at,
		networks_read: true,
		unread_chain_ids: [],
		tokens: []
	} as BalanceView;
}

const refreshes = () =>
	resident.dispatch.mock.calls.filter(
		([e]) => (e as { type: string }).type === 'refresh_requested'
	);

/**
 * A fresh document: its own sync module (on a fake channel) and balance
 * bridge — showing the balance, as the wallet home does, unless `noted`
 * (a page that only notes the account, perf-10).
 */
async function bootBalance(noted = false) {
	vi.resetModules();
	const sync = await import('./wallet-sync');
	const channel = fakeChannel(sync.useWalletSyncChannel);
	const { balance } = await import('./balance.svelte');
	await balance.boot();
	if (noted) await balance.noteAccount(ADDRESS);
	else await balance.setAccount(ADDRESS);
	resident.dispatch.mockClear();
	return { balance, channel };
}

// A loaded full run (both projects at once, `bun run test:unit`) overran the
// default 5 s here: a first test pays for compiling its module graph (the
// real core, where one loads), not its own time (the house pattern,
// bridge.test.ts).
vi.setConfig({ testTimeout: 30_000 });

beforeEach(() => {
	resident.dispatch.mockClear();
	resident.state.view = null;
	vi.stubGlobal('document', { visibilityState: 'visible' });
});

afterEach(() => {
	useWalletSyncChannel(undefined);
	vi.unstubAllGlobals();
});

describe('the words between documents', () => {
	it('reads only its own shapes', () => {
		expect(readSyncMessage({ topic: 'currency' })).toEqual({ topic: 'currency' });
		expect(readSyncMessage({ topic: 'networks', extra: 1 })).toEqual({ topic: 'networks' });
		expect(readSyncMessage({ topic: 'holdings', address: ADDRESS, space: 'mainnet' })).toEqual({
			topic: 'holdings',
			address: ADDRESS,
			space: 'mainnet'
		});
		expect(readSyncMessage({ topic: 'holdings', address: 1 })).toBeNull();
		expect(readSyncMessage('changed')).toBeNull();
		expect(readSyncMessage(null)).toBeNull();
	});

	it('posts and hears through the channel; without one, nothing throws', () => {
		const channel = fakeChannel();
		const heard: unknown[] = [];
		const stop = onWalletSyncElsewhere((m) => heard.push(m));
		announceWalletSync({ topic: 'currency' });
		channel.deliver({ topic: 'networks' });
		channel.deliver({ topic: 'nonsense' });
		stop();
		channel.deliver({ topic: 'networks' });
		expect(channel.posted).toEqual([{ topic: 'currency' }]);
		expect(heard).toEqual([{ topic: 'networks' }]);
		useWalletSyncChannel(null);
		expect(() => announceWalletSync({ topic: 'currency' })).not.toThrow();
		expect(onWalletSyncElsewhere(() => {})()).toBeUndefined();
	});
});

describe('a settled round reaches the other documents', () => {
	it('announces a round this document settled', async () => {
		const { channel } = await bootBalance();
		resident.emit(view(null));
		resident.emit(view(1_000));
		expect(channel.posted).toEqual([{ topic: 'holdings', address: ADDRESS, space: 'mainnet' }]);
	});

	it('reads again, past the token cache, when another document settled the same account', async () => {
		const { channel } = await bootBalance();
		resident.emit(view(1_000));
		channel.deliver({ topic: 'holdings', address: ADDRESS.toLowerCase(), space: 'mainnet' });
		expect(refreshes()).toEqual([[{ type: 'refresh_requested', force: true, pull: false }]]);
		// …and that read's settle is not announced back.
		channel.posted.length = 0;
		resident.emit(view(2_000));
		expect(channel.posted).toEqual([]);
		// The next round of its own is.
		resident.emit(view(3_000));
		expect(channel.posted).toHaveLength(1);
	});

	it('ignores another account, another space', async () => {
		const { channel } = await bootBalance();
		resident.emit(view(1_000));
		channel.deliver({ topic: 'holdings', address: '0x' + '1'.repeat(40), space: 'mainnet' });
		channel.deliver({ topic: 'holdings', address: ADDRESS, space: 'testnet' });
		expect(refreshes()).toEqual([]);
	});

	it('a page that never showed the balance (Settings, perf-10) reads nothing for it', async () => {
		const { channel } = await bootBalance(true);
		resident.emit(view(1_000));
		channel.deliver({ topic: 'holdings', address: ADDRESS, space: 'mainnet' });
		expect(refreshes()).toEqual([]);
	});

	it('hidden, the read waits for the person to come back', async () => {
		const { balance, channel } = await bootBalance();
		resident.emit(view(1_000));
		vi.stubGlobal('document', { visibilityState: 'hidden' });
		channel.deliver({ topic: 'holdings', address: ADDRESS, space: 'mainnet' });
		expect(refreshes()).toEqual([]);
		balance.focused();
		expect(refreshes()).toEqual([[{ type: 'refresh_requested', force: true, pull: false }]]);
		balance.focused();
		expect(refreshes()).toHaveLength(1);
	});
});
