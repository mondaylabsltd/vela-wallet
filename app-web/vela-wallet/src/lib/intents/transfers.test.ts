/**
 * Transfers through the intents (spec 103): a draft reaches the review by the
 * send core's own events, in the form's order, and the operation's state is
 * read from the send, the passkey's ceremony and the chain tracker — played
 * here by a desk whose "core" answers each event as the real one does.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendView } from '$lib/core/generated/SendView';

const ceremony: { listener: ((event: 'started' | 'signed' | 'failed') => void) | null } = {
	listener: null
};
const tracker = { entries: [] as Record<string, unknown>[] };
const held = {
	tokens: [
		{ chain_id: 5042002, symbol: 'USDC', token_address: null, spam: false, balance: '20' },
		{ chain_id: 1, symbol: 'SCAM', token_address: '0x' + 'f'.repeat(40), spam: true, balance: '9' }
	]
};

vi.mock('$app/navigation', () => ({ goto: vi.fn(async () => {}) }));
vi.mock('$app/paths', () => ({ resolve: (path: string) => path }));
vi.mock('$lib/wallet/core/balance.svelte', () => ({
	balance: { view: held, settledFor: () => true }
}));
vi.mock('$lib/session/core/session.svelte', () => ({ session: { view: { address: '0xme' } } }));
vi.mock('$lib/wallet/core/tracker-resident', () => ({ txTrackerView: () => tracker }));
vi.mock('$lib/services/networks', () => ({ networkId: (chain: number) => `net-${chain}` }));
vi.mock('$lib/onboarding/core/passkey', () => ({
	onSignCeremony: (listener: (event: 'started' | 'signed' | 'failed') => void) => {
		ceremony.listener = listener;
		return () => {};
	}
}));

const { sendDesk } = await import('$lib/flows/send-desk.svelte');
const { transfers } = await import('./transfers.svelte');

const BOB = '0x1111111111111111111111111111111111111111';

type Behaviour = {
	/** What `set_amount` / `tap_max` leaves on the form. */
	amountWarning?: { type: string } | null;
	/** What `continue` raises instead of reaching the review. */
	alertOnContinue?: { type: string };
	/** What the split form says of the recipients it was seeded with. */
	splitIssues?: { ordinal: number; address: string; amount: string }[];
};

/** A send the way the core answers the form's events. */
function desk(behaviour: Behaviour = {}) {
	let view: SendView | null = null;
	let alert: { type: string } | null = null;
	let session = 0;
	const events: SendEvent[] = [];
	const opened: unknown[] = [];
	const port = {
		view: () => view,
		alert: () => alert as never,
		session: () => session,
		open: (prefill: unknown) => {
			session = sendDesk.begin();
			opened.push(prefill);
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
				receipt: null
			} as unknown as SendView;
		},
		dispatch: (event: SendEvent) => {
			events.push(event);
			if (!view) return;
			if (event.type === 'set_amount' || event.type === 'tap_max') {
				const warning = behaviour.amountWarning ?? null;
				view = { ...view, amount_warning: warning, can_continue: warning === null } as SendView;
			}
			if (event.type === 'seed_split_recipients') {
				const issues = behaviour.splitIssues ?? [];
				view = {
					...view,
					recipients: event.recipients,
					split_row_issues: issues,
					split_over_balance: false,
					can_continue: issues.length === 0
				} as unknown as SendView;
			}
			if (event.type === 'continue') {
				if (behaviour.alertOnContinue) alert = behaviour.alertOnContinue;
				else view = { ...view, stage: 'confirm', can_confirm: true } as SendView;
			}
			if (event.type === 'slide_confirm') view = { ...view, tx_status: 'preparing' } as SendView;
		},
		close: () => {
			view = null;
			alert = null;
		}
	};
	const detach = sendDesk.attach(port);
	return {
		port,
		events,
		opened,
		detach,
		set: (patch: Partial<SendView>) => {
			view = { ...(view as SendView), ...patch } as SendView;
		}
	};
}

const draft = (amount = '1.5') => ({ chain_id: 5042002, token: null, to: BOB, amount });

beforeEach(() => {
	sendDesk.port = null;
	tracker.entries = [];
});

