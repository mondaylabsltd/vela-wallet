/**
 * The send overlays (spec 026 T236): the drawn screens filled from `SendView`
 * and `FeeView`, and nothing else. Every assertion here is "the core said so".
 */
import { describe, expect, it } from 'vitest';
import type { FeeSpeedEvent } from '$lib/core/generated/FeeSpeedEvent';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeView } from '$lib/core/generated/FeeView';
import { FeeSpeedCore } from '$lib/core/client';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import type { WalletIdentity } from '$lib/wallet/identity';
import { shortenAddress } from '$lib/wallet/identity';
import { fill } from '$lib/wallet/messages';
import { buildFlowState } from './fixtures';
import {
	liveFeeTokenPick,
	liveSendConfirm,
	alertWords,
	liveSendForm,
	liveSendPick,
	liveSendReceipt,
	sendTokenClass,
	sendTokenId,
	visibleSendTokens,
	type SendLiveInputs
} from './live-send';

const m = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const identity: WalletIdentity = {
	name: 'My Wallet',
	address: '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c',
	identiconSvg: '<svg/>'
};
const USD = { code: 'USD', rate: 1, committed: true };

const USDT: SendToken = {
	network: 'eth-mainnet',
	chain_id: 1,
	symbol: 'USDT',
	balance: '53.4836',
	decimals: 6,
	token_address: '0x' + 'dd'.repeat(20),
	price_usd: 1,
	logo_urls: [],
	spam: false
};

const ETH: SendToken = {
	...USDT,
	symbol: 'ETH',
	token_address: null,
	balance: '1.5',
	decimals: 18,
	price_usd: 3000
};

const EMPTY_SEND: SendView = {
	stage: 'enter_details',
	loading: false,
	locked: false,
	amount_locked: false,
	lock_error: null,
	resolving_lock: false,
	adding_network: false,
	add_network_msg: null,
	tokens: [],
	selected_token: null,
	recipient: '',
	amount: '',
	amount_fiat_code: null,
	denom_toggle_shown: false,
	denom_toggle_enabled: false,
	denom_toggle_reason: null,
	confirm_amount_issue: null,
	token_amount: '',
	confirm_amount: '',
	split_mode: false,
	recipients: [],
	split_over_balance: false,
	split_duplicates: [],
	split_row_issues: [],
	split_remaining: null,
	split_import_room: 60,
	picker_target: null,
	multi_select_mode: false,
	multi_selected_ids: [],
	multi_valuable_ids: [],
	multi_chain_id: null,
	multi_specs: [],
	show_scanner: false,
	show_contact_picker: false,
	show_batch_import: false,
	estimating_gas: false,
	fee_busy: false,
	fee: null,
	gas_fee_token: null,
	amount_warning: null,
	same_asset_fee_issue: null,
	can_continue: false,
	can_confirm: false,
	sending: false,
	tx_status: 'idle',
	tx_error: null,
	tx_hash: null,
	user_op_hash: null,
	receipt: null,
	treasury_bootstrap: null,
	recipient_identity: null,
	recipient_risk: null,
	sim_json: null
};

const IDLE_FEE: FeeView = {
	busy: false,
	failed: null,
	fee: null,
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: false
};

const QUOTE = {
	chain_id: 1,
	total_wei: '2100000000000000',
	max_fee_per_gas: '1',
	network_fee_per_gas: '1',
	relayer_fee_per_gas: '0',
	bundler_gas_price: '1',
	in_band_gas_basis: '1',
	effective_gas_price: null,
	max_gas_price: null,
	total_gas: '1',
	deployed: true,
	tier: 'fast' as const,
	quoted: true,
	fee_asset: { type: 'native' as const },
	fee_recipient: null
};

function inputs(send: Partial<SendView>, fee: Partial<FeeView> = {}): SendLiveInputs {
	return {
		send: { ...EMPTY_SEND, ...send },
		fee: { ...IDLE_FEE, ...fee },
		m,
		currency: USD,
		identity,
		identicon
	};
}

const pickModel = () => {
	const state = buildFlowState('sd1', m, identicon);
	if (state.base.kind !== 'send-pick') throw new Error('kind');
	return state.base.model;
};
const formModel = () => {
	const state = buildFlowState('sd2', m, identicon);
	if (state.base.kind !== 'send-form') throw new Error('kind');
	return state.base.model;
};
const confirmModel = () => {
	const state = buildFlowState('sd3', m, identicon);
	if (state.base.kind !== 'send-confirm') throw new Error('kind');
	return state.base.model;
};
const receiptModel = () => {
	const state = buildFlowState('sd4a', m, identicon);
	if (state.base.kind !== 'send-receipt') throw new Error('kind');
	return state.base.model;
};
const feeSheetModel = () => {
	const state = buildFlowState('sd2f', m, identicon);
	if (state.sheet?.kind !== 'fee-token') throw new Error('kind');
	return state.sheet.model;
};

describe('the token picker', () => {
	it("shows the core's holdings, priced at the committed currency", () => {
		const model = liveSendPick(pickModel(), inputs({ tokens: [USDT, ETH] }));
		expect(model.rows.map((r) => r.ticker)).toEqual(['USDT', 'ETH']);
		expect(model.rows[1]).toMatchObject({ balance: '1.5', fiat: { text: '$4,500.00' } });
		expect(JSON.stringify(model)).not.toContain('0.8533');
	});

	it('says why the list is empty — nothing held, or nothing matching', () => {
		// Rows exist but a filter hid them: "nothing matches", not "nothing held".
		const filtered = liveSendPick(pickModel(), {
			...inputs({ tokens: [USDT, ETH] }),
			chainFilter: 137
		});
		expect(filtered.rows).toHaveLength(0);
		expect(filtered.empty).toBe(m['send.noMatchingTokens']);

		const nothing = liveSendPick(pickModel(), inputs({ tokens: [] }));
		expect(nothing.rows).toHaveLength(0);
		expect(nothing.empty).toBe(m['send.noTokensWithBalance']);
	});

	it('an unpriced token says so rather than showing a zero', () => {
		const model = liveSendPick(pickModel(), inputs({ tokens: [{ ...ETH, price_usd: null }] }));
		expect(model.rows[0].fiat).toEqual({ kind: 'no-price', text: '—' });
	});
});

