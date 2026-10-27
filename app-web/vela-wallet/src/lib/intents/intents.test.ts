/**
 * The intents' way through (spec 103): the core's real check (the shipped
 * wasm), the shell's own rules, and residents played by a world the test
 * changes — as the machines would once a step reaches them.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, intentCheck } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import type { IntentAction } from '$lib/core/generated/IntentAction';
import type { IntentCheck } from '$lib/core/generated/IntentCheck';
import type { IntentScreen } from '$lib/core/generated/IntentScreen';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { NetNetworkRow } from '$lib/core/generated/NetNetworkRow';
import type { NetView } from '$lib/core/generated/NetView';
import type { SessionView } from '$lib/session/generated/SessionView';
import { ACTIVITY_KEPT, Intents, type IntentOutcome } from './intents.svelte';
import { agentState } from './state';
import type { World } from './world';

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

const ALICE = '0x00000000219ab540356cbb839cbe05303d7705fa';
const BOB = '0x1111111111111111111111111111111111111111';
const MINE = '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

function person(address: string, name: string) {
	return {
		address,
		name,
		resolved_name: null,
		resolved_source: null,
		kind: 'unknown',
		favorite: false,
		note: null,
		tx_count: 0,
		last_used_ms: 0,
		first_seen_ms: 0,
		source: 'manual'
	};
}

function book(names: Record<string, string>): ContactsView {
	return {
		loaded: true,
		contacts: Object.entries(names).map(([address, name]) => person(address, name)),
		sections: [],
		groups: [],
		last_import: null,
		import_failure: null,
		import_failure_key: null,
		export: null,
		recipient: null
	} as unknown as ContactsView;
}

function session(): SessionView {
	return {
		loading: false,
		has_wallet: true,
		address: MINE,
		active_index: 0,
		accounts: [{ index: 0, account: { address: MINE, name: 'Main' } }],
		allowed_route: 'wallet',
		sign_out: null
	} as unknown as SessionView;
}

function world(): World {
	return {
		session: session(),
		contacts: book({ [ALICE]: 'Alice' }),
		currency: null,
		feeTier: null,
		preferences: {
			theme: 'system',
			language: 'auto',
			textScale: 'standard',
			numberFormat: 'auto',
			dateFormat: 'auto',
			timeFormat: 'auto',
			analytics: false
		},
		agent: { enabled: true, wallets: [session().address] },
		networkSpace: 'mainnet',
		operations: [],
		balance: null,
		feed: null,
		networks: null
	};
}

/** The residents, played: `apply` is what the owning machine does with a step. */
function harness(apply: (action: IntentAction, now: World) => string | null | void = () => {}) {
	let now = world();
	const performed: IntentAction[] = [];
	const revealed: IntentScreen[] = [];
	const intents = new Intents({
		ready: async () => {},
		check: (actor, action, seen) =>
			JSON.parse(
				intentCheck(
					actor,
					JSON.stringify(action),
					JSON.stringify({
						agent_enabled: seen.agent.enabled,
						wallets: seen.agent.wallets,
						wallet: seen.session?.address ?? ''
					})
				)
			) as IntentCheck,
		world: () => now,
		perform: async (action) => {
			performed.push(action);
			return apply(action, now) ?? null;
		},
		now: () => Date.now(),
		reveal: async (screen) => {
			revealed.push(screen);
		},
		approvalMs: 300,
		observeMs: 300,
		pollMs: 5
	});
	return {
		intents,
		performed,
		revealed,
		get world() {
			return now;
		},
		set world(next: World) {
			now = next;
		}
	};
}

const code = (outcome: IntentOutcome) =>
	outcome.status === 'rejected' ? outcome.reason.code : 'completed';

const tick = (ms = 20) => new Promise((resolve) => setTimeout(resolve, ms));

const deleteAlice: IntentAction = { type: 'delete_contact', address: ALICE };
const saveBob: IntentAction = { type: 'save_contact', address: BOB, name: 'Bob' };

