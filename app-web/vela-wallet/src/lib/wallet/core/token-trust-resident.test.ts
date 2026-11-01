/**
 * r9-17: the resident's half of a planned scan — the account's facts and its
 * kept coverage go to the core in one `scan_requested`, and what the poll read
 * is kept for the next one (and the next session). The machine is a stand-in
 * here: what it does with them is pinned in Rust (`app_incoming_scan.rs`).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TrustEvent } from '$lib/core/generated/TrustEvent';
import type { TrustView } from '$lib/core/generated/TrustView';
import type { TrustScanFacts } from '$lib/core/generated/TrustScanFacts';

const kv = vi.hoisted(() => new Map<string, string>());
vi.mock('$lib/services/storage', () => ({
	getItem: async (key: string) => kv.get(key) ?? null,
	setItem: async (key: string, value: string) => void kv.set(key, value)
}));
vi.mock('$lib/services/chain-tokens', () => ({ fetchChainTokens: async () => null }));
vi.mock('$lib/services/wallet-api', () => ({ getCachedHeldTokens: () => [] }));

const machine = vi.hoisted(() => ({
	events: [] as TrustEvent[],
	onView: null as ((view: TrustView) => void) | null
}));
vi.mock('./token-trust-session', () => ({
	createTokenTrustSession: (options: { onView: (view: TrustView) => void }) => {
		machine.onView = options.onView;
		const take = (event: TrustEvent) => {
			machine.events.push(event);
			// The core commits its "scanning" view in the same dispatch.
			if (event.type === 'scan_requested') {
				options.onView({
					address: event.address.toLowerCase(),
					scanning: true,
					incoming: [],
					sim: null
				});
			}
		};
		return { start: take, dispatch: take };
	}
}));

const { pollIncoming, SCAN_CURSORS_KEY_PREFIX } = await import('./token-trust-resident');

const SAFE = '0x710311C8d71cc405c5b7782be1367eafa3d78937';
const FACTS: TrustScanFacts = {
	held_chain_ids: [],
	account_networks: [{ chain_id: 5042002, deploy_block: null }],
	space_chain_ids: [11155111, 5042002],
	added_chain_ids: []
};
const KEPT = [{ chain_id: 5042002, covered: [{ from_block: 100, to_block: 200 }] }];
const READ = [{ chain_id: 5042002, covered: [{ from_block: 100, to_block: 300 }] }];

beforeEach(() => {
	kv.clear();
	machine.events = [];
});

describe('a planned scan (r9-17)', () => {
	it('hands the core the facts and the kept coverage, and keeps what it read', async () => {
		kv.set(SCAN_CURSORS_KEY_PREFIX + SAFE.toLowerCase(), JSON.stringify(KEPT));
		const polled = pollIncoming(SAFE, FACTS, [5042002]);
		await vi.waitFor(() => expect(machine.events).toHaveLength(1));
		expect(machine.events[0]).toEqual({
			type: 'scan_requested',
			address: SAFE,
			facts: FACTS,
			cursors: KEPT
		});
		// The poll settles: the core's view carries what it read.
		machine.onView!({
			address: SAFE.toLowerCase(),
			scanning: false,
			incoming: [],
			sim: null,
			cursors: READ
		});
		await polled;
		expect(JSON.parse(kv.get(SCAN_CURSORS_KEY_PREFIX + SAFE.toLowerCase())!)).toEqual(READ);
	});

	it('a garbled kept coverage is no coverage', async () => {
		kv.set(SCAN_CURSORS_KEY_PREFIX + SAFE.toLowerCase(), '{not json');
		const polled = pollIncoming(SAFE, FACTS, [5042002]);
		await vi.waitFor(() => expect(machine.events).toHaveLength(1));
		expect(machine.events[0]).toMatchObject({ type: 'scan_requested', cursors: [] });
		machine.onView!({ address: SAFE.toLowerCase(), scanning: false, incoming: [], sim: null });
		await polled;
	});
});