describe('the form', () => {
	it('carries the chosen token, the typed amount and its fiat value', () => {
		const model = liveSendForm(
			formModel(),
			inputs({ selected_token: ETH, amount: '0.5', token_amount: '0.5' })
		);
		expect(model.header.title).toContain('ETH');
		expect(model.token).toMatchObject({ symbol: 'ETH' });
		expect(model.token?.detail).toContain('1.5');
		expect(model.amount).toMatchObject({ value: '0.5', fiat: '≈ $1,500.00' });
	});

	/**
	 * Issue 209: the address book opened a send on an account holding
	 * nothing, and the drawn SD2 card — "USDT · Ethereum · Balance 53.4836" —
	 * stood in for the token the core never named. A wallet at $0.00 with an
	 * empty Assets list quoted a balance it does not have.
	 */
	it('shows no token card and names no token when the core has not chosen one', () => {
		const model = liveSendForm(formModel(), inputs({ recipient: '0x' + 'ab'.repeat(20) }));
		expect(model.token).toBeUndefined();
		expect(model.header.title).not.toContain('USDT');
		expect(JSON.stringify(model)).not.toContain('53.4836');
	});

	it('splits the recipient across the drawn two lines; empty stays empty', () => {
		const address = '0x' + 'ab'.repeat(20);
		const filled = liveSendForm(formModel(), inputs({ recipient: address }));
		expect(filled.recipient?.lines.join('')).toBe(address);
		const blank = liveSendForm(formModel(), inputs({ recipient: '' }));
		expect(blank.recipient?.lines).toEqual(['', '']);
		expect(blank.recipient?.identiconSvg).toBe('');
	});

	it('says who the recipient is when the core resolved a name, and warns on a first send', () => {
		const named = liveSendForm(
			formModel(),
			inputs({ recipient_identity: { name: 'alice.eth', source: 'ENS' } })
		);
		expect(named.recipient?.note).toBe('alice.eth · ENS');
		const firstTime = liveSendForm(
			formModel(),
			inputs({ recipient_risk: { is_contract: false, first_time: true } })
		);
		expect(firstTime.recipient?.note).toBe(m['componentsUi.signing.firstTimeTag']);
		expect(liveSendForm(formModel(), inputs({})).recipient?.note).toBeUndefined();
	});

	it('the fee row waits rather than inventing a number, then reads the quote', () => {
		const waiting = liveSendForm(formModel(), inputs({ fee_busy: true, selected_token: ETH }));
		expect(waiting.fee.value).toBe('…');
		const quoted = liveSendForm(formModel(), inputs({ selected_token: ETH, fee: QUOTE }));
		expect(quoted.fee.value).toBe('0.0021 ETH');
		expect(quoted.fee.valueFiat).toBe('≈ $6.30');
		const idle = liveSendForm(formModel(), inputs({ selected_token: ETH }));
		expect(idle.fee.value).toBe('—');
	});

	// Issue 210: `Max` on a balance the fee outruns fills `0`, and this shell
	// rendered that zero with nothing beside it — the core's sentence was
	// computed and dropped. It rides 211's live-warning line.
	it('says why the amount is nothing when the fee outruns the balance', () => {
		const model = liveSendForm(
			formModel(),
			inputs({
				selected_token: { ...ETH, balance: '0.00005' },
				amount: '0',
				token_amount: '0',
				fee: QUOTE,
				amount_warning: { type: 'insufficient_gas', symbol: 'ETH' }
			})
		);
		expect(model.alert).toBe('Insufficient ETH for gas fees');
	});

	it('words every other reading of the money the core hands it', () => {
		const warn = (amount_warning: SendView['amount_warning']) =>
			liveSendForm(formModel(), inputs({ selected_token: ETH, amount_warning })).alert;
		expect(warn({ type: 'not_enough_token', symbol: 'ETH' })).toBe(
			'You do not have enough ETH in this account'
		);
		expect(warn({ type: 'insufficient_for_gas', symbol: 'ETH' })).toBe(
			'Insufficient ETH to cover amount + gas fees'
		);
		expect(warn({ type: 'need_gas', symbol: 'USDT' })).toBe('You need USDT to pay gas fees');
		expect(warn({ type: 'cannot_convert', code: 'EUR', symbol: 'ETH' })).toContain('EUR');
		// A `null` symbol is the chain's own coin: the core leaves it to the
		// shell's registry rather than printing a sentence with a hole in it.
		expect(warn({ type: 'insufficient_gas', symbol: null })).toBe('Insufficient ETH for gas fees');
		expect(warn(null)).toBeUndefined();
	});

	// Issue 197: the ⇄ under the figure was drawn on every form and wired to
	// nothing — `denom_toggle_shown/enabled/reason` and the one event that
	// moves them all went unread, so the tap was swallowed in silence.
	describe('the ⇄ denomination toggle', () => {
		it('is offered exactly where the core offers it, and live where it can act', () => {
			const off = liveSendForm(formModel(), inputs({ selected_token: ETH }));
			expect(off.amount?.denomToggle).toBeUndefined();

			const live = liveSendForm(
				formModel(),
				inputs({ selected_token: ETH, denom_toggle_shown: true, denom_toggle_enabled: true })
			);
			expect(live.amount?.denomToggle).toEqual({ enabled: true });

			const dimmed = liveSendForm(
				formModel(),
				inputs({ selected_token: ETH, denom_toggle_shown: true, denom_toggle_enabled: false })
			);
			expect(dimmed.amount?.denomToggle).toEqual({ enabled: false });
		});

		it('says why it is inert rather than just dimming', () => {
			const model = liveSendForm(
				formModel(),
				inputs({
					selected_token: ETH,
					denom_toggle_shown: true,
					denom_toggle_enabled: false,
					denom_toggle_reason: { code: 'CNY', symbol: 'ETH' }
				})
			);
			expect(model.alert).toBe('No CNY rate right now — enter the amount in ETH.');
		});

		it('yields to the money warnings, which are about the figure itself', () => {
			const model = liveSendForm(
				formModel(),
				inputs({
					selected_token: ETH,
					denom_toggle_shown: true,
					denom_toggle_enabled: false,
					denom_toggle_reason: { code: 'CNY', symbol: 'ETH' },
					amount_warning: { type: 'not_enough_token', symbol: 'ETH' }
				})
			);
			expect(model.alert).toBe('You do not have enough ETH in this account');
		});

		it('shows the OTHER denomination, so a swap is visible in the line beneath', () => {
			// Typing tokens: the money. (The default, asserted above too.)
			const inToken = liveSendForm(
				formModel(),
				inputs({ selected_token: ETH, amount: '0.5', token_amount: '0.5' })
			);
			expect(inToken.amount).toMatchObject({
				value: '0.5',
				fiat: '≈ $1,500.00',
				denomLabel: 'ETH'
			});

			// Typing money: the tokens it buys — the core's own resolved figure,
			// not a second conversion. Restating the fiat here made a working
			// swap read as a dead one.
			const inFiat = liveSendForm(
				formModel(),
				inputs({
					selected_token: ETH,
					amount: '1500',
					amount_fiat_code: 'USD',
					token_amount: '0.5'
				})
			);
			expect(inFiat.amount).toMatchObject({
				value: '1500',
				fiat: '≈ 0.5 ETH',
				denomLabel: 'USD'
			});
		});

		it('never invents a token line for a send with no token chosen', () => {
			const model = liveSendForm(
				formModel(),
				inputs({ amount: '1500', amount_fiat_code: 'USD', token_amount: '' })
			);
			expect(model.amount?.fiat).toBe('');
		});
	});

	/**
	 * Issue 231: the hero read "4.00" with no unit. The unit was known all
	 * along — it reached the screen reader and nothing else.
	 */
	describe('the unit on the figure', () => {
		const typedIn = (code: string | null, currency = USD) =>
			liveSendForm(formModel(), {
				...inputs({ selected_token: ETH, amount: '4.00', amount_fiat_code: code }),
				currency
			}).amount;

		it('leads a fiat figure with the symbol the line beneath would use', () => {
			expect(typedIn('USD')?.adornment).toEqual({ prefix: '$' });
			expect(typedIn('EUR')?.adornment).toEqual({ prefix: '€' });
		});

		it('follows the figure with the code when the catalog has no symbol for it', () => {
			// Never a borrowed "$": an unknown currency is named, not guessed.
			expect(typedIn('PLN')?.adornment).toEqual({ suffix: 'PLN' });
		});

		it('follows a token figure with the token', () => {
			expect(typedIn(null)?.adornment).toEqual({ suffix: 'ETH' });
		});

		it("is the FIGURE's currency, not the display currency that moved under it", () => {
			// Typed in yuan; the display currency has since become euros. "€"
			// over these digits would be a relabel — the same number, a new unit.
			const figure = typedIn('CNY', { code: 'EUR', rate: 0.9, committed: true });
			expect(figure?.adornment).toEqual({ prefix: '¥' });
			expect(figure?.denomLabel).toBe('CNY');
		});

		it('claims no unit at all when there is none to name', () => {
			const model = liveSendForm(formModel(), inputs({ selected_token: null }));
			expect(model.amount?.adornment).toEqual({});
		});

		it('leaves the empty field empty, with a placeholder to show for it', () => {
			// It used to hand the input a real "0", so typing 4 made "04".
			const model = liveSendForm(formModel(), inputs({ selected_token: ETH, amount: '' }));
			expect(model.amount?.value).toBe('');
			expect(model.amount?.placeholder).toBe('0');
		});
	});

	it('is always the single-send shape in this phase (split and sweep are 026 batch)', () => {
		const model = liveSendForm(formModel(), inputs({ selected_token: ETH }));
		expect(model.mode).toBe('single');
		expect(model.recipients).toBeUndefined();
		expect(model.summary).toBeUndefined();
	});
});

describe('the confirm screen', () => {
	it('states the amount, the parties and the fee the core settled on', () => {
		const model = liveSendConfirm(
			confirmModel(),
			inputs({
				selected_token: ETH,
				confirm_amount: '0.5',
				recipient: '0x' + 'ab'.repeat(20),
				fee: QUOTE
			})
		);
		expect(model.amount).toBe('0.5 ETH');
		expect(model.subline).toBe('≈ $1,500.00');
		const byLabel = new Map(model.facts.map((f) => [f.label, f.value]));
		expect(byLabel.get(m['send.fromLabel'])).toBe('My Wallet');
		expect(byLabel.get(m['send.toLabel'])).toMatch(/^0xabab/);
		expect(byLabel.get(m['send.estFeeLabel'])).toBe('0.0021 ETH · ≈$6.30');
		expect(model.breakdown).toBeUndefined();
	});

	it('names a resolved recipient instead of their address', () => {
		const model = liveSendConfirm(
			confirmModel(),
			inputs({
				recipient: '0x' + 'ab'.repeat(20),
				recipient_identity: { name: 'alice.eth', source: 'ENS' }
			})
		);
		expect(model.facts.find((f) => f.label === m['send.toLabel'])?.value).toBe('alice.eth');
	});
});