describe('the person and an agent, one way in (spec 103)', () => {
	it('answers completed only once the change is seen', async () => {
		const h = harness();
		const pending = h.intents.invoke('person', saveBob);
		await tick();
		expect(h.performed).toEqual([saveBob]);
		let settled = false;
		void pending.then(() => (settled = true));
		await tick();
		expect(settled, 'performed, not yet seen').toBe(false);
		h.world = { ...h.world, contacts: book({ [ALICE]: 'Alice', [BOB]: 'Bob' }) };
		expect(code(await pending)).toBe('completed');
	});

	it('opens where an agent’s change is seen, once it is done', async () => {
		const h = harness((action, now) => {
			if (action.type === 'save_contact')
				h.world = { ...now, contacts: book({ [ALICE]: 'Alice', [action.address]: action.name }) };
		});
		expect(code(await h.intents.invoke('person', saveBob))).toBe('completed');
		expect(h.revealed, 'the person is where they did it').toEqual([]);
		const carol = `0x${'c'.repeat(40)}`;
		const saveCarol: IntentAction = { type: 'save_contact', address: carol, name: 'Carol' };
		expect(code(await h.intents.invoke('agent', saveCarol))).toBe('completed');
		expect(h.revealed).toEqual([{ type: 'contact', address: carol }]);

		const refused = harness(() => 'storage is full');
		expect(code(await refused.intents.invoke('agent', saveBob))).toBe('refused');
		expect(refused.revealed, 'nothing changed: nothing to show').toEqual([]);
	});

	it('does not claim a change that never shows', async () => {
		const h = harness();
		expect(code(await h.intents.invoke('person', saveBob))).toBe('not_observed');
	});

	it('passes on the owner’s refusal', async () => {
		const h = harness(() => 'storage is full');
		const outcome = await h.intents.invoke('person', saveBob);
		expect(outcome).toEqual({
			status: 'rejected',
			reason: { code: 'refused', detail: 'storage is full' }
		});
	});

	it('asks nothing for what already holds', async () => {
		const h = harness();
		const outcome = await h.intents.invoke('agent', {
			type: 'save_contact',
			address: ALICE,
			name: 'Alice'
		});
		expect(code(outcome)).toBe('completed');
		expect(h.performed).toEqual([]);
	});

	it('names what is not there', async () => {
		const h = harness();
		const outcome = await h.intents.invoke('person', { type: 'delete_contact', address: BOB });
		expect(code(outcome)).toBe('not_found');
		expect(h.performed).toEqual([]);
	});

	it('refuses malformed input with the core’s own words', async () => {
		const h = harness();
		const outcome = await h.intents.invoke('agent', { type: 'delete_contact', address: '0x1' });
		expect(code(outcome)).toBe('invalid_input');
	});
});