describe('a drafted transfer (spec 103)', () => {
	it('lands on the review by the form’s own events, in order', async () => {
		const d = desk();
		const result = await transfers.prepare(draft());
		expect(result).toMatchObject({ state: 'awaiting_confirmation', chain_id: 5042002 });
		expect(d.opened).toEqual([
			{ preselected_symbol: 'USDC', preselected_network: 'net-5042002', prefilled_recipient: BOB }
		]);
		expect(d.events.map((event) => event.type)).toEqual(['set_amount', 'continue']);
		expect(d.events[0]).toEqual({ type: 'set_amount', amount: '1.5' });
		d.detach();
	});

	it('takes Max with the form’s own Max', async () => {
		const d = desk();
		await transfers.prepare(draft('max'));
		expect(d.events[0]).toEqual({ type: 'tap_max' });
		d.detach();
	});

	it('leaves a send the person has open alone', async () => {
		const d = desk();
		d.port.open({});
		const result = await transfers.prepare(draft());
		expect(result).toMatchObject({ code: 'busy' });
		expect(d.opened).toHaveLength(1);
		d.detach();
	});

	it('refuses a token the wallet does not hold — spam included', async () => {
		const d = desk();
		expect(await transfers.prepare({ ...draft(), chain_id: 1 })).toMatchObject({
			code: 'not_found'
		});
		expect(
			await transfers.prepare({ ...draft(), chain_id: 1, token: '0x' + 'f'.repeat(40) })
		).toMatchObject({ code: 'not_found' });
		expect(d.opened).toEqual([]);
		d.detach();
	});

	it('says the amount’s warning, and leaves nothing open', async () => {
		const d = desk({ amountWarning: { type: 'not_enough_token' } });
		const result = await transfers.prepare(draft('999'));
		expect(result).toMatchObject({ code: 'refused', detail: 'amount: not_enough_token' });
		expect(d.port.view()).toBeNull();
		d.detach();
	});

	it('says the core’s refusal at Continue, and leaves nothing open', async () => {
		const d = desk({ alertOnContinue: { type: 'estimate_failed' } });
		const result = await transfers.prepare(draft());
		expect(result).toMatchObject({ code: 'refused', detail: 'alert: estimate_failed' });
		expect(d.port.view()).toBeNull();
		d.detach();
	});
});

describe('an operation, read from the machines that own it', () => {
	it('goes passkey → submitting → submitted → confirmed', async () => {
		const d = desk();
		const prepared = await transfers.prepare(draft());
		const id = (prepared as { id: string }).id;

		expect(transfers.submit(id)).toMatchObject({ state: 'awaiting_passkey' });
		expect(d.events.at(-1)).toEqual({ type: 'slide_confirm' });

		ceremony.listener?.('signed');
		expect(transfers.get(id)?.state).toBe('submitting');

		d.set({ stage: 'receipt', user_op_hash: '0xOP', receipt: { status: 'submitted' } } as never);
		expect(transfers.get(id)).toMatchObject({ state: 'submitted', user_op_hash: '0xOP' });

		// The person pressed Done: the send is gone, the chain still tells.
		d.port.close();
		tracker.entries = [{ user_op_hash: '0xop', status: 'confirmed', tx_hash: '0xTX' }];
		expect(transfers.get(id)).toMatchObject({ state: 'confirmed', tx_hash: '0xTX' });
		d.detach();
	});

	it('is awaiting the passkey while the core is “submitting” before it signs', async () => {
		const d = desk();
		const id = ((await transfers.prepare(draft())) as { id: string }).id;
		transfers.submit(id);
		// The core builds the operation (`submitting`) before it asks for the passkey.
		d.set({ tx_status: 'submitting' });
		expect(transfers.get(id)?.state).toBe('awaiting_passkey');
		ceremony.listener?.('signed');
		expect(transfers.get(id)?.state).toBe('submitting');
		d.detach();
	});

	it('is back on the review when the passkey prompt is dismissed', async () => {
		const d = desk();
		const id = ((await transfers.prepare(draft())) as { id: string }).id;
		transfers.submit(id);
		d.set({ tx_status: 'idle' });
		expect(transfers.get(id)?.state).toBe('awaiting_confirmation');
		d.detach();
	});

	it('is cancelled when its review is left unsent', async () => {
		const d = desk();
		const id = ((await transfers.prepare(draft())) as { id: string }).id;
		expect(transfers.cancel(id)).toBeNull();
		expect(transfers.get(id)).toMatchObject({ state: 'cancelled' });
		expect(transfers.submit(id)).toMatchObject({ code: 'not_found' });
		d.detach();
	});

	it('takes in a send the person filled in themselves, at its review', () => {
		const d = desk();
		d.port.open({});
		expect(transfers.current(), 'still on the form').toBeNull();
		d.set({ stage: 'confirm', can_confirm: true } as never);
		const adopted = transfers.current();
		expect(adopted).toMatchObject({ state: 'awaiting_confirmation' });
		expect(transfers.current()?.id, 'once, not again').toBe(adopted?.id);
		d.detach();
	});
});