describe('the core’s refusals reach the screen (spec 038 #D4)', () => {
	const alice = '0x' + 'ab'.repeat(20);
	it('a failed estimate is a sentence on the form, and on the confirm', () => {
		const form = liveSendForm(formModel(), {
			...inputs({ selected_token: ETH, recipient: alice, amount: '1' }),
			alert: { type: 'estimate_failed', kind: 'generic' as never }
		});
		expect(form.alert).toContain(m['send.alertEstimateFailedTitle']);
		expect(form.alert).toContain(m['send.alertEstimateFailedBody']);
		const confirm = liveSendConfirm(confirmModel(), {
			...inputs({ selected_token: ETH, recipient: alice, confirm_amount: '1' }),
			alert: { type: 'invalid_amount' }
		});
		expect(confirm.alert).toContain(m['send.alertInvalidAmountTitle']);
	});

	it('no refusal, no line', () => {
		const form = liveSendForm(formModel(), inputs({ selected_token: ETH }));
		expect(form.alert).toBeUndefined();
	});

	it('a split’s total is the core’s sum, not the single field', () => {
		const form = liveSendForm(
			formModel(),
			inputs({
				selected_token: ETH,
				split_mode: true,
				confirm_amount: '0.06',
				token_amount: '',
				recipients: [
					{ id: 'a', address: alice, amount: '0.03', name: null },
					{ id: 'b', address: '0x' + 'cd'.repeat(20), amount: '0.03', name: null }
				]
			})
		);
		expect(form.summary?.value).toBe('0.06 ETH');
	});

	// Issues 204-206, the multi-recipient send. Each of these is something the
	// split form did without a word; each test pins the word.
	describe('a split says what it is doing', () => {
		const bob = '0x' + 'cd'.repeat(20);
		const split = (over: Partial<SendView>) =>
			liveSendForm(
				formModel(),
				inputs({
					selected_token: { ...ETH, price_usd: 3000 },
					split_mode: true,
					can_continue: true,
					confirm_amount: '0.75',
					recipients: [
						{ id: 'a', address: alice, amount: '0.5', name: 'Alice' },
						{ id: 'b', address: bob, amount: '0.25', name: null }
					],
					...over
				})
			);

		it('counts the people in the total and prices the sum', () => {
			const form = split({});
			expect(form.summary?.label).toBe('Total · 2 recipients');
			expect(form.summary?.value).toBe('0.75 ETH');
			expect(form.summary?.detail).toBe('≈ $2,250.00');
			expect(form.summary?.over).toBe(false);
		});

		it('prices nothing while there is nothing to price', () => {
			const form = split({ confirm_amount: '0' });
			expect(form.summary?.detail).toBeUndefined();
			// A row that cannot be summed leaves the core's total empty — a dash,
			// never a bare symbol.
			expect(split({ confirm_amount: '' }).summary?.value).toBe('—');
		});

		it('an imported row keeps its name — over the address it pays, never instead of it', () => {
			const [named, bare] = split({}).recipients ?? [];
			expect(named.name).toBe('Alice');
			expect(named.addressShort).toBe(shortenAddress(alice));
			expect(bare.name).toBe(shortenAddress(bob));
			expect(bare.addressShort).toBeUndefined();
		});

		it('a total over the balance says so BEFORE Continue is pressed', () => {
			const form = split({ split_over_balance: true });
			expect(form.summary?.over).toBe(true);
			expect(form.alert).toBe(m['send.alertInsufficientBalanceBody']);
		});

		it('a dark Continue names the recipient it is waiting on, and what for', () => {
			const waiting = split({
				can_continue: false,
				split_row_issues: [
					{ id: 'b', ordinal: 2, address: 'ok', amount: 'empty' },
					{ id: 'c', ordinal: 3, address: 'empty', amount: 'empty' }
				]
			});
			expect(waiting.hint).toBe('Recipient 2 needs an amount.');
			expect(waiting.alert).toBeUndefined();
			expect(
				split({
					can_continue: false,
					split_row_issues: [{ id: 'a', ordinal: 1, address: 'invalid', amount: 'ok' }]
				}).hint
			).toBe('Recipient 1 needs an address.');
			// While the pre-check is out the gate is closed for another reason,
			// and the button says that one itself.
			expect(
				split({
					can_continue: false,
					estimating_gas: true,
					split_row_issues: [{ id: 'b', ordinal: 2, address: 'ok', amount: 'empty' }]
				}).hint
			).toBeUndefined();
			expect(split({}).hint).toBeUndefined();
		});

		it('only a field with something IN it is called wrong', () => {
			const [first, second] =
				split({
					split_row_issues: [
						{ id: 'a', ordinal: 1, address: 'invalid', amount: 'invalid' },
						{ id: 'b', ordinal: 2, address: 'empty', amount: 'empty' }
					]
				}).recipients ?? [];
			expect(first.addressNote).toBe('Invalid address');
			expect(first.amountNote).toBe('Not a valid amount');
			// Unfinished is not mistaken: the prompt already says what it wants.
			expect(second.addressNote).toBeUndefined();
			expect(second.amountNote).toBeUndefined();
		});

		it('says what is left to give out — the core’s subtraction, not this file’s', () => {
			expect(split({ split_remaining: '0.75' }).summary?.remaining).toBe('0.75 ETH left');
			expect(split({ split_remaining: null }).summary?.remaining).toBeUndefined();
		});

		it('offers one amount for every empty row, only while it would do something', () => {
			const offered = split({
				recipients: [
					{ id: 'a', address: alice, amount: '0.5', name: null },
					{ id: 'b', address: bob, amount: '', name: null }
				],
				split_row_issues: [{ id: 'b', ordinal: 2, address: 'ok', amount: 'empty' }]
			});
			expect(offered.fillEmpty).toEqual({
				amount: '0.5',
				label: 'Use 0.5 ETH for the empty rows'
			});
			// No empty row, nothing to fill.
			expect(split({}).fillEmpty).toBeUndefined();
			// A figure the core rejects is never the one that is copied.
			expect(
				split({
					recipients: [
						{ id: 'a', address: alice, amount: '1,5', name: null },
						{ id: 'b', address: bob, amount: '', name: null }
					],
					split_row_issues: [
						{ id: 'a', ordinal: 1, address: 'ok', amount: 'invalid' },
						{ id: 'b', ordinal: 2, address: 'ok', amount: 'empty' }
					]
				}).fillEmpty
			).toBeUndefined();
		});

		it('a split does not offer Max — there is no single amount for it to fill', () => {
			expect(split({}).token?.max).toBeUndefined();
			expect(liveSendForm(formModel(), inputs({ selected_token: ETH })).token?.max).toBeDefined();
		});

		it('an empty row asks for an address', () => {
			const form = split({
				recipients: [{ id: 'a', address: '', amount: '', name: null }]
			});
			expect(form.recipients?.[0].addressPlaceholder).toBe(m['send.recipientPlaceholder']);
			expect(form.recipients?.[0].identiconSvg).toBe('');
		});
	});

	// Issue #203: the same payee could take two lines of one batch with
	// nothing on screen saying so. The core names the repeats; these two
	// tests say the form and the signing page both repeat what it named.
	it('a repeated payee is named on the row that repeats it, never on the first', () => {
		const form = liveSendForm(
			formModel(),
			inputs({
				selected_token: ETH,
				split_mode: true,
				recipients: [
					{ id: 'a', address: alice, amount: '0.03', name: null },
					{ id: 'b', address: '0x' + 'cd'.repeat(20), amount: '0.03', name: null },
					{ id: 'c', address: alice, amount: '0.03', name: null }
				],
				split_duplicates: [{ id: 'c', first_ordinal: 1 }]
			})
		);
		expect(form.recipients?.[0].duplicateNote).toBeUndefined();
		expect(form.recipients?.[1].duplicateNote).toBeUndefined();
		expect(form.recipients?.[2].duplicateNote).toBe(fill(m['send.recipientDuplicate'], { n: 1 }));
	});

	it('the signing page never shows a name without the address it pays', () => {
		const confirm = liveSendConfirm(
			confirmModel(),
			inputs({
				selected_token: ETH,
				split_mode: true,
				confirm_amount: '0.06',
				recipients: [
					{ id: 'a', address: alice, amount: '0.03', name: 'Alice' },
					{ id: 'b', address: '0x' + 'cd'.repeat(20), amount: '0.03', name: null }
				]
			})
		);
		// A sheet can call any address "Alice": the name is a claim, the address
		// is what is signed for.
		expect(confirm.breakdown?.[0]).toMatchObject({
			label: 'Alice',
			detail: shortenAddress(alice),
			mono: false
		});
		// Nobody named the second: the address is the label, set as one.
		expect(confirm.breakdown?.[1]).toMatchObject({
			label: shortenAddress('0x' + 'cd'.repeat(20)),
			mono: true
		});
		expect(confirm.breakdown?.[1].detail).toBeUndefined();
	});

	it('the signing page says it too — the last screen before a signature', () => {
		const confirm = liveSendConfirm(
			confirmModel(),
			inputs({
				selected_token: ETH,
				split_mode: true,
				confirm_amount: '0.06',
				recipients: [
					{ id: 'a', address: alice, amount: '0.03', name: null },
					{ id: 'b', address: alice, amount: '0.03', name: null }
				],
				split_duplicates: [{ id: 'b', first_ordinal: 1 }]
			})
		);
		expect(confirm.breakdown?.[0].note).toBeUndefined();
		expect(confirm.breakdown?.[1].note).toBe(fill(m['send.recipientDuplicate'], { n: 1 }));
	});
});