describe('who may ask', () => {
	it('keeps the agent off its own connection', async () => {
		const h = harness();
		const outcome = await h.intents.invoke('agent', {
			type: 'configure_agent',
			enabled: null,
			bridge_url: 'wss://elsewhere.example/connect',
			client_token: null
		});
		expect(code(outcome)).toBe('person_only');
		expect(h.performed).toEqual([]);
	});

	it('refuses a switched-off agent, and not the person', async () => {
		const h = harness();
		h.world = { ...h.world, agent: { enabled: false, wallets: [] } };
		expect(code(await h.intents.invoke('agent', saveBob))).toBe('agent_off');
		void h.intents.invoke('person', saveBob);
		await tick();
		expect(h.performed).toEqual([saveBob]);
	});

	it('holds an agent’s manage-level request until the person approves', async () => {
		const h = harness((action, now) => {
			if (action.type === 'delete_contact') h.world = { ...now, contacts: book({}) };
		});
		const pending = h.intents.invoke('agent', deleteAlice);
		await tick();
		expect(h.performed, 'nothing before the approval').toEqual([]);
		expect(h.intents.approvals).toHaveLength(1);
		expect(h.intents.approvals[0].risk).toBe('manage');
		expect(h.intents.activity[0].stage).toBe('approving');

		h.intents.answer(h.intents.approvals[0].id, true);
		expect(code(await pending)).toBe('completed');
		expect(h.performed).toEqual([deleteAlice]);
		expect(h.intents.approvals).toEqual([]);
	});

	it('does nothing the person denies', async () => {
		const h = harness();
		const pending = h.intents.invoke('agent', { type: 'clear_caches' });
		await tick();
		h.intents.answer(h.intents.approvals[0].id, false);
		expect(code(await pending)).toBe('denied');
		expect(h.performed).toEqual([]);
	});

	it('lets an unanswered approval lapse', async () => {
		const h = harness();
		expect(code(await h.intents.invoke('agent', { type: 'erase_device' }))).toBe(
			'approval_timed_out'
		);
		expect(h.intents.approvals).toEqual([]);
		expect(h.performed).toEqual([]);
	});

	it('stops the agent, and only the agent', async () => {
		const h = harness((action, now) => {
			if (action.type === 'save_contact')
				h.world = { ...now, contacts: book({ [ALICE]: 'Alice', [BOB]: 'Bob' }) };
		});
		const theirs = h.intents.invoke('agent', deleteAlice);
		const mine = h.intents.invoke('person', saveBob);
		await tick();
		h.intents.stop();
		expect(code(await theirs)).toBe('stopped');
		expect(code(await mine)).toBe('completed');
		expect(h.intents.approvals).toEqual([]);
	});

	it('looks again after an approval: the wallet may have changed under it', async () => {
		const h = harness();
		const pending = h.intents.invoke('agent', deleteAlice);
		await tick();
		h.world = { ...h.world, contacts: book({}) };
		h.intents.answer(h.intents.approvals[0].id, true);
		expect(code(await pending)).toBe('not_found');
		expect(h.performed).toEqual([]);
	});

	it('leaves an agent’s sign-out to the session’s own sheet', async () => {
		const h = harness((_, now) => {
			h.world = {
				...now,
				session: { ...now.session!, sign_out: { pending_upload_warning: true, account_count: 1 } }
			};
		});
		const pending = h.intents.invoke('agent', { type: 'sign_out' });
		await tick();
		expect(h.intents.approvals, 'no generic approval').toEqual([]);
		expect(h.performed).toEqual([{ type: 'sign_out' }]);
		// They keep the wallet: the sheet goes, the wallet stays.
		h.world = { ...h.world, session: { ...h.world.session!, sign_out: null } };
		expect(code(await pending)).toBe('denied');
	});
});

describe('what the person sees of the agent', () => {
	it('logs the agent’s calls, newest first, and not the person’s', async () => {
		const h = harness();
		for (let i = 0; i < ACTIVITY_KEPT + 3; i++) {
			await h.intents.invoke('agent', { type: 'show', screen: { type: 'home' } });
		}
		await h.intents.invoke('person', { type: 'show', screen: { type: 'home' } });
		expect(h.intents.activity).toHaveLength(ACTIVITY_KEPT);
		expect(h.intents.activity.every((entry) => entry.outcome?.status === 'completed')).toBe(true);
		expect(h.intents.activity[0].id).toBe(ACTIVITY_KEPT + 3);
	});
});

function net(
	chain_id: number,
	custom: boolean,
	rpc_url = `https://rpc.${chain_id}.example`
): NetNetworkRow {
	return {
		id: custom ? `custom-${chain_id}` : `net-${chain_id}`,
		chain_id,
		display_name: `Chain ${chain_id}`,
		native_symbol: 'ETH',
		is_custom: custom,
		rpc_url,
		explorer_url: `https://scan.${chain_id}.example`,
		bundler_url: '',
		rpc_health: null,
		explorer_health: null,
		rpc_chain_mismatch: null,
		rpc_save_deferred: false,
		is_testnet: false
	};
}

