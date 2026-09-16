/**
 * The send overlays (spec 026 T236): the drawn screens filled from `SendView`
 * and `FeeView`, and nothing else. Every assertion here is "the core said so".
 */
import { describe, expect, it } from 'vitest';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import type { WalletIdentity } from '$lib/wallet/identity';
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
		expect(byLabel.get(m['send.estFeeLabel'])).toBe('0.0021 ETH');
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
		expect(model.fee.mark.ticker).toBe('USDT');
	});

	it('a native fee still reads in the native coin', () => {
		const model = liveSendForm(formModel(), inputs({ selected_token: ETH, fee: QUOTE }));
		expect(model.fee.value).toBe('0.0021 ETH');
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