describe('the receipt', () => {
	const receipt = (status: 'submitted' | 'confirmed' | 'failed') => ({
		status,
		hold_reason: null,
		kind: null,
		transfers: [],
		amount: '0.5',
		usd_value: 1500,
		submitted_at_ms: null,
		typical_inclusion_s: null
	});

	it('signing shows the submitting state — nothing is accepted yet', () => {
		const model = liveSendReceipt(receiptModel(), inputs({ tx_status: 'signing' }));
		expect(model.stage).toBe('submitting');
		expect(model.hash).toBeUndefined();
	});

	it('accepted but unlanded is SUBMITTED, even though the core calls the send confirmed', () => {
		// The core flips `tx_status` to confirmed the moment the signature is a
		// fact. The receipt's own status is what tracks the chain — reading the
		// wrong one would tell a person their money had arrived mid-air.
		const model = liveSendReceipt(
			receiptModel(),
			inputs({
				tx_status: 'confirmed',
				user_op_hash: '0xop',
				receipt: receipt('submitted'),
				selected_token: ETH
			})
		);
		expect(model.stage).toBe('submitted');
		expect(model.hash?.value).toBe('0xop');
		expect(model.ctaAccent).toBe(false);
	});

	it('a landed hash is the confirmed screen, with the amount the core froze', () => {
		const model = liveSendReceipt(
			receiptModel(),
			inputs({
				tx_status: 'confirmed',
				user_op_hash: '0xop',
				tx_hash: '0xtx',
				receipt: receipt('confirmed'),
				selected_token: ETH,
				recipient: '0x' + 'ab'.repeat(20)
			})
		);
		expect(model.stage).toBe('confirmed');
		expect(model.title).toContain('0.5');
		expect(model.hash?.value).toBe('0xtx');
		expect(model.ctaAccent).toBe(true);
	});

	it('a split lists every recipient the core froze and counts them in the caption (#D2)', () => {
		const alice = '0x' + 'cd'.repeat(20);
		const bob = '0x' + 'ef'.repeat(20);
		const model = liveSendReceipt(
			receiptModel(),
			inputs({
				tx_status: 'confirmed',
				user_op_hash: '0xop',
				tx_hash: '0xtx',
				selected_token: ETH,
				split_mode: true,
				receipt: {
					...receipt('confirmed'),
					kind: 'split',
					transfers: [
						{
							to: alice,
							to_name: 'Alice',
							amount: '0.2',
							symbol: 'ETH',
							logo_urls: [],
							usd_value: 600
						},
						{ to: bob, to_name: null, amount: '0.3', symbol: 'ETH', logo_urls: [], usd_value: 900 }
					]
				}
			})
		);
		expect(model.breakdownTitle).toContain('2');
		expect(model.breakdown?.map((row) => row.label)).toEqual([
			'Alice',
			expect.stringMatching(/^0xef/)
		]);
		expect(model.breakdown?.map((row) => row.value)).toEqual(['0.2 ETH', '0.3 ETH']);
		expect(model.breakdown?.[0].identiconSvg).toBeTruthy();
		// Not "To " with nobody after it.
		expect(model.captions[0]).toContain(model.breakdownTitle ?? '');
	});

	it('a single send carries no parts', () => {
		const model = liveSendReceipt(
			receiptModel(),
			inputs({ tx_status: 'confirmed', tx_hash: '0xtx', receipt: receipt('confirmed') })
		);
		expect(model.breakdown).toBeUndefined();
		expect(model.breakdownTitle).toBeUndefined();
	});

	it('a refused submit is the failed stage, worded by the core’s key', () => {
		const failed = liveSendReceipt(
			receiptModel(),
			inputs({ tx_status: 'error', tx_error: 'generic' })
		);
		expect(failed.stage).toBe('failed');
		expect(failed.title).toBe(m['send.txErrorGeneric']);
		expect(failed.hash).toBeUndefined();
	});
});

/**
 * Issue 211: a send paying gas in a coin the account did not hold went through
 * with nothing said. The core now refuses it — and these are the sentences the
 * shell had been dropping on the floor while it did.
 */
describe('what the form says about a gas coin that cannot pay', () => {
	it("reads the core's live warning out, before any tap", () => {
		const model = liveSendForm(
			formModel(),
			inputs({
				selected_token: ETH,
				amount: '0.5',
				token_amount: '0.5',
				amount_warning: { type: 'need_gas', symbol: 'POL' }
			})
		);
		expect(model.alert).toBe(m['send.warnNeedGas'].replace('{{sym}}', 'POL'));
	});

	it('and the refusal names the coin, instead of blaming the balance that is fine', () => {
		expect(liveSendForm(formModel(), inputs({ selected_token: ETH })).alert).toBeUndefined();
		expect(
			alertWords(
				{
					type: 'insufficient_balance',
					warning: { type: 'need_gas', symbol: 'POL' }
				},
				m
			)
		).toBe(
			`${m['send.alertInsufficientBalanceTitle']} · ${m['send.warnNeedGas'].replace('{{sym}}', 'POL')}`
		);
		// A refusal the core did not qualify keeps the generic sentence.
		expect(alertWords({ type: 'insufficient_balance', warning: null }, m)).toBe(
			`${m['send.alertInsufficientBalanceTitle']} · ${m['send.alertInsufficientBalanceBody']}`
		);
	});

	it('an alert the person just earned outranks the standing warning', () => {
		const model = liveSendForm(formModel(), {
			...inputs({
				selected_token: ETH,
				amount_warning: { type: 'need_gas', symbol: 'POL' }
			}),
			alert: { type: 'invalid_address' }
		});
		expect(model.alert).toContain(m['send.alertInvalidAddressTitle']);
	});
});

/**
 * The follow-up report on issue 211: USDT ticked in the sheet, "0.000392 ETH"
 * still on the row behind it. Two causes, both here — the quote reached the
 * send machine with no symbol on it (the core now carries the relay's own),
 * and this shell filled that gap with the CHAIN's native symbol, which names a
 * different coin.
 */
describe('the fee row names the coin that is paying', () => {
	const usdcQuote = {
		...QUOTE,
		total_wei: '0',
		fee_asset: {
			type: 'erc20' as const,
			token: '0x' + 'cc'.repeat(20),
			decimals: 6,
			amount: '944000',
			symbol: 'USDT'
		}
	};

	it('an erc20 fee reads in that token, never in the native coin', () => {
		const model = liveSendForm(formModel(), inputs({ selected_token: ETH, fee: usdcQuote }));
		expect(model.fee.value).toBe('0.944 USDT');
		expect(model.fee.mark.ticker).toBe('USDT');
	});

	it('a quote with no symbol falls back to the relay row, not to ETH', () => {
		const model = liveSendForm(
			formModel(),
			inputs(
				{
					selected_token: ETH,
					fee: { ...usdcQuote, fee_asset: { ...usdcQuote.fee_asset, symbol: null } }
				},
				{
					options: [
						{
							symbol: 'USDT',
							contract: '0x' + 'cc'.repeat(20),
							decimals: 6,
							balance: '6000000',
							recipient: '0x1',
							usd_balance: '6',
							usd_price: '1',
							amount: '944000',
							insufficient: false,
							selected: true
						}
					]
				}
			)
		);
		expect(model.fee.value).toBe('0.944 USDT');
		expect(model.fee.valueFiat).toBe('≈ $0.94');
		expect(model.fee.mark.ticker).toBe('USDT');
	});

	it('a native fee still reads in the native coin', () => {
		const model = liveSendForm(formModel(), inputs({ selected_token: ETH, fee: QUOTE }));
		expect(model.fee.value).toBe('0.0021 ETH');
		expect(model.fee.valueFiat).toBe('≈ $6.30');
	});
});

/**
 * Issue 201: the fee was the one figure on the send screen with no money
 * beside it. The amount's "≈" line had one, the fee's did not, so a person who
 * does not track the coin's price could not tell what the transfer cost.
 */