function networks(rows: NetNetworkRow[]): NetView {
	return {
		loaded: true,
		networks: rows,
		wizard: { phase: 'idle', error: null },
		endpoints: [],
		providers: [{ provider: 'alchemy', key: 'sk-secret', has_key: true, test: null }],
		last_added_chain_id: null
	} as unknown as NetView;
}

describe('the rest of what the person can do (spec 103)', () => {
	it('turns the balance eye only when it is not already so', async () => {
		const h = harness((action, now) => {
			if (action.type === 'set_balance_privacy')
				h.world = { ...now, balance: { hidden: action.hidden } as BalanceView };
		});
		h.world = { ...h.world, balance: { hidden: true } as BalanceView };
		const hide: IntentAction = { type: 'set_balance_privacy', hidden: true };
		expect(code(await h.intents.invoke('agent', hide))).toBe('completed');
		expect(h.performed).toEqual([]);
		// Showing them undoes the person's privacy: theirs to allow.
		const show: IntentAction = { type: 'set_balance_privacy', hidden: false };
		const asked = h.intents.invoke('agent', show);
		await tick();
		expect(h.intents.approvals[0]?.risk).toBe('manage');
		expect(h.performed).toEqual([]);
		h.intents.answer(h.intents.approvals[0].id, true);
		expect(code(await asked)).toBe('completed');
		expect(h.performed).toEqual([show]);
	});

	it('removes only a network the person added', async () => {
		const h = harness();
		h.world = { ...h.world, networks: networks([net(1, false), net(4242, true)]) };
		expect(code(await h.intents.invoke('person', { type: 'remove_network', chain_id: 1 }))).toBe(
			'refused'
		);
		expect(code(await h.intents.invoke('person', { type: 'remove_network', chain_id: 7 }))).toBe(
			'not_found'
		);
		expect(h.performed).toEqual([]);
	});

	it('says so when an RPC answers for another chain, and does not call it saved', async () => {
		const h = harness((action, now) => {
			if (action.type !== 'set_network_endpoint') return;
			const row = { ...net(4242, true, action.url), rpc_chain_mismatch: {} as never };
			h.world = { ...now, networks: networks([row]) };
		});
		h.world = { ...h.world, networks: networks([net(4242, true)]) };
		const outcome = await h.intents.invoke('person', {
			type: 'set_network_endpoint',
			chain_id: 4242,
			field: 'rpc',
			url: 'https://mainnet.example'
		});
		expect(outcome).toMatchObject({ status: 'rejected', reason: { code: 'refused' } });
	});

	it('has a network already there, and adds nothing', async () => {
		const h = harness();
		h.world = { ...h.world, networks: networks([net(8453, false)]) };
		const outcome = await h.intents.invoke('person', {
			type: 'add_network',
			chain_id: 8453,
			rpc_url: null
		});
		expect(code(outcome)).toBe('completed');
		expect(h.performed).toEqual([]);
	});

	it('deletes an activity record the wallet has, with the person’s approval', async () => {
		const h = harness((action, now) => {
			if (action.type === 'delete_activity')
				h.world = { ...now, feed: { transactions: [] } as unknown as FeedView };
		});
		h.world = {
			...h.world,
			feed: { transactions: [{ id: 'r1', user_op_hash: '' }] } as unknown as FeedView
		};
		expect(code(await h.intents.invoke('agent', { type: 'delete_activity', id: 'r9' }))).toBe(
			'not_found'
		);
		const pending = h.intents.invoke('agent', { type: 'delete_activity', id: 'r1' });
		await tick();
		expect(h.intents.approvals[0]?.risk).toBe('manage');
		h.intents.answer(h.intents.approvals[0].id, true);
		expect(code(await pending)).toBe('completed');
	});

	it('never shows the agent a provider’s key — only that one is set', () => {
		const state = agentState({
			world: { ...world(), networks: networks([net(1, false)]) },
			balance: null,
			feed: null,
			page: 'settings',
			approvals: [],
			activity: []
		});
		expect(JSON.stringify(state)).not.toContain('sk-secret');
		expect((state.networks as { providers: unknown }).providers).toEqual([
			{ provider: 'alchemy', has_key: true }
		]);
	});
});

