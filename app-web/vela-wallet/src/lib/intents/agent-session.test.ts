/**
 * An agent sending in batches, start to finish (spec 103): the real intents —
 * the core's check (shipped wasm), the shell's rules, `perform`, `transfers` —
 * over a send form that answers each event as the send core does. What it
 * pins is the session the owner's agent ran into: one batch after another, a
 * batch still on its way, the passkey prompt dismissed, confirm pressed twice.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync, intentCheck } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import type { IntentAction } from '$lib/core/generated/IntentAction';
import type { IntentCheck } from '$lib/core/generated/IntentCheck';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendView } from '$lib/core/generated/SendView';
import type { SessionView } from '$lib/session/generated/SessionView';

const ceremony: { listener: ((event: 'started' | 'signed' | 'failed') => void) | null } = {
	listener: null
};
const tracker = { entries: [] as Record<string, unknown>[] };
const MINE = '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

vi.mock('$app/navigation', () => ({ goto: vi.fn(async () => {}) }));
vi.mock('$app/paths', () => ({ resolve: (path: string) => path }));
vi.mock('$lib/wallet/core/balance.svelte', () => ({
	balance: {
		view: {
			hidden: false,
			tokens: [
				{ chain_id: 5042002, symbol: 'USDC', token_address: null, spam: false, balance: '20' }
			]
		},
		settledFor: () => true
	}
}));
vi.mock('$lib/session/core/session.svelte', () => ({ session: { view: { address: MINE } } }));
vi.mock('$lib/wallet/core/tracker-resident', () => ({ txTrackerView: () => tracker }));
vi.mock('$lib/services/networks', () => ({
	networkId: (chain: number) => `net-${chain}`,
	getAllNetworksSync: () => []
}));
vi.mock('$lib/onboarding/core/passkey', () => ({
	onSignCeremony: (listener: (event: 'started' | 'signed' | 'failed') => void) => {
		ceremony.listener = listener;
		return () => {};
	}
}));

const { sendDesk } = await import('$lib/flows/send-desk.svelte');
const { transfers } = await import('./transfers.svelte');
const { perform } = await import('./perform');
const { Intents } = await import('./intents.svelte');

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

/** The send form, answering as the send core does. */
function form() {
	let view: SendView | null = null;
	let session = 0;
	const events: SendEvent[] = [];
	const detach = sendDesk.attach({
		view: () => view,
		alert: () => null,
		session: () => session,
		open: () => {
			session = sendDesk.begin();
			view = {
				stage: 'enter_details',
				loading: false,
				selected_token: { chain_id: 5042002 },
				can_continue: false,
				can_confirm: false,
				amount_warning: null,
				tx_status: 'idle',
				tx_error: null,
				user_op_hash: null,
				tx_hash: null,
				receipt: null,
				recipients: [],
				split_row_issues: [],
				split_over_balance: false
			} as unknown as SendView;
		},
		dispatch: (event) => {
			events.push(event);
			if (!view) return;
			if (event.type === 'seed_split_recipients' || event.type === 'set_amount')
				view = {
					...view,
					...(event.type === 'seed_split_recipients' && { recipients: event.recipients }),
					can_continue: true
				} as SendView;
			if (event.type === 'continue') view = { ...view, stage: 'confirm', can_confirm: true };
			// The core's own gate: confirm only from the review, once.
			if (event.type === 'slide_confirm' && view.stage === 'confirm' && view.can_confirm)
				view = { ...view, tx_status: 'preparing', can_confirm: false };
		},
		close: () => (view = null)
	});
	return {
		events,
		detach,
		/** The passkey answered: signed and handed to the relay, at its receipt. */
		signed(hash: string) {
			ceremony.listener?.('signed');
			view = {
				...(view as SendView),
				stage: 'receipt',
				tx_status: 'confirmed',
				user_op_hash: hash,
				receipt: { status: 'submitted' }
			} as SendView;
		},
		/** The person dismissed the passkey prompt: back on the review. */
		dismissed() {
			ceremony.listener?.('failed');
			view = { ...(view as SendView), tx_status: 'idle', can_confirm: true };
		}
	};
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

function wallet(grant: boolean) {
	return new Intents({
		ready: async () => {},
		check: (actor, action, seen) =>
			JSON.parse(
				intentCheck(
					actor,
					JSON.stringify(action),
					JSON.stringify({
						agent_enabled: seen.agent.enabled,
						submit_transactions: seen.agent.submitTransactions
					})
				)
			) as IntentCheck,
		world: () => ({
			session: session(),
			contacts: null,
			currency: null,
			feeTier: null,
			preferences: {} as never,
			agent: { enabled: true, submitTransactions: grant },
			operations: transfers.list(),
			balance: null,
			feed: null,
			networks: null
		}),
		perform,
		now: () => Date.now(),
		approvalMs: 300,
		observeMs: 300,
		pollMs: 5
	});
}

const recipients = (count: number) =>
	Array.from({ length: count }, (_, i) => ({
		to: `0x${(i + 1).toString(16).padStart(40, '0')}`,
		amount: '0.01'
	}));
const batch = (count: number): IntentAction => ({
	type: 'prepare_batch_transfer',
	chain_id: 5042002,
	token: null,
	recipients: recipients(count)
});
const submit = (operation: string): IntentAction => ({ type: 'submit_transfer', operation });

beforeEach(() => {
	sendDesk.port = null;
	tracker.entries = [];
});

describe('an agent sending in batches (spec 103)', () => {
	it('sends batch after batch, the first still on its way when the second is drafted', async () => {
		const f = form();
		const intents = wallet(true);

		const first = await intents.invoke('agent', batch(60));
		expect(first).toMatchObject({
			status: 'accepted',
			operation: { state: 'awaiting_confirmation' }
		});
		const one = (first as { operation: { id: string } }).operation.id;
		expect(await intents.invoke('agent', submit(one))).toMatchObject({
			status: 'accepted',
			operation: { state: 'awaiting_passkey' }
		});
		f.signed('0xOP1');
		expect(transfers.get(one)?.state).toBe('submitted');

		// Batch 1 is out but not yet confirmed; its receipt is showing.
		const second = await intents.invoke('agent', batch(40));
		expect(second).toMatchObject({ status: 'accepted', closed: one });
		const two = (second as { operation: { id: string } }).operation.id;
		expect(f.events.filter((event) => event.type === 'done')).toHaveLength(1);

		// Batch 1 is still followed — on the chain now.
		tracker.entries = [{ user_op_hash: '0xop1', status: 'confirmed', tx_hash: '0xTX1' }];
		expect(transfers.get(one)).toMatchObject({ state: 'confirmed', tx_hash: '0xTX1' });

		expect(await intents.invoke('agent', submit(two))).toMatchObject({ status: 'accepted' });
		f.signed('0xOP2');
		expect(transfers.get(two)?.state).toBe('submitted');
		f.detach();
	});

	it('does not raise the passkey again once the person dismissed it — the person still can', async () => {
		const f = form();
		const intents = wallet(true);
		const drafted = await intents.invoke('agent', batch(3));
		const id = (drafted as { operation: { id: string } }).operation.id;
		await intents.invoke('agent', submit(id));
		f.dismissed();
		expect(transfers.get(id)).toMatchObject({
			state: 'awaiting_confirmation',
			reason: 'the person dismissed the passkey prompt'
		});

		expect(await intents.invoke('agent', submit(id))).toMatchObject({
			status: 'rejected',
			reason: { code: 'denied' }
		});
		expect(f.events.filter((event) => event.type === 'slide_confirm')).toHaveLength(1);

		expect(await intents.invoke('person', submit(id))).toMatchObject({
			status: 'accepted',
			operation: { state: 'awaiting_passkey', reason: null }
		});
		expect(f.events.filter((event) => event.type === 'slide_confirm')).toHaveLength(2);
		f.detach();
	});

	it('presses confirm once when asked twice at the same moment', async () => {
		const f = form();
		const intents = wallet(true);
		const drafted = await intents.invoke('agent', batch(2));
		const id = (drafted as { operation: { id: string } }).operation.id;
		const [a, b] = await Promise.all([
			intents.invoke('agent', submit(id)),
			intents.invoke('agent', submit(id))
		]);
		expect([a.status, b.status].sort()).toEqual(['accepted', 'rejected']);
		expect(f.events.filter((event) => event.type === 'slide_confirm')).toHaveLength(1);
		f.detach();
	});

	it('cannot redraft over a batch on its review, nor confirm without the grant', async () => {
		const f = form();
		const intents = wallet(false);
		const drafted = await intents.invoke('agent', batch(2));
		const id = (drafted as { operation: { id: string } }).operation.id;
		expect(await intents.invoke('agent', batch(5))).toMatchObject({
			status: 'rejected',
			reason: { code: 'busy' }
		});
		expect(await intents.invoke('agent', submit(id))).toMatchObject({
			status: 'rejected',
			reason: { code: 'not_granted' }
		});
		expect(f.events.filter((event) => event.type === 'slide_confirm')).toHaveLength(0);
		f.detach();
	});
});