describe('the fee row says what the fee costs', () => {
	const cheapChain: SendToken = { ...ETH, chain_id: 56, symbol: 'BNB', price_usd: 600 };
	const bnbQuote = { ...QUOTE, chain_id: 56, total_wei: '91000000000000' };

	it('prices a native fee from the relay row that published the quote', () => {
		const model = liveSendForm(
			formModel(),
			inputs(
				{ selected_token: cheapChain, fee: bnbQuote },
				{
					options: [
						{
							symbol: 'BNB',
							contract: null,
							decimals: 18,
							balance: '1500000000000000000',
							recipient: '0x1',
							usd_balance: '900',
							usd_price: '600',
							amount: '91000000000000',
							insufficient: false,
							selected: true
						}
					]
				}
			)
		);
		expect(model.fee.value).toBe('0.000091 BNB');
		expect(model.fee.valueFiat).toBe('≈ $0.05');
	});

	it('falls back to the balances feed when the relay published no price', () => {
		const model = liveSendForm(
			formModel(),
			inputs({ tokens: [cheapChain], selected_token: cheapChain, fee: bnbQuote })
		);
		expect(model.fee.value).toBe('0.000091 BNB');
		expect(model.fee.valueFiat).toBe('≈ $0.05');
	});

	it('shows the coin alone when nothing can price it', () => {
		const unpriced = { ...cheapChain, price_usd: null };
		const model = liveSendForm(
			formModel(),
			inputs({ tokens: [unpriced], selected_token: unpriced, fee: bnbQuote })
		);
		expect(model.fee.value).toBe('0.000091 BNB');
		expect(model.fee.valueFiat).toBeUndefined();
	});

	// Half a cent is where the fiat half stops helping: "$0.00" beside a real
	// fee reads as free, and the token amount is the honest primary.
	it('leaves off a figure that would round to nothing', () => {
		const dust = { ...bnbQuote, total_wei: '1000000000000' };
		const model = liveSendForm(
			formModel(),
			inputs({ tokens: [cheapChain], selected_token: cheapChain, fee: dust })
		);
		expect(model.fee.value).toBe('0.000001 BNB');
		expect(model.fee.valueFiat).toBeUndefined();
	});

	it('converts into the display currency, at the committed rate only', () => {
		const model = liveSendForm(formModel(), {
			...inputs({ tokens: [cheapChain], selected_token: cheapChain, fee: bnbQuote }),
			currency: { code: 'EUR', rate: 2, committed: true }
		});
		expect(model.fee.value).toBe('0.000091 BNB');
		expect(model.fee.valueFiat).toBe('≈ €0.11');
		const unpriced = liveSendForm(formModel(), {
			...inputs({ tokens: [cheapChain], selected_token: cheapChain, fee: bnbQuote }),
			currency: { code: 'EUR', rate: null, committed: false }
		});
		expect(unpriced.fee.value).toBe('0.000091 BNB');
		expect(unpriced.fee.valueFiat).toBe('≈ $0.05');
	});
});

/**
 * The founder's ruling of 2026-09-17: the confirm page named the coin in words
 * while every row beneath it carried art.
 */
describe('the confirm page draws the coin it is about to send', () => {
	it("carries the selected token's mark", () => {
		const model = liveSendConfirm(
			confirmModel(),
			inputs({ selected_token: USDT, confirm_amount: '5', recipient: '0x' + 'ab'.repeat(20) })
		);
		expect(model.mark?.ticker).toBe('USDT');
	});

	it('carries none on a sweep, where one mark would name the wrong coin', () => {
		const model = liveSendConfirm(
			confirmModel(),
			inputs({ selected_token: USDT, multi_select_mode: true, tokens: [USDT, ETH] })
		);
		expect(model.mark).toBeUndefined();
	});
});

describe('the fee-coin sheet', () => {
	it('lists every row the relay published, including one that cannot pay', () => {
		const model = liveFeeTokenPick(
			feeSheetModel(),
			inputs(
				{ selected_token: ETH },
				{
					options: [
						{
							symbol: 'ETH',
							contract: null,
							decimals: 18,
							balance: '1500000000000000000',
							recipient: '0x1',
							usd_balance: '4500',
							usd_price: '3000',
							amount: '2100000000000000',
							insufficient: false,
							selected: true
						},
						{
							symbol: 'USDC',
							contract: '0x' + 'cc'.repeat(20),
							decimals: 6,
							balance: '0',
							recipient: '0x1',
							usd_balance: '0',
							usd_price: '1',
							amount: null,
							insufficient: true,
							selected: false
						}
					]
				}
			)
		);
		expect(model.rows.map((r) => r.symbol)).toEqual(['ETH', 'USDC']);
		expect(model.rows[0]).toMatchObject({ selected: true, fee: '~0.0021 ETH' });
		// No quote for a coin that cannot pay — said, not guessed.
		expect(model.rows[1].fee).toBe('—');
		// …and the core's verdict travels with it (issue 211). Without this the
		// row looked exactly like a payable one and silently did nothing.
		expect(model.rows[0].insufficient).toBe(false);
		expect(model.rows[1].insufficient).toBe(true);
		expect(model.rows[1].insufficientNote).toBe(
			m['send.warnInsufficientGas'].replace('{{sym}}', 'USDC')
		);
	});

	// Issue 682. The cheaper floor on a coin the relay cannot price is
	// 83,333,333,333,334 wei — 0.000083 OKB, ≈$0.01 at ~$120/coin. This sheet's
	// four-decimal trim truncated that to a bare "0", so the row one tap above
	// read "0.000083 OKB · ≈ $0.01" and the sheet said the fee was "~0 OKB".
	// A fee that reads as free is the worst answer a fee surface can give.
	it('never prints a real fee as "0", however small the coin figure is', () => {
		const model = liveFeeTokenPick(
			feeSheetModel(),
			inputs(
				{ selected_token: ETH },
				{
					options: [
						{
							symbol: 'OKB',
							contract: null,
							decimals: 18,
							balance: '1000000000000000000',
							recipient: '0x1',
							usd_balance: '120',
							usd_price: null,
							amount: '83333333333334',
							insufficient: false,
							selected: true
						}
					]
				}
			)
		);
		expect(model.rows[0].fee).toBe('~0.000083 OKB');
	});
});

// ---------------------------------------------------------------------------
// SD1's two narrowings (spec 028 Phase 10)
// ---------------------------------------------------------------------------

describe('the picker narrows to the sidebar chain and the class chips', () => {
	const XDAI: SendToken = {
		...ETH,
		network: 'gnosis-mainnet',
		chain_id: 100,
		symbol: 'XDAI',
		balance: '71.39',
		price_usd: 1
	};
	const send = { tokens: [USDT, ETH, XDAI], stage: 'select_token' as const };

	it('classes a chain coin as gas, a stable by symbol, the rest as other', () => {
		expect(sendTokenClass(ETH)).toBe('gas');
		expect(sendTokenClass(XDAI)).toBe('gas');
		expect(sendTokenClass(USDT)).toBe('stable');
		expect(sendTokenClass({ ...USDT, symbol: 'LINK' })).toBe('other');
	});

	it('shows every token with no filter, and the chips read "all" selected', () => {
		const model = liveSendPick(pickModel(), inputs(send));
		expect(model.rows.map((r) => r.ticker)).toEqual(['USDT', 'ETH', 'XDAI']);
		expect(model.filters.find((f) => f.selected)?.id).toBe('all');
		expect(model.header.pill?.label).toBe(m['componentsUi.networkFilter.pillAll']);
	});

	it('narrows to the chosen chain and names it on the pill', () => {
		const model = liveSendPick(pickModel(), { ...inputs(send), chainFilter: 100 });
		expect(model.rows.map((r) => r.ticker)).toEqual(['XDAI']);
		expect(model.header.pill?.label).toBe('Gnosis');
		expect(model.header.pill?.dots).toHaveLength(1);
	});

	it('narrows to a class, and the index a tap emits is into the same list', () => {
		const filters = { classFilter: 'gas' as const };
		const model = liveSendPick(pickModel(), { ...inputs(send), ...filters });
		expect(model.rows.map((r) => r.ticker)).toEqual(['ETH', 'XDAI']);
		expect(model.filters.find((f) => f.selected)?.id).toBe('gas');
		// The page resolves the row the same way, so row 1 IS XDAI.
		expect(visibleSendTokens({ ...EMPTY_SEND, ...send }, filters)[1]).toBe(XDAI);
	});

	it('keeps the sweep ticks aligned to the narrowed rows', () => {
		const model = liveSendPick(pickModel(), {
			...inputs({ ...send, multi_selected_ids: [sendTokenId(XDAI)], multi_chain_id: 100 }),
			sweepPicking: true,
			classFilter: 'gas'
		});
		expect(model.rows.map((r) => r.ticker)).toEqual(['ETH', 'XDAI']);
		expect(model.selection?.selected).toEqual([false, true]);
		expect(model.selection?.dimmed).toEqual([true, false]);
	});

	it('keeps a fee figure on screen while a re-quote is out', () => {
		const model = liveSendForm(
			formModel(),
			inputs({ ...send, selected_token: ETH, fee: QUOTE, fee_busy: true })
		);
		expect(model.fee.value).not.toBe('…');
		expect(model.fee.value).toMatch(/ETH/);
	});
});

// ---------------------------------------------------------------------------
// Spec 068 — the fee you can refresh, at a speed you can choose
// ---------------------------------------------------------------------------