describe('showing one thing', () => {
	it('names an asset or a record the wallet does not have, and goes nowhere', async () => {
		const h = harness();
		h.world = {
			...h.world,
			balance: {
				tokens: [{ chain_id: 1, token_address: null, symbol: 'ETH', spam: false }]
			} as unknown as BalanceView,
			feed: { transactions: [{ id: 'r1', user_op_hash: '' }] } as unknown as FeedView
		};
		const show = (screen: object) =>
			h.intents.invoke('agent', { type: 'show', screen } as IntentAction);
		expect(code(await show({ type: 'asset', chain_id: 1, token: null }))).toBe('completed');
		expect(code(await show({ type: 'asset', chain_id: 10, token: null }))).toBe('not_found');
		expect(code(await show({ type: 'activity', id: 'r1' }))).toBe('completed');
		expect(code(await show({ type: 'activity', id: 'r9' }))).toBe('not_found');
		expect(h.performed).toHaveLength(2);
	});
});

describe('leaving a transfer', () => {
	const op = (state: string, user_op_hash: string | null) =>
		({ id: 't1', state, chain_id: 1, user_op_hash, tx_hash: null, reason: null }) as never;

	it('leaves a review that failed before sending, and not one that went out', async () => {
		const h = harness((_, now) => {
			h.world = { ...now, operations: [op('cancelled', null)] };
		});
		h.world = { ...h.world, operations: [op('failed', null)] };
		const leave: IntentAction = { type: 'cancel_transfer', operation: 't1' };
		expect(code(await h.intents.invoke('agent', leave))).toBe('completed');
		h.world = { ...h.world, operations: [op('confirmed', '0xOP')] };
		expect(code(await h.intents.invoke('agent', leave))).toBe('refused');
	});
});

describe('a call the agent’s side gave up on (the bridge’s timeout, a lost connection)', () => {
	it('withdraws its approval, and a late “allow” does nothing', async () => {
		const h = harness();
		const gaveUp = new AbortController();
		const pending = h.intents.invoke('agent', deleteAlice, { signal: gaveUp.signal });
		await tick();
		const [asked] = h.intents.approvals;
		expect(asked).toBeDefined();
		gaveUp.abort();
		const outcome = await pending;
		expect(outcome).toMatchObject({ status: 'rejected', reason: { code: 'stopped' } });
		expect(h.intents.approvals, 'no longer asked').toEqual([]);
		h.intents.answer(asked.id, true);
		await tick();
		expect(h.performed).toEqual([]);
	});

	it('is not done when it is given up between the approval and the step', async () => {
		const h = harness();
		const gaveUp = new AbortController();
		const pending = h.intents.invoke('agent', deleteAlice, { signal: gaveUp.signal });
		await tick();
		h.intents.answer(h.intents.approvals[0].id, true);
		gaveUp.abort();
		expect(code(await pending)).toBe('stopped');
		expect(h.performed).toEqual([]);
	});

	it('does nothing for a call already given up', async () => {
		const h = harness();
		const gaveUp = new AbortController();
		gaveUp.abort();
		expect(code(await h.intents.invoke('agent', saveBob, { signal: gaveUp.signal }))).toBe(
			'stopped'
		);
		expect(h.performed).toEqual([]);
	});
});

describe('deleting a batch’s row', () => {
	it('names the batch by its operation’s hash, and holds once every record is gone', async () => {
		const records = [
			{ id: 'a', user_op_hash: '0xBATCH' },
			{ id: 'b', user_op_hash: '0xBATCH' }
		];
		const h = harness((action, now) => {
			if (action.type === 'delete_activity')
				h.world = { ...now, feed: { transactions: [] } as unknown as FeedView };
		});
		h.world = { ...h.world, feed: { transactions: records } as unknown as FeedView };
		expect(code(await h.intents.invoke('person', { type: 'delete_activity', id: '0xbatch' }))).toBe(
			'completed'
		);
	});
});