describe('one token to several (spec 103)', () => {
	const CAROL = '0x2222222222222222222222222222222222222222';
	const batch = {
		chain_id: 5042002,
		token: null,
		recipients: [
			{ to: BOB, amount: '1' },
			{ to: CAROL, amount: '.5' }
		]
	};

	it('seeds the split form with every recipient, then reaches the review', async () => {
		const d = desk();
		const result = await transfers.prepareBatch(batch);
		expect(result).toMatchObject({ state: 'awaiting_confirmation' });
		expect(d.opened, 'no single recipient prefilled').toEqual([
			{ preselected_symbol: 'USDC', preselected_network: 'net-5042002' }
		]);
		expect(d.events.map((event) => event.type)).toEqual(['seed_split_recipients', 'continue']);
		expect(d.events[0]).toEqual({
			type: 'seed_split_recipients',
			recipients: [
				{ id: '', address: BOB, amount: '1', name: null },
				{ id: '', address: CAROL, amount: '0.5', name: null }
			]
		});
		d.detach();
	});

	it('says which recipient the form refused, and leaves nothing open', async () => {
		const d = desk({ splitIssues: [{ ordinal: 2, address: 'ok', amount: 'invalid' }] });
		const result = await transfers.prepareBatch(batch);
		expect(result).toMatchObject({
			code: 'refused',
			detail: 'recipient 2: address ok, amount invalid'
		});
		expect(d.port.view()).toBeNull();
		d.detach();
	});
});

describe('one transfer after another (spec 103)', () => {
	it('a sent transfer’s receipt gives way to the next draft, by its own Done', async () => {
		const d = desk();
		const first = (await transfers.prepare(draft())) as { id: string };
		transfers.submit(first.id);
		ceremony.listener?.('signed');
		d.set({ stage: 'receipt', user_op_hash: '0xOP1', receipt: { status: 'confirmed' } } as never);
		expect(transfers.get(first.id)?.state).toBe('confirmed');

		const second = await transfers.prepare(draft('2'));
		expect(second).toMatchObject({ state: 'awaiting_confirmation' });
		const types = d.events.map((event) => event.type);
		expect(types.slice(types.indexOf('slide_confirm') + 1)).toEqual([
			'done',
			'set_amount',
			'continue'
		]);
		expect(d.opened, 'a new send for the second').toHaveLength(2);
		expect(transfers.get(first.id)?.state, 'the first stays as it ended').toBe('confirmed');
		d.detach();
	});

	it('a receipt still waiting on the chain is followed there once it closes', async () => {
		const d = desk();
		const first = (await transfers.prepare(draft())) as { id: string };
		transfers.submit(first.id);
		d.set({ stage: 'receipt', user_op_hash: '0xOP2', receipt: { status: 'submitted' } } as never);
		await transfers.prepare(draft('2'));
		expect(transfers.get(first.id)).toMatchObject({ state: 'submitted', user_op_hash: '0xOP2' });
		tracker.entries = [{ user_op_hash: '0xop2', status: 'confirmed', tx_hash: '0xTX2' }];
		expect(transfers.get(first.id)).toMatchObject({ state: 'confirmed', tx_hash: '0xTX2' });
		d.detach();
	});

	it('a review still waiting is busy, and names the transfer to submit or cancel', async () => {
		const d = desk();
		const first = (await transfers.prepare(draft())) as { id: string };
		const second = await transfers.prepare(draft('2'));
		expect(second).toMatchObject({ code: 'busy' });
		expect((second as { detail: string }).detail).toContain(first.id);
		d.detach();
	});

	it('a review that failed before sending can be left, and the next one drafted', async () => {
		const d = desk();
		const first = (await transfers.prepare(draft())) as { id: string };
		transfers.submit(first.id);
		d.set({ tx_status: 'error', tx_error: 'estimate_failed' } as never);
		expect(transfers.get(first.id)?.state).toBe('failed');
		expect(transfers.cancel(first.id)).toBeNull();
		expect(transfers.get(first.id)?.state).toBe('cancelled');
		expect(await transfers.prepare(draft('2'))).toMatchObject({
			state: 'awaiting_confirmation'
		});
		d.detach();
	});
});

describe('one network, one nonce', () => {
	it('says the confirm waits for the previous transfer, and names it', async () => {
		const d = desk();
		const id = ((await transfers.prepare(draft())) as { id: string }).id;
		d.set({
			can_confirm: false,
			previous_pending: { chain_id: 5042002, user_op_hash: '0xOP0', key: 'k' }
		} as never);
		const result = transfers.submit(id, 'agent');
		expect(result).toMatchObject({ code: 'not_ready' });
		expect((result as { detail: string }).detail).toContain('0xOP0');
		expect(d.events.some((event) => event.type === 'slide_confirm')).toBe(false);
		d.detach();
	});
});