describe('the fee row’s refresh and its stale line (spec 068)', () => {
	it('always offers the refresh, named from the corpus', () => {
		const model = liveSendForm(formModel(), inputs({ selected_token: ETH, fee: QUOTE }));
		expect(model.fee.refreshLabel).toBe(m['send.feeRefresh']);
		expect(model.fee.refreshLabel).not.toBe('send.feeRefresh');
	});

	// `FeeView.stale` had NO consumer in this shell before 068: the quote's 30s
	// TTL elapsed and the screen said nothing at all.
	it('says an old quote is old — and says it calmly, in the corpus’s words', () => {
		const fresh = liveSendForm(formModel(), inputs({ selected_token: ETH, fee: QUOTE }));
		expect(fresh.fee.staleNote).toBeUndefined();

		const old = liveSendForm(
			formModel(),
			inputs({ selected_token: ETH, fee: QUOTE }, { stale: true })
		);
		expect(old.fee.staleNote).toBe(m['send.feeStale']);
		// The figure is still there. "Old" is not "gone", and blanking it would
		// be a worse answer than the number the person is looking at.
		expect(old.fee.value).toBe('0.0021 ETH');
	});

	it('stops saying it the moment a fresh measurement is out', () => {
		const measuring = liveSendForm(
			formModel(),
			inputs({ selected_token: ETH, fee: QUOTE }, { stale: true, busy: true })
		);
		expect(measuring.fee.staleNote).toBeUndefined();
		expect(measuring.fee.refreshing).toBe(true);
	});

	it('is not spinning when nothing is in flight', () => {
		const settled = liveSendForm(formModel(), inputs({ selected_token: ETH, fee: QUOTE }));
		expect(settled.fee.refreshing).toBe(false);
	});
});

describe('the folded speed control (spec 068)', () => {
	const quoteAt = (tier: 'fast' | 'standard' | 'slow', totalWei: string) => ({
		...IDLE_FEE,
		fee: { ...QUOTE, tier, total_wei: totalWei }
	});
	/**
	 * The control's inputs as the route builds them: the REAL `fee_speed` core
	 * (spec 069), driven the way the route drives it, over these rows.
	 *
	 * `tier` is the tier the test wants in force. With no flag it is simply the
	 * stored default, with the send off its form so no free upgrade is asked
	 * about; `picked` makes it a one-shot pick over another default; `free`
	 * makes the default Slow on the form, so the core takes Fast itself if the
	 * numbers say it is free; `oneSpeedKnown` first shows the core a settled
	 * one-speed set on this chain. The shell's promotion is modelled by
	 * re-reporting the rows until the tier in force settles.
	 */
	const speedInputs = (
		open: boolean,
		tier: 'fast' | 'standard' | 'slow',
		rows: { tier: 'fast' | 'standard' | 'slow'; view: FeeView; busy?: boolean }[],
		how: { picked?: boolean; free?: boolean; oneSpeedKnown?: boolean } = {}
	): SendLiveInputs => ({
		...inputs({ selected_token: ETH, fee: QUOTE }),
		speed: {
			view: driveSpeed(open, tier, rows, how),
			feeOptions: (t) => rows.find((row) => row.tier === t)?.view.options ?? []
		}
	});
	function driveSpeed(
		open: boolean,
		tier: 'fast' | 'standard' | 'slow',
		rows: { tier: 'fast' | 'standard' | 'slow'; view: FeeView; busy?: boolean }[],
		how: { picked?: boolean; free?: boolean; oneSpeedKnown?: boolean }
	): FeeSpeedView {
		const core = new FeeSpeedCore();
		let view = JSON.parse(core.view()) as FeeSpeedView;
		const send = (event: FeeSpeedEvent) => {
			view = (JSON.parse(core.dispatch(JSON.stringify(event))) as { view: FeeSpeedView }).view;
		};
		const report = (set: typeof rows) => {
			for (let round = 0; round < 3; round += 1) {
				const before = view.tier;
				const mine = set.find((row) => row.tier === view.tier);
				send({
					type: 'quotes_changed',
					chain_id: QUOTE.chain_id,
					in_force: {
						busy: (mine?.busy ?? false) || (mine?.view.busy ?? false),
						fee: mine?.view.fee ?? null
					},
					previews: set
						.filter((row) => row.tier !== view.tier)
						.map((row) => ({
							tier: row.tier,
							busy: (row.busy ?? false) || row.view.busy,
							fee: row.view.fee
						}))
				});
				if (view.tier === before) break;
			}
		};
		const preferred = how.free ? 'slow' : how.picked ? (tier === 'fast' ? 'slow' : 'fast') : tier;
		send({ type: 'configure', preferred, number: 'comma_dot' });
		if (how.oneSpeedKnown) {
			const flat = (t: 'fast' | 'standard' | 'slow') => ({
				tier: t,
				view: { ...IDLE_FEE, fee: { ...QUOTE, tier: t, total_wei: '10000000000000' } }
			});
			report([flat('fast'), flat('standard'), flat('slow')]);
			send({ type: 'reset' });
		}
		send({ type: 'stage_changed', on_form: how.free === true });
		if (how.picked) send({ type: 'pick', tier });
		if (open) send({ type: 'toggle' });
		report(rows);
		core.free();
		return view;
	}

	const THREE = [
		{ tier: 'fast' as const, view: quoteAt('fast', '2100000000000000') },
		{ tier: 'standard' as const, view: quoteAt('standard', '1300000000000000') },
		{ tier: 'slow' as const, view: quoteAt('slow', '1000000000000000') }
	];

	it('is not drawn at all where no session can honour it', () => {
		expect(liveSendForm(formModel(), inputs({ selected_token: ETH })).speed).toBeUndefined();
	});

	// The trap this whole control exists to avoid: a folded line that shows a
	// hardcoded tier would disagree with Settings the moment somebody changed
	// their default.
	it('shows the tier in force folded, whichever one that is', () => {
		for (const tier of ['fast', 'standard', 'slow'] as const) {
			const model = liveSendForm(formModel(), speedInputs(false, tier, THREE));
			expect(model.speed?.open).toBe(false);
			expect(model.speed?.value).toBe(m[`send.gasTier.${tier}` as 'send.gasTier.fast']);
			expect(model.speed?.options.filter((o) => o.selected).map((o) => o.id)).toEqual([tier]);
		}
	});

	// Named by SPEED, all three of them: the control's own label is "Speed", so
	// a name that answered "cheap" instead would be answering another question.
	it('names the three tiers from the corpus and never offers the dead fourth', () => {
		const model = liveSendForm(formModel(), speedInputs(true, 'fast', THREE));
		expect(model.speed?.options.map((o) => o.id)).toEqual(['fast', 'standard', 'slow']);
		expect(model.speed?.options.map((o) => o.label)).toEqual(['Fast', 'Standard', 'Slow']);
		expect(model.speed?.options.some((o) => o.id === 'rapid')).toBe(false);
	});

	// …and the advantage the name no longer carries, one line per option. This
	// is what makes the slow tier a choice rather than a defect, so a row that
	// lost it would quietly undo the rename above.
	it('gives every option the line that says what that speed buys', () => {
		const model = liveSendForm(formModel(), speedInputs(true, 'fast', THREE));
		expect(model.speed?.options.map((o) => o.detail)).toEqual([
			m['send.gasTierHintFast'],
			m['send.gasTierHintStandard'],
			m['send.gasTierHintSlow']
		]);
		expect(model.speed?.options.map((o) => o.detail)).toEqual([
			'First to confirm, even when the network is busy',
			'Balanced for everyday transfers',
			'Lowest fee, if you can wait'
		]);
	});

	// Each option's figure is its OWN quote's — never one number scaled into
	// three. The reported tier price and the relay's submit cap are different
	// quantities (spec 068), so arithmetic here would be wrong as well as a
	// second writer of a number the core owns.
	it('gives every option its own fee, from its own quote', () => {
		const model = liveSendForm(formModel(), speedInputs(true, 'fast', THREE));
		expect(model.speed?.options.map((o) => o.value)).toEqual([
			'0.0021 ETH',
			'0.0013 ETH',
			'0.001 ETH'
		]);
		expect(model.speed?.options.map((o) => o.valueFiat)).toEqual(['≈ $6.30', '≈ $3.90', '≈ $3.00']);
	});

	it('waits for a tier’s own measurement rather than borrowing another tier’s', () => {
		const model = liveSendForm(
			formModel(),
			speedInputs(true, 'fast', [
				THREE[0],
				{ tier: 'standard', view: IDLE_FEE, busy: true },
				{ tier: 'slow', view: { ...IDLE_FEE, busy: true } }
			])
		);
		expect(model.speed?.options.map((o) => o.value)).toEqual(['0.0021 ETH', '…', '…']);
		expect(model.speed?.options.map((o) => o.valueFiat)).toEqual(['≈ $6.30', undefined, undefined]);
	});

	it('says a dash, not a borrowed figure, when a tier settles with nothing', () => {
		const model = liveSendForm(
			formModel(),
			speedInputs(true, 'fast', [THREE[0], THREE[1], { tier: 'slow', view: IDLE_FEE }])
		);
		expect(model.speed?.options[2].value).toBe('—');
	});

	/**
	 * Issue 684 — what each speed actually BUYS.
	 *
	 * The owner surveyed the picker across every network and found three
	 * identical fees on seven of them. Measured, the cause is the $0.01 dust
	 * floor: `fee_policy` clamps every tier to it on any chain whose real cost
	 * is under a cent, so three genuinely different tiers round to one figure.
	 * The tiers still buy different inclusion — each signs a different tip —
	 * and this is the only thing on the row that shows it.
	 */
	describe('the gas price beside each speed (issue 684)', () => {
		const priced = (
			tier: 'fast' | 'standard' | 'slow',
			totalWei: string,
			effective: string | null
		) => ({
			tier,
			view: {
				...IDLE_FEE,
				fee: { ...QUOTE, tier, total_wei: totalWei, effective_gas_price: effective }
			}
		});

		it('states the effective gas price the core settled on, per tier', () => {
			// The three mined Polygon receipts, to the wei.
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					priced('fast', '2100000000000000', '299589817385'),
					priced('standard', '1300000000000000', '282464783233'),
					priced('slow', '1000000000000000', '270164477149')
				])
			);
			expect(model.speed?.options.map((o) => o.gasPrice)).toEqual([
				'300 gwei',
				'282 gwei',
				'270 gwei'
			]);
		});

		/**
		 * The case the whole figure exists for: every tier clamped to the same
		 * $0.01 fee, so the fees say nothing and the gas prices say everything.
		 * Optimism's real underlying tiers, which are all under a cent.
		 */
		it('is what tells three floor-clamped tiers apart', () => {
			const cent = (tier: 'fast' | 'standard' | 'slow', effective: string) => ({
				tier,
				view: {
					...IDLE_FEE,
					fee: {
						...QUOTE,
						tier,
						total_wei: '0',
						effective_gas_price: effective,
						fee_asset: {
							type: 'erc20' as const,
							token: '0xdead',
							decimals: 6,
							amount: '10000',
							symbol: 'USDC'
						}
					}
				}
			});
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					cent('fast', '3244'),
					cent('standard', '2377'),
					cent('slow', '1937')
				])
			);
			// Three rows, still. Nothing collapses and nothing is hidden.
			expect(model.speed?.options).toHaveLength(3);
			expect(model.speed?.options.map((o) => o.value)).toEqual([
				'0.01 USDC',
				'0.01 USDC',
				'0.01 USDC'
			]);
			expect(model.speed?.options.map((o) => o.gasPrice)).toEqual([
				'3,244 wei',
				'2,377 wei',
				'1,937 wei'
			]);
		});

		it('says nothing at all where there is no honest number', () => {
			// A chain with no priority fee (Tempo): every tier settled, none
			// with a figure. Nothing is drawn, and no empty line is kept for it.
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					priced('fast', '2100000000000000', null),
					priced('standard', '1300000000000000', null),
					priced('slow', '1000000000000000', null)
				])
			);
			expect(model.speed?.options.map((o) => o.gasPrice)).toEqual([
				undefined,
				undefined,
				undefined
			]);
			expect(model.speed?.gasPriceLine).toBe(false);
		});

		/**
		 * The set is formatted together, so it is drawn together. Each tier is
		 * its own quote session and they land one at a time; drawn as they came,
		 * a row's "270 gwei" became "270.2 gwei" — or changed unit — when a
		 * neighbour arrived. Held back until every row has answered, and the
		 * line held open meanwhile so the option does not change height.
		 */
		it('draws the gas prices once the whole set has answered, not row by row', () => {
			const partial = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					priced('fast', '2100000000000000', '299589817385'),
					priced('standard', '1300000000000000', '270400000000'),
					{ tier: 'slow' as const, view: IDLE_FEE, busy: true }
				])
			);
			expect(partial.speed?.options.map((o) => o.gasPrice)).toEqual([
				undefined,
				undefined,
				undefined
			]);
			expect(partial.speed?.gasPriceLine).toBe(true);
			const complete = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					priced('fast', '2100000000000000', '299589817385'),
					priced('standard', '1300000000000000', '270400000000'),
					priced('slow', '1000000000000000', '270164477149')
				])
			);
			expect(complete.speed?.options.map((o) => o.gasPrice)).toEqual([
				'299.6 gwei',
				'270.4 gwei',
				'270.2 gwei'
			]);
			expect(complete.speed?.gasPriceLine).toBe(true);
		});

		// A refresh is not a wait: the row keeps its settled figure while it
		// re-measures, as the fee beside it does, so the gas prices hold too.
		it('keeps the gas prices through a refresh, as the fees are kept', () => {
			const refreshing = (row: ReturnType<typeof priced>) => ({
				...row,
				view: { ...row.view, busy: true }
			});
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					refreshing(priced('fast', '2100000000000000', '299589817385')),
					priced('standard', '1300000000000000', '282464783233'),
					refreshing(priced('slow', '1000000000000000', '270164477149'))
				])
			);
			expect(model.speed?.options.map((o) => o.gasPrice)).toEqual([
				'300 gwei',
				'282 gwei',
				'270 gwei'
			]);
		});

		it('never draws another tier’s gas price under this tier’s name', () => {
			// The row in force renders the MAIN session's view, which still holds
			// the previous speed's quote for a frame after a tap (issue 681).
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'slow', [
					priced('fast', '2100000000000000', '299589817385'),
					priced('standard', '1300000000000000', '282464783233'),
					{ tier: 'slow' as const, view: priced('fast', '2100000000000000', '299589817385').view }
				])
			);
			expect(model.speed?.options[2].gasPrice).toBeUndefined();
		});

		it('names the figure for a screen reader, from the corpus', () => {
			const model = liveSendForm(formModel(), speedInputs(true, 'fast', THREE));
			expect(model.speed?.gasPriceLabel).toBe(m['send.gasPriceLabel']);
			expect(model.speed?.gasPriceLabel).not.toBe('send.gasPriceLabel');
		});
	});

	/**
	 * Issue 685 — the gas price as a RANGE: what a speed bids now ~ its cap.
	 *
	 * The owner, on an ETH L2: 0.00006 / 0.00004 / 0.000033 ETH over 0.02021 /
	 * 0.02013 / 0.02011 gwei. The fees differ 1.8×, the bids 0.5%, and a person
	 * paying 80% more for a figure 0.5% higher concludes they are being cheated.
	 * Most of what the dearer tier buys is its cap, so the cap is drawn too.
	 */
	describe('the gas price as a range (issue 685)', () => {
		const ranged = (
			tier: 'fast' | 'standard' | 'slow',
			totalWei: string,
			effective: string | null,
			max: string | null
		) => ({
			tier,
			view: {
				...IDLE_FEE,
				fee: {
					...QUOTE,
					tier,
					total_wei: totalWei,
					effective_gas_price: effective,
					max_gas_price: max
				}
			}
		});

		it('draws each tier as its bid ~ its cap, from the core’s own numbers', () => {
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					ranged('fast', '60000000000000', '20210000', '60410000'),
					ranged('standard', '40000000000000', '20130000', '40230000'),
					ranged('slow', '33000000000000', '20110000', '30160000')
				])
			);
			expect(model.speed?.options.map((o) => o.gasPrice)).toEqual([
				'0.02021 ~ 0.06041 gwei',
				'0.02013 ~ 0.04023 gwei',
				'0.02011 ~ 0.03016 gwei'
			]);
		});

		it('draws the single figure where the two ends are one number', () => {
			// BSC: no base fee, so every cap is its own tip.
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					ranged('fast', '2100000000000000', '100000000', '100000000'),
					ranged('standard', '1300000000000000', '62500000', '62500000'),
					ranged('slow', '1000000000000000', '50000000', '50000000')
				])
			);
			expect(model.speed?.options.map((o) => o.gasPrice)).toEqual([
				'0.1 gwei',
				'0.0625 gwei',
				'0.05 gwei'
			]);
		});

		it('never draws a cap without the bid it caps', () => {
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					ranged('fast', '2100000000000000', null, '805065222658'),
					ranged('standard', '1300000000000000', null, '527288291674'),
					ranged('slow', '1000000000000000', null, '390302745042')
				])
			);
			expect(model.speed?.options.map((o) => o.gasPrice)).toEqual([
				undefined,
				undefined,
				undefined
			]);
			expect(model.speed?.gasPriceLine).toBe(false);
		});

		it('holds all three ranges back until every tier has answered', () => {
			const partial = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					ranged('fast', '2100000000000000', '299589817385', '805065222658'),
					ranged('standard', '1300000000000000', '282464783233', '527288291674'),
					{ tier: 'slow' as const, view: IDLE_FEE, busy: true }
				])
			);
			expect(partial.speed?.options.map((o) => o.gasPrice)).toEqual([
				undefined,
				undefined,
				undefined
			]);
			expect(partial.speed?.gasPriceLine).toBe(true);
		});
	});

	/**
	 * Issue 681 — the fee row after a pick that had to re-measure.
	 *
	 * Promotion covers the normal tap: the row the person touched was already a
	 * settled quote, so it simply becomes the fee in force. The exception is a
	 * tap on a row that had not landed yet (the picker was re-opened a moment
	 * ago, say), where a real measurement is the only honest answer. What the
	 * row must NOT do while that measurement is out is keep drawing the money of
	 * the speed that was just left — `send.fee` survives a tier change, so it
	 * would, and the person would read 超快's figure under 较慢's name. That is
	 * the reported defect, one size smaller.
	 */
	it('never shows the previous tier’s money under the new tier’s name', () => {
		const measuring = liveSendForm(
			formModel(),
			// The person has just chosen Slow; everything in hand was priced Fast.
			speedInputs(false, 'slow', [THREE[0], THREE[1], { tier: 'slow', view: IDLE_FEE, busy: true }])
		);
		expect(measuring.fee.value).toBe('…');
		expect(measuring.fee.valueFiat).toBeUndefined();
		// The option in force says the same thing, rather than borrowing the
		// figure the row above just stopped showing.
		expect(measuring.speed?.options.find((o) => o.id === 'slow')?.value).toBe('…');
		// And "this figure is from a while ago" is a fact about a figure.
		const old = liveSendForm(formModel(), {
			...speedInputs(false, 'slow', THREE),
			fee: { ...IDLE_FEE, stale: true }
		});
		expect(old.fee.value).toBe('…');
		expect(old.fee.staleNote).toBeUndefined();
		// Once this tier's own quote lands, the row is a figure again.
		const landed = liveSendForm(formModel(), {
			...speedInputs(false, 'slow', THREE),
			send: { ...EMPTY_SEND, selected_token: ETH, fee: { ...QUOTE, tier: 'slow' } }
		});
		expect(landed.fee.value).toBe('0.0021 ETH');
	});

	// The promise the picker makes, in words, before the tap that would
	// otherwise look like it rewrote a setting.
	it('says out loud that a pick here is one-shot', () => {
		const model = liveSendForm(formModel(), speedInputs(true, 'slow', THREE));
		expect(model.speed?.onceNote).toBe(m['send.feeSpeedOnce']);
		expect(model.speed?.label).toBe(m['send.feeSpeedLabel']);
	});

	// The confirm is the last screen before a signature. A speed that was
	// CHOSEN belongs on it — that is where a mis-tap is still cheap to undo —
	// while a send at the stored default adds no row, because most sends need
	// no decision about speed and a permanent line would ask for one.
	it('restates a chosen speed on the confirm, and only a chosen one', () => {
		const chosen = liveSendConfirm(confirmModel(), speedInputs(false, 'slow', THREE, { picked: true }));
		expect(new Map(chosen.facts.map((f) => [f.label, f.value])).get(m['send.feeSpeedLabel'])).toBe(
			m['send.gasTier.slow']
		);

		const untouched = liveSendConfirm(confirmModel(), speedInputs(false, 'fast', THREE));
		expect(untouched.facts.some((f) => f.label === m['send.feeSpeedLabel'])).toBe(false);
	});

	/**
	 * Issue 686 — what picking a speed buys on THIS network.
	 *
	 * A: the fastest speed taken because it costs no more than a slower
	 * default — said on the control, restated on the confirm.
	 * B: a network whose speeds nothing tells apart (Tempo) — one statement
	 * where three options offered a choice that does nothing.
	 */
	describe('when the choice buys nothing extra (issue 686)', () => {
		const at = (
			tier: 'fast' | 'standard' | 'slow',
			totalWei: string,
			range: [string, string] | null
		) => ({
			tier,
			view: {
				...IDLE_FEE,
				fee: {
					...QUOTE,
					tier,
					total_wei: totalWei,
					effective_gas_price: range?.[0] ?? null,
					max_gas_price: range?.[1] ?? null
				}
			}
		});
		const TEMPO = [
			at('fast', '10000000000000', null),
			at('standard', '10000000000000', null),
			at('slow', '10000000000000', null)
		];
		const FLOOR_CLAMPED = [
			at('fast', '10000000000000', ['3244', '9000']),
			at('standard', '10000000000000', ['2377', '6000']),
			at('slow', '10000000000000', ['1937', '4500'])
		];

		it('says a Tempo-shaped network has one speed, instead of three options', () => {
			const model = liveSendForm(formModel(), speedInputs(true, 'fast', TEMPO));
			expect(model.speed?.singleNote).toBe(m['send.feeSpeedSingle']);
			expect(model.speed?.singleNote).not.toBe('send.feeSpeedSingle');
		});

		it('keeps all three options where only the fees are equal', () => {
			// The owner's ruling: on a floor-clamped chain the range is the choice.
			const model = liveSendForm(formModel(), speedInputs(true, 'fast', FLOOR_CLAMPED));
			expect(model.speed?.singleNote).toBeUndefined();
			expect(model.speed?.options).toHaveLength(3);
		});

		it('keeps the options, not a premature statement, while a speed is measuring', () => {
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					TEMPO[0],
					TEMPO[1],
					{ tier: 'slow' as const, view: IDLE_FEE, busy: true }
				])
			);
			expect(model.speed?.singleNote).toBeUndefined();
		});

		it('keeps the statement through a refresh rather than blinking back to three rows', () => {
			const model = liveSendForm(
				formModel(),
				speedInputs(true, 'fast', [
					{ ...TEMPO[0], view: { ...TEMPO[0].view, busy: true }, busy: true },
					TEMPO[1],
					TEMPO[2]
				])
			);
			expect(model.speed?.singleNote).toBe(m['send.feeSpeedSingle']);
		});

		it('says why a slower default is running at the fastest speed', () => {
			const model = liveSendForm(formModel(), speedInputs(false, 'fast', FLOOR_CLAMPED, { free: true }));
			// The summary names the tier actually in force…
			expect(model.speed?.value).toBe(m['send.gasTier.fast']);
			// …and says why, in the corpus's words.
			expect(model.speed?.freeNote).toBe(m['send.feeSpeedFree']);
			expect(model.speed?.freeNote).not.toBe('send.feeSpeedFree');

			const notFree = liveSendForm(formModel(), speedInputs(false, 'fast', FLOOR_CLAMPED));
			expect(notFree.speed?.freeNote).toBeUndefined();
		});

		it('never calls a speed free over a network with one speed (B before A)', () => {
			const model = liveSendForm(formModel(), speedInputs(true, 'fast', TEMPO, { free: true }));
			expect(model.speed?.singleNote).toBe(m['send.feeSpeedSingle']);
			expect(model.speed?.freeNote).toBeUndefined();
		});

		it('restates a free upgrade on the confirm, as it restates a pick', () => {
			const model = liveSendConfirm(confirmModel(), speedInputs(false, 'fast', FLOOR_CLAMPED, { free: true }));
			expect(new Map(model.facts.map((f) => [f.label, f.value])).get(m['send.feeSpeedLabel'])).toBe(
				m['send.gasTier.fast']
			);
			// …with its reason, so Settings saying Slow is not left unexplained on
			// the last screen before the signature either.
			const fact = model.facts.find((f) => f.label === m['send.feeSpeedLabel']);
			expect(fact?.note).toBe(m['send.feeSpeedFree']);

			// A pick carries no reason: the person made it.
			const picked = liveSendConfirm(confirmModel(), speedInputs(true, 'slow', FLOOR_CLAMPED));
			expect(picked.facts.find((f) => f.label === m['send.feeSpeedLabel'])?.note).toBeUndefined();
		});

		const measuring = [
			TEMPO[0],
			{ tier: 'standard' as const, view: IDLE_FEE, busy: true },
			{ tier: 'slow' as const, view: IDLE_FEE, busy: true }
		];

		it('says a network it already knows has one speed at once, while the rest re-measure', () => {
			const model = liveSendForm(formModel(), speedInputs(true, 'fast', measuring, { oneSpeedKnown: true }));
			expect(model.speed?.singleNote).toBe(m['send.feeSpeedSingle']);
			// Unknown, the same frame is three rows still measuring — never a guess.
			const unknown = liveSendForm(formModel(), speedInputs(true, 'fast', measuring));
			expect(unknown.speed?.singleNote).toBeUndefined();
		});

		it('lets settled numbers overrule what it remembered', () => {
			const model = liveSendForm(formModel(), speedInputs(true, 'fast', FLOOR_CLAMPED, { oneSpeedKnown: true }));
			expect(model.speed?.singleNote).toBeUndefined();
			expect(model.speed?.options).toHaveLength(3);
		});

	});
});
