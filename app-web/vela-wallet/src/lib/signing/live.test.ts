/**
 * The signing sheet's builder (spec 026 T244).
 *
 * Two properties matter more than any layout question, and both are asserted
 * here: the confirm gate is an AND of three separate answers, and the
 * never-unlimited mandate reaches the screen as a DISABLED chip plus a shut
 * slider — not as a warning somebody can slide past.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { ClearOperation } from '$lib/core/generated/ClearOperation';
import type { ClearShellResult } from '$lib/core/generated/ClearShellResult';
import type { ClearSigningEvent } from '$lib/core/generated/ClearSigningEvent';
import type { ClearProvenance } from '$lib/core/generated/ClearProvenance';
import type { ClearSignField } from '$lib/core/generated/ClearSignField';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { FeeSpeedEvent } from '$lib/core/generated/FeeSpeedEvent';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { ClearSigningCore, FeeSpeedCore } from '$lib/core/client';
import { toClearLocale } from './core/clear-types';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { SignView } from '$lib/core/generated/SignView';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { CLEAR_TERMS } from './terms';
import { shortenAddress, type WalletIdentity } from '$lib/wallet/identity';
import { INITIAL_CLEAR_VIEW, INITIAL_GUARD_VIEW } from './core/sheet.svelte';
import { txKickoff } from './core/tx-params';
import { INITIAL_SIGN_VIEW } from './core/sign-resident.svelte';
import { clearEstimateReverts, recordEstimateReverts } from '$lib/services/estimate-verdict';
import { fill } from '$lib/wallet/messages';
import {
	approveOptsOf,
	buildSigningModel,
	calldataBytes,
	cappedApproval,
	localizedTerms,
	signingCloseEvent,
	signingStatus,
	summaryOf,
	type SigningLiveInputs
} from './live';

const m = resolveSigningMessages('en');
const identity: WalletIdentity = {
	name: 'My Wallet',
	address: '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c',
	identiconSvg: '<svg/>'
};
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;

const REQUEST = {
	id: 'req-1',
	method: 'eth_sendTransaction',
	kind: 'transaction' as const,
	params_json: '[{"to":"0xdead","data":"0x","value":"0x0"}]',
	origin: 'https://app.example',
	dapp: null,
	chain_id: 1,
	signer_address: identity.address
};

const OPEN_SIGN: SignView = {
	...INITIAL_SIGN_VIEW,
	surface: 'sheet',
	request: REQUEST,
	confirm_gate_open: true
};

const QUOTED_FEE: FeeView = {
	busy: false,
	failed: null,
	fee: {
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
		tier: 'fast',
		quoted: true,
		fee_asset: { type: 'native' },
		fee_recipient: null
	},
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: true
};

function field(over: Partial<ClearSignField> = {}): ClearSignField {
	return {
		label: 'Amount',
		value: '100 USDC',
		format: 'amount',
		token_address: null,
		warning: false,
		unverified: false,
		role: 'send_amount',
		detail: false,
		expired: false,
		address: null,
		usd_value: 100,
		label_term: null,
		value_term: null,
		...over
	} as ClearSignField;
}

const DECODED: ClearSigningView = {
	...INITIAL_CLEAR_VIEW,
	resolved: true,
	surface: 'clear_sign',
	result: {
		intent: 'Send USDC',
		intent_term: null,
		contract_name: 'USD Coin',
		owner: null,
		fields: [
			field(),
			field({ label: 'To', value: 'alice.eth', role: 'recipient', address: '0xab' })
		],
		risk: 'normal',
		contract_address: '0x' + 'cc'.repeat(20),
		verified: true,
		provenance: 'built_in',
		sign_type: 'transaction',
		partial: false,
		best_effort: false,
		to_own_token: false,
		terms_off_chain: false
	}
};

function inputs(over: Partial<SigningLiveInputs> = {}): SigningLiveInputs {
	return {
		sign: OPEN_SIGN,
		clear: DECODED,
		guard: INITIAL_GUARD_VIEW,
		fee: QUOTED_FEE,
		currency: { code: 'USD', rate: 1, committed: true },
		m,
		identity,
		identicon,
		...over
	};
}

/**
 * The sheet and the send screens quote one fee session, and the design sheet
 * says they must not drift. This one printed the estimate's NATIVE figure
 * beside the CHAIN's name, and never what the fee cost (issue 201).
 */
describe('the fee the sheet shows', () => {
	it('reads in the coin that pays, with what it costs', () => {
		const priced = {
			...QUOTED_FEE,
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
					selected: true,
					spent_by_operation: false
				}
			]
		};
		const model = buildSigningModel(inputs({ fee: priced }));
		expect(model?.fee).toEqual({
			kind: 'onchain',
			label: m.feeLabel,
			value: '0.0021 ETH · ≈$6.30',
			selector: undefined,
			speed: undefined,
			warning: undefined,
			// One coin and a quote in hand: nothing to choose, nothing to ask
			// again. The row is drawn as the fee STATED — no chevron, no pointer
			// (spec 081, dead-controls #6: it was a button whose handler, live,
			// was `SigningHost`'s defaulted no-op).
			tappable: false,
			// Spec 079: the send form's refresh control, and no chevron — one
			// coin, so there is no list for a tap to open.
			refreshLabel: m.feeRefresh,
			refreshing: false,
			chevron: false
		});
	});

	/** Two coins to pay in, and the row is the door to the list again. */
	it('is a control again when there is a coin to choose', () => {
		const priced = {
			...QUOTED_FEE,
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
					selected: true,
					spent_by_operation: false
				},
				{
					symbol: 'USDC',
					contract: '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48',
					decimals: 6,
					balance: '2000000',
					recipient: '0x1',
					usd_balance: '2',
					usd_price: '1',
					amount: '6300000',
					insufficient: false,
					selected: false,
					spent_by_operation: false
				}
			]
		};
		const model = buildSigningModel(inputs({ fee: priced }));
		expect(model?.fee).toMatchObject({ kind: 'onchain', tappable: true });
	});

	/** A refused quote can always be asked again, one coin or many. */
	it('is a control when the quote failed, with one coin', () => {
		const failed = {
			...QUOTED_FEE,
			fee: null,
			busy: false,
			failed: 'quote_unavailable' as const,
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
					selected: true,
					spent_by_operation: false
				}
			]
		};
		const model = buildSigningModel(inputs({ fee: failed }));
		expect(model?.fee).toMatchObject({ kind: 'onchain', tappable: true });
	});

	// Issue 262: 0 ETH and 2 USDT on mainnet, quoted in ETH. The core keeps the
	// quote (the person sees the figure) and shuts the gate; the row says why.
	it('says why the slide is shut when the coin that pays is not there', () => {
		const eth = {
			symbol: 'ETH',
			contract: null,
			decimals: 18,
			balance: '0',
			recipient: '0x1',
			usd_balance: '0',
			usd_price: '3000',
			amount: '2100000000000000',
			insufficient: true,
			selected: true,
			spent_by_operation: false
		};
		const usdt = {
			...eth,
			symbol: 'USDT',
			contract: '0x' + 'cc'.repeat(20),
			decimals: 6,
			balance: '2000000',
			usd_balance: '2',
			usd_price: '1',
			amount: '6300000',
			insufficient: false,
			selected: false,
			spent_by_operation: false
		};
		const short = { ...QUOTED_FEE, options: [eth, usdt], confirm_fee_ready: false };
		const model = buildSigningModel(inputs({ fee: short }));
		expect(model?.fee).toMatchObject({ warning: 'Insufficient ETH for gas fees' });
		expect(model?.confirm.enabled).toBe(false);

		const paid = {
			...short,
			options: [{ ...eth, balance: '1', insufficient: false }],
			confirm_fee_ready: true
		};
		expect(buildSigningModel(inputs({ fee: paid }))?.fee).not.toHaveProperty(
			'warning',
			expect.anything()
		);
	});

	it('names the ERC-20 that is paying, in its own decimals', () => {
		const usdt = {
			...QUOTED_FEE,
			fee: {
				...QUOTED_FEE.fee!,
				fee_asset: {
					type: 'erc20' as const,
					token: '0x' + 'cc'.repeat(20),
					decimals: 6,
					amount: '944000',
					symbol: 'USDT'
				}
			}
		};
		const model = buildSigningModel(inputs({ fee: usdt }));
		// Not "0.0021 Ethereum": a different coin, and eighteen decimals of it.
		expect(model?.fee).toMatchObject({ value: '0.944 USDT' });
	});

	it('shows the coin alone when nothing can price it', () => {
		expect(buildSigningModel(inputs())?.fee).toMatchObject({ value: '0.0021 ETH' });
	});
});

/**
 * Spec 079 (the owner: "似乎没有刷新网络费的按钮呀"; F12: a relay that did not
 * answer turned the fee into "点击重试" with no reason, and nothing asked again
 * when it came back).
 */
describe('the fee can be refreshed, and says why it failed', () => {
	const ETH_OPTION = {
		symbol: 'ETH',
		contract: null,
		decimals: 18,
		balance: '1500000000000000000',
		recipient: '0x1',
		usd_balance: '4500',
		usd_price: '3000',
		amount: '2100000000000000',
		insufficient: false,
		selected: true,
		spent_by_operation: false
	};
	const failedWith = (failed: FeeView['failed']): FeeView => ({
		...QUOTED_FEE,
		fee: null,
		busy: false,
		failed,
		confirm_fee_ready: false,
		options: [ETH_OPTION]
	});

	it('a shown fee carries the send form’s refresh, still while nothing is measuring', () => {
		const fee = buildSigningModel(inputs())?.fee;
		expect(fee).toMatchObject({ refreshLabel: m.feeRefresh, refreshing: false });
	});

	it('the control turns while a measurement is out', () => {
		const fee = buildSigningModel(
			inputs({ fee: { ...QUOTED_FEE, fee: null, busy: true, confirm_fee_ready: false } })
		)?.fee;
		expect(fee).toMatchObject({ value: m.feeEstimating, refreshing: true });
	});

	const NET = m.feeReasons['componentsUi.funding.denialNetworkError'];

	it('a relay out of reach says so, and that the sheet asks again — the row is still a retry', () => {
		const fee = buildSigningModel(inputs({ fee: failedWith('quote_unavailable') }))?.fee;
		expect(fee).toMatchObject({
			value: m.feeRetry,
			warning: NET,
			tappable: true,
			// One coin: a tap asks again but opens no list, so no chevron.
			chevron: false,
			refreshLabel: m.feeRefresh
		});
	});

	it('every failure a retry can clear gets the sentence; the ones it cannot, none', () => {
		// The core's schedule (`requote_delay_ms`) decides — the sentence
		// promises the retry the timer makes.
		for (const failed of [
			'quote_unavailable',
			'fee_token_unavailable',
			'estimate_failed',
			'gas_quote_too_high'
		] as const) {
			expect(buildSigningModel(inputs({ fee: failedWith(failed) }))?.fee, failed).toMatchObject({
				warning: NET
			});
		}
		for (const failed of ['missing_public_key', 'calculation_failed'] as const) {
			const fee = buildSigningModel(inputs({ fee: failedWith(failed) }))?.fee;
			expect(fee, failed).toMatchObject({ value: m.feeRetry, tappable: true });
			expect(fee && 'warning' in fee ? fee.warning : undefined, failed).toBeUndefined();
		}
	});

	it('while the sheet asks again the reason is not said, but its line keeps its height (G47)', () => {
		const asking: FeeView = { ...QUOTED_FEE, fee: null, busy: true, confirm_fee_ready: false };
		const again = buildSigningModel(inputs({ fee: asking, feeFailing: 'quote_unavailable' }))?.fee;
		expect(again).toMatchObject({ value: m.feeEstimating, warningReserved: NET });
		expect(again && 'warning' in again ? again.warning : undefined).toBeUndefined();
		// A first measurement, with no failure behind it, says nothing yet.
		const first = buildSigningModel(inputs({ fee: asking, feeFailing: null }))?.fee;
		expect((first as { warning?: string } | undefined)?.warning).toBeUndefined();
		expect((first as { warningReserved?: string } | undefined)?.warningReserved).toBeUndefined();
		// The answer: a quote, and no sentence.
		const answered = buildSigningModel(inputs({ feeFailing: null }))?.fee;
		expect(answered && 'warning' in answered ? answered.warning : 'x').toBeUndefined();
	});

	it('a chain node, not Vela, is named when the chain read failed (RJ13, G48)', () => {
		const limited = buildSigningModel(
			inputs({ fee: failedWith({ chain_read: { rate_limited: true } }) })
		)?.fee;
		expect(limited).toMatchObject({
			warning: m.feeReasons['home.balanceDetailStatusRetrying']
		});
		const down = buildSigningModel(
			inputs({ fee: failedWith({ chain_read: { rate_limited: false } }) })
		)?.fee;
		const words = down && 'warning' in down ? (down.warning ?? '') : '';
		expect(words).toContain('Ethereum');
		expect(words).not.toContain('{{chain}}');
		expect(words).not.toBe(NET);
	});

	it('an old quote gets the send form’s calm note — not while a fresh one is out', () => {
		expect(buildSigningModel(inputs({ fee: { ...QUOTED_FEE, stale: true } }))?.fee).toMatchObject({
			staleNote: m.feeStale
		});
		const fresh = buildSigningModel(inputs())?.fee;
		expect(fresh && 'staleNote' in fresh ? fresh.staleNote : 'x').toBeUndefined();
	});

	it('a message has no fee row, so no refresh', () => {
		const message = {
			...OPEN_SIGN,
			request: { ...REQUEST, method: 'personal_sign', kind: 'personal_sign' as const }
		};
		expect(buildSigningModel(inputs({ sign: message }))?.fee).toEqual({
			kind: 'offchain',
			note: m.okNoNetworkFee
		});
	});
});

/** Spec 079 (F14): "127.0.0.1:8137" over "127.0.0.1:8137". */
describe('the header says the site once', () => {
	it('a site with no name of its own is named by its host, and the host line goes', () => {
		const model = buildSigningModel(inputs());
		expect(model?.dapp.name).toBe('app.example');
		expect(model?.dapp.host).toBe('');
	});

	it('a named site keeps its host under the name', () => {
		const named = {
			...OPEN_SIGN,
			request: {
				...REQUEST,
				dapp: { name: 'Example App', url: 'https://app.example' }
			}
		};
		const model = buildSigningModel(inputs({ sign: named }));
		expect(model?.dapp.name).toBe('Example App');
		expect(model?.dapp.host).toBe('app.example');
	});

	it('a port is part of the host, said once too', () => {
		const local = {
			...OPEN_SIGN,
			request: { ...REQUEST, origin: 'http://127.0.0.1:8137' }
		};
		const model = buildSigningModel(inputs({ sign: local }));
		expect(model?.dapp.name).toBe('127.0.0.1:8137');
		expect(model?.dapp.host).toBe('');
	});
});

describe('the sheet’s one close (spec 079)', () => {
	it('names its ✕ with the sheet’s own word', () => {
		expect(buildSigningModel(inputs())?.closeLabel).toBe(m.close);
	});
});

/**
 * Spec 079 (F11 — "可信签名器签完后，回到签名提示框，似乎没有任何提示"): from the
 * approval on, the sheet is a status — Android's signing receipt, word for
 * word — and never the form with a greyed slide.
 */
describe('after the approval the sheet is a status', () => {
	const SUMMARY = 'Send USDC · -100 USDC';
	const UNSIGNED = { signed: false, ceremonyUp: false };
	const PROMPT_UP = { signed: false, ceremonyUp: true };
	const SIGNED = { signed: true, ceremonyUp: false };
	const MESSAGE_REQUEST = { ...REQUEST, method: 'personal_sign', kind: 'personal_sign' as const };
	const at = (over: Partial<SignView>): SignView => ({ ...OPEN_SIGN, ...over });

	it('before the approval there is no status: the form, and the ✕ refuses (4001)', () => {
		expect(signingStatus(OPEN_SIGN, UNSIGNED, SUMMARY, m)).toBeNull();
		expect(buildSigningModel(inputs())?.status).toBeUndefined();
		expect(signingCloseEvent(null)).toBe('reject_tapped');
	});

	it('preparing — the precheck, the sponsor, the relay estimate — says so, never "Waiting for biometric" (RA9, G22)', () => {
		for (const progress of [UNSIGNED, PROMPT_UP]) {
			const status = signingStatus(
				at({ phase: 'preparing', is_signing: true }),
				progress,
				SUMMARY,
				m
			);
			expect(status).toEqual({
				stage: 'submitting',
				title: m.status.preparing,
				captions: [SUMMARY],
				closable: false
			});
			expect(status?.title).not.toBe(m.status.signing);
			expect(signingCloseEvent(status)).toBeNull();
		}
		expect(m.status.preparing).toBe('Preparing transaction...');
		expect(m.status.preparing).not.toBe(m.status.signing);
	});

	it('the passkey prompt is up: "Waiting for biometric…", and the ✕ shut', () => {
		const status = signingStatus(
			at({ phase: 'awaiting_signature', is_signing: true, is_submitting: true }),
			PROMPT_UP,
			SUMMARY,
			m
		);
		expect(status).toEqual({
			stage: 'submitting',
			title: m.status.signing,
			captions: [SUMMARY],
			closable: false
		});
		expect(signingCloseEvent(status)).toBeNull();
	});

	it('signed, going to the relay: "Submitting to network…" + closing keeps it running; the ✕ closes without refusing', () => {
		const status = signingStatus(
			at({ phase: 'submitting', is_signing: true, is_submitting: true }),
			SIGNED,
			SUMMARY,
			m
		);
		expect(status).toEqual({
			stage: 'submitting',
			title: m.status.submitting,
			captions: [SUMMARY, m.status.backgroundHint],
			closable: true
		});
		expect(signingCloseEvent(status)).toBe('dismiss_tapped');
	});

	it('every phase has its own words; idle is no status at all', () => {
		const titles = (['preparing', 'awaiting_signature', 'submitting'] as const).map(
			(phase) => signingStatus(at({ phase, is_signing: true }), UNSIGNED, SUMMARY, m)?.title
		);
		expect(titles).toEqual([m.status.preparing, m.status.signing, m.status.submitting]);
		expect(new Set(titles).size).toBe(3);
		expect(signingStatus(at({ phase: 'idle', is_signing: true }), SIGNED, SUMMARY, m)).toBeNull();
	});

	it('a second passkey prompt in the same approval shuts the ✕ again', () => {
		const status = signingStatus(
			at({ phase: 'submitting', is_signing: true, is_submitting: true }),
			{ signed: true, ceremonyUp: true },
			SUMMARY,
			m
		);
		expect(status).toMatchObject({ title: m.status.submitting, closable: false });
		expect(signingCloseEvent(status)).toBeNull();
	});

	it('a message never "submits": "Signing…" throughout', () => {
		const message = (flags: Partial<SignView>) => at({ request: MESSAGE_REQUEST, ...flags });
		expect(signingStatus(message({ phase: 'awaiting_signature' }), UNSIGNED, undefined, m)).toEqual(
			{
				stage: 'submitting',
				title: m.status.messageSigning,
				captions: [],
				closable: false
			}
		);
		expect(signingStatus(message({ phase: 'submitting' }), SIGNED, undefined, m)).toMatchObject({
			title: m.status.messageSigning,
			closable: true
		});
	});

	it('a failed submission says so, with the funds-are-safe line; the ✕ just closes', () => {
		const failed = signingStatus(
			at({ error: { kind: 'submit_failed', detail: 'relay said no' } }),
			UNSIGNED,
			SUMMARY,
			m
		);
		expect(failed).toEqual({
			stage: 'failed',
			title: m.receipt.failed,
			captions: [SUMMARY, m.status.failedHint],
			closable: true,
			// Spec 096 F8: a labelled close — the page hears the failure then.
			actions: { close: m.receipt.done }
		});
		expect(signingCloseEvent(failed)).toBe('dismiss_tapped');
	});

	it('a failure that sent nothing offers Try again beside the close (spec 096 F8)', () => {
		const retryable = signingStatus(
			at({
				error: { kind: 'submit_failed', detail: 'Could not estimate gas' },
				failure_retryable: true
			}),
			UNSIGNED,
			SUMMARY,
			m
		);
		expect(retryable?.actions).toEqual({ close: m.receipt.done, retry: m.status.retry });
		expect(retryable?.captions).toEqual([SUMMARY, m.status.failedHint]);
		// A refusal says so and offers no retry: the same op is refused again.
		const refused = signingStatus(
			at({
				error: { kind: 'submit_failed', detail: 'AA23' },
				failure_refused: true,
				failure_retryable: false
			}),
			UNSIGNED,
			SUMMARY,
			m
		);
		expect(refused?.actions).toEqual({ close: m.receipt.done });
		expect(refused?.captions).toEqual([SUMMARY, m.receipt.refused]);
	});

	it('an error before any approval is not a status (the form says it)', () => {
		expect(
			signingStatus(at({ error: { kind: 'user_rejected', detail: null } }), UNSIGNED, SUMMARY, m)
		).toBeNull();
		expect(
			signingStatus(at({ error: { kind: 'stale_fee_quote', detail: null } }), UNSIGNED, SUMMARY, m)
		).toBeNull();
	});

	it('the sheet carries the status, with the request in one line, and no refused request gets one', () => {
		const model = buildSigningModel(
			inputs({
				sign: at({ phase: 'submitting', is_signing: true, is_submitting: true }),
				progress: SIGNED
			})
		);
		expect(model?.status).toMatchObject({
			title: m.status.submitting,
			captions: ['Send USDC · \u2212100 USDC', m.status.backgroundHint]
		});
		const blocked = buildSigningModel(
			inputs({
				sign: at({
					phase: 'submitting',
					is_signing: true,
					blocked: {
						function: 'enableModule',
						selector: '0x610b5925',
						leg_index: null,
						nested: false
					}
				}),
				progress: SIGNED
			})
		);
		expect(blocked?.status).toBeUndefined();
	});

	it('summaryOf: what it is and its figure, or a swap’s two sides', () => {
		expect(summaryOf([{ kind: 'intent', text: 'Swap', tone: 'neutral' }])).toBe('Swap');
		expect(
			summaryOf([
				{ kind: 'intent', text: 'Swap', tone: 'neutral' },
				{
					kind: 'swap',
					pay: { sign: '-', value: '1', symbol: 'ETH', tone: 'neutral' },
					receive: { sign: '+', value: '3000', symbol: 'USDC', tone: 'success' }
				}
			])
		).toBe('Swap · 1 ETH → 3000 USDC');
		expect(summaryOf([])).toBeUndefined();
	});
});

/**
 * Spec 069: the dApp sheet chooses a speed exactly as the send form does —
 * the same core machine, the same builder, the same words.
 */
describe('the speed under the fee', () => {
	/** The real `fee_speed` core, told the fee in force and nothing beside it. */
	function speedView(preferred: FeeTier, pick?: FeeTier, open = false): FeeSpeedView {
		const core = new FeeSpeedCore();
		let view = JSON.parse(core.view()) as FeeSpeedView;
		const send = (event: FeeSpeedEvent) => {
			view = (JSON.parse(core.dispatch(JSON.stringify(event))) as { view: FeeSpeedView }).view;
		};
		send({ type: 'configure', preferred, number: 'comma_dot' });
		if (pick) send({ type: 'pick', tier: pick });
		if (open) send({ type: 'toggle' });
		send({
			type: 'quotes_changed',
			chain_id: 1,
			in_force: { busy: false, fee: QUOTED_FEE.fee },
			previews: []
		});
		core.free();
		return view;
	}

	it('is drawn folded under the fee, naming the tier in force', () => {
		const model = buildSigningModel(
			inputs({ speed: { view: speedView('fast'), feeOptions: () => [] } })
		);
		const fee = model?.fee;
		expect(fee?.kind).toBe('onchain');
		if (fee?.kind !== 'onchain') return;
		expect(fee.speed?.label).toBe(m.speed.label);
		expect(fee.speed?.value).toBe(m.speed.names.fast);
		expect(fee.speed?.open).toBe(false);
	});

	it('opens onto three speeds, each with what it buys', () => {
		const model = buildSigningModel(
			inputs({ speed: { view: speedView('fast', undefined, true), feeOptions: () => [] } })
		);
		const fee = model?.fee;
		if (fee?.kind !== 'onchain') throw new Error('an on-chain fee');
		expect(fee.speed?.options.map((option) => option.id)).toEqual(['fast', 'standard', 'slow']);
		expect(fee.speed?.options[2].detail).toBe(m.speed.hints.slow);
		// The option in force is this sheet's own fee.
		expect(fee.speed?.options[0].value).toBe('0.0021 ETH');
	});

	it('never shows the previous speed’s money under a newly picked one (issue 681)', () => {
		const model = buildSigningModel(
			inputs({ speed: { view: speedView('fast', 'slow'), feeOptions: () => [] } })
		);
		const fee = model?.fee;
		if (fee?.kind !== 'onchain') throw new Error('an on-chain fee');
		expect(fee.value).toBe(m.feeEstimating);
		expect(fee.speed?.value).toBe(m.speed.names.slow);
		// …and the slide stays shut: the core's gate is still open on the old
		// figure, which is exactly the speed the person just walked away from.
		expect(model?.confirm.enabled).toBe(false);
	});

	it('opens the slide once the fee in hand is the tier in force', () => {
		const model = buildSigningModel(
			inputs({ speed: { view: speedView('fast'), feeOptions: () => [] } })
		);
		expect(model?.confirm.enabled).toBe(true);
	});
});

describe('when there is nothing to sign', () => {
	it('builds no sheet at all — not an empty one', () => {
		expect(buildSigningModel(inputs({ sign: INITIAL_SIGN_VIEW }))).toBeNull();
		expect(buildSigningModel(inputs({ sign: { ...OPEN_SIGN, request: null } }))).toBeNull();
	});
});

describe('a decoded request', () => {
	it('leads with what it does, then the amount, then who receives it', () => {
		const model = buildSigningModel(inputs())!;
		expect(model.blocks[0]).toMatchObject({ kind: 'intent', text: 'Send USDC' });
		expect(model.blocks[1]).toMatchObject({ kind: 'amount' });
		expect(model.blocks.find((b) => b.kind === 'party')).toMatchObject({ name: 'alice.eth' });
		// Named by its host, said once (spec 079 F14).
		expect(model.dapp.name).toBe('app.example');
		expect(model.dapp.host).toBe('');
		expect(model.network.name).toBe('Ethereum');
	});

	it('carries the core’s risk grade into the intent’s tone, never a guess', () => {
		const danger = buildSigningModel(
			inputs({ clear: { ...DECODED, result: { ...DECODED.result!, risk: 'danger' } } })
		)!;
		expect(danger.blocks[0]).toMatchObject({ tone: 'danger' });
	});

	it('says every flag the core raised: a burn, a best-effort decode', () => {
		const flagged = buildSigningModel(
			inputs({
				clear: {
					...DECODED,
					result: {
						...DECODED.result!,
						to_own_token: true,
						verified: false,
						provenance: 'selector_db',
						best_effort: true
					}
				}
			})
		)!;
		const warnings = flagged.blocks.filter((b) => b.kind === 'warning');
		expect(warnings).toHaveLength(2);
		expect(warnings[0]).toMatchObject({ tone: 'danger' });
		// …and says them in words, not in template slots: "Calling {{fn}} —"
		// shipped once (spec 062 found it on the registry backup).
		for (const warning of warnings) {
			expect(JSON.stringify(warning)).not.toContain('{{');
		}
	});

	/*
	 * Spec 081 FR-008. The old rule was `!verified` → "selector not listed",
	 * which named the wrong problem for three of the five sources: the
	 * descriptor service answered, the selector WAS listed, and nobody
	 * authenticated the answer. Only a fetched descriptor gets a line, and it
	 * is a line about authentication.
	 */
	it('says where a fetched description came from, and nothing about the other sources', () => {
		const said = (provenance: ClearProvenance) =>
			buildSigningModel(
				inputs({
					m: { ...m, warnDescriptorFetched: 'Not authenticated.' },
					clear: {
						...DECODED,
						result: { ...DECODED.result!, verified: false, provenance }
					}
				})
			)!.blocks.filter((b) => b.kind === 'warning');

		expect(said('fetched')).toMatchObject([{ tone: 'caution', text: 'Not authenticated.' }]);
		expect(said('standard')).toHaveLength(0);
		expect(said('pinned_match')).toHaveLength(0);
		expect(said('none')).toHaveLength(0);
	});

	it("the wallet's own registry backup is drawn verified, with rows and no warning", () => {
		const backup = buildSigningModel(
			inputs({
				clear: {
					...DECODED,
					result: {
						...DECODED.result!,
						intent: 'Back up public keys',
						contract_name: 'Vela passkey registry',
						owner: 'Vela',
						verified: true,
						best_effort: false,
						partial: false,
						risk: 'safe',
						fields: [
							{
								...DECODED.result!.fields[0],
								label: 'Wallet',
								value: 'Interleave',
								role: 'generic',
								format: 'raw',
								address: null,
								token_address: null
							},
							{
								...DECODED.result!.fields[0],
								label: 'Keys',
								value: '3',
								role: 'generic',
								format: 'raw',
								address: null,
								token_address: null
							}
						]
					}
				}
			})
		)!;
		expect(backup.blocks.filter((b) => b.kind === 'warning')).toEqual([]);
		expect(backup.blocks[0]).toMatchObject({ kind: 'intent', text: 'Back up public keys' });
		const rows = backup.blocks.find((b) => b.kind === 'rows');
		expect(rows && 'rows' in rows ? rows.rows.map((r) => [r.label, r.value]) : null).toEqual([
			['Wallet', 'Interleave'],
			['Keys', '3']
		]);
	});
});

describe('the confirm gate is an AND', () => {
	it('arms only when the core, the guard and the fee all agree', () => {
		expect(buildSigningModel(inputs())!.confirm.enabled).toBe(true);
		// The core has not opened its gate.
		expect(
			buildSigningModel(inputs({ sign: { ...OPEN_SIGN, confirm_gate_open: false } }))!.confirm
				.enabled
		).toBe(false);
		// The guard is still waiting for a cap.
		expect(
			buildSigningModel(inputs({ guard: { ...INITIAL_GUARD_VIEW, confirm_allowed: false } }))!
				.confirm.enabled
		).toBe(false);
		// The fee has not settled.
		expect(
			buildSigningModel(inputs({ fee: { ...QUOTED_FEE, confirm_fee_ready: false } }))!.confirm
				.enabled
		).toBe(false);
		// A signature is already in flight.
		expect(
			buildSigningModel(inputs({ sign: { ...OPEN_SIGN, is_signing: true } }))!.confirm.enabled
		).toBe(false);
	});

	it('an off-chain signature needs no fee to arm', () => {
		const model = buildSigningModel(
			inputs({
				sign: { ...OPEN_SIGN, request: { ...REQUEST, kind: 'personal_sign' } },
				fee: { ...QUOTED_FEE, fee: null, confirm_fee_ready: false }
			})
		)!;
		expect(model.fee.kind).toBe('offchain');
		expect(model.confirm.enabled).toBe(true);
	});
});

describe('an unlimited approval is kept as asked, and said', () => {
	// The core's opening state for approve(spender, MAX) since 2026-09-26: the
	// Requested chip, choice `unlimited`, the site's own bytes, consent reported.
	const unbounded: GuardView = {
		...INITIAL_GUARD_VIEW,
		surface: 'approval_editor',
		confirm_allowed: true,
		unlimited_consented: true,
		unlimited_warning: true,
		meta: { symbol: 'USDC', decimals: 6, verified: true, loading: false },
		editor: {
			mode: 'requested',
			custom_text: '',
			error: null,
			choice: { type: 'unlimited' },
			display_amount_raw: null,
			requested_finite: false,
			requested_unlimited: true,
			has_balance_cap: true,
			revoke_offered: true,
			balance_raw: '1000'
		}
	};

	it('opens on its own chip, in the danger tone, with the warning, and arms the slider', () => {
		const model = buildSigningModel(inputs({ guard: unbounded }))!;
		const allowance = model.blocks.find((b) => b.kind === 'allowance');
		expect(allowance).toBeDefined();
		if (allowance?.kind !== 'allowance') throw new Error('kind');
		expect(allowance.value).toBe(m.valueUnlimited);
		expect(allowance.valueTone).toBe('danger');
		expect(allowance.chips.find((c) => c.id === 'requested')?.state).toBe('selected');
		expect(model.blocks).toContainEqual({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
		// Permit2 bundles revert when the approve is re-encoded — so the site's
		// ask is signable as it stands.
		expect(model.confirm.enabled).toBe(true);
	});

	it('choosing a finite cap settles the tone and drops the warning', () => {
		const capped: GuardView = {
			...unbounded,
			unlimited_consented: false,
			unlimited_warning: false,
			editor: {
				...unbounded.editor!,
				mode: 'balance',
				choice: { type: 'amount', amount_raw: '1000' },
				display_amount_raw: '1000'
			}
		};
		const model = buildSigningModel(inputs({ guard: capped }))!;
		const allowance = model.blocks.find((b) => b.kind === 'allowance');
		if (allowance?.kind !== 'allowance') throw new Error('kind');
		expect(allowance.chips.find((c) => c.id === 'balance')?.state).toBe('selected');
		expect(allowance.chips.find((c) => c.id === 'requested')?.state).toBe('idle');
		// 1000 base units at 6 decimals, in tokens — never "1000".
		expect(allowance.value).toBe('0.001 USDC');
		expect(allowance.valueTone).toBe('neutral');
		expect(model.blocks.some((b) => b.kind === 'warning' && b.text === m.warnUnlimited)).toBe(
			false
		);
		expect(model.confirm.enabled).toBe(true);
	});

	it('a typed cap too large to be one reads as an invalid amount, not "unlimited is disabled"', () => {
		const huge: GuardView = {
			...unbounded,
			confirm_allowed: false,
			unlimited_consented: false,
			unlimited_warning: false,
			editor: {
				...unbounded.editor!,
				mode: 'custom',
				custom_text: '1' + '0'.repeat(60),
				error: 'unlimited_disabled',
				choice: null
			}
		};
		const model = buildSigningModel(inputs({ guard: huge }))!;
		const allowance = model.blocks.find((b) => b.kind === 'allowance');
		if (allowance?.kind !== 'allowance') throw new Error('kind');
		expect(allowance.custom?.error).toBe(m.invalidAmount);
		expect(model.confirm.enabled).toBe(false);
	});

	it('a balance cap nobody could read is offered as disabled, not as a lie', () => {
		const noBalance: GuardView = {
			...unbounded,
			editor: { ...unbounded.editor!, has_balance_cap: false }
		};
		const model = buildSigningModel(inputs({ guard: noBalance }))!;
		const allowance = model.blocks.find((b) => b.kind === 'allowance');
		if (allowance?.kind !== 'allowance') throw new Error('kind');
		expect(allowance.chips.find((c) => c.id === 'balance')?.state).toBe('disabled');
	});
});

describe('an off-chain permit says it cannot be capped, and warns when unlimited (spec 094 S8)', () => {
	const permit = (unlimited: boolean): GuardView => ({
		...INITIAL_GUARD_VIEW,
		surface: 'permit_sign',
		confirm_allowed: true,
		unlimited_warning: unlimited,
		meta: { symbol: 'USDC', decimals: 6, verified: true, loading: false },
		editor: null
	});

	it('an unlimited Permit2 gets the danger sentence and the can’t-cap line — no editor', () => {
		const model = buildSigningModel(inputs({ guard: permit(true) }))!;
		expect(model.blocks).toContainEqual({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
		expect(model.blocks).toContainEqual({
			kind: 'warning',
			tone: 'danger',
			text: m.warnPermitCantCap
		});
		expect(model.blocks.some((b) => b.kind === 'allowance')).toBe(false);
		expect(model.confirm.enabled).toBe(true);
	});

	it('a bounded permit only says it cannot be capped', () => {
		const model = buildSigningModel(inputs({ guard: permit(false) }))!;
		expect(model.blocks.some((b) => b.kind === 'warning' && b.text === m.warnUnlimited)).toBe(
			false
		);
		expect(model.blocks).toContainEqual({
			kind: 'warning',
			tone: 'danger',
			text: m.warnPermitCantCap
		});
	});
});

describe('a capped unlimited approval reads the cap, not the request', () => {
	const APPROVE: ClearSigningView = {
		...DECODED,
		result: {
			...DECODED.result!,
			intent: 'Approve',
			risk: 'danger',
			fields: [
				field({ value: 'Unlimited', format: 'tokenAmount', warning: true, usd_value: null }),
				field({ label: 'Spender', value: '0x1111', role: 'spender', address: '0x1111' })
			]
		}
	};
	const guard = (choice: GuardView['editor']): GuardView => ({
		...INITIAL_GUARD_VIEW,
		surface: 'approval_editor',
		detected: {
			kind: 'erc20_approve',
			token_address: '0xdd',
			spender: '0x1111',
			amount_raw: 'f',
			amount_bits: 256,
			is_unbounded: true,
			is_boolean_grant: false,
			is_reducing: false,
			editable: true,
			block_reason: null,
			deadline: null,
			locus: { type: 'calldata_word', word_index: 1 }
		},
		meta: { symbol: 'USDC', decimals: 6, verified: true, loading: false },
		editor: choice
	});
	const editor = {
		mode: 'balance' as const,
		custom_text: '',
		error: null,
		choice: { type: 'amount' as const, amount_raw: '250000000' },
		display_amount_raw: '250000000',
		requested_finite: false,
		requested_unlimited: true,
		has_balance_cap: true,
		revoke_offered: true,
		balance_raw: '250000000'
	};

	it('a chosen cap replaces "Unlimited" and the danger it carried', () => {
		const shown = cappedApproval(APPROVE, guard(editor)).result!;
		expect(shown.fields[0].value).toBe('250 USDC');
		expect(shown.fields[0].warning).toBe(false);
		expect(shown.fields[1].value).toBe('0x1111');
		expect(shown.risk).toBe('caution');
	});

	it("a batch's first leg, capped, is what its decode reads too", () => {
		const leg = guard(editor);
		const batch: GuardView = {
			...INITIAL_GUARD_VIEW,
			surface: 'batch',
			batch: {
				legs: [
					{
						to: '0xdd',
						approval: leg.detected,
						meta: leg.meta,
						editor,
						choice: editor.choice,
						needs_editor: true,
						needs_choice: false,
						grants_broad: false
					}
				],
				any_uncapped: false,
				any_to_own_token: false,
				all_settled: true
			}
		};
		expect(cappedApproval(APPROVE, batch).result!.fields[0].value).toBe('250 USDC');
	});

	it("a batch draws each unbounded leg's own cap card, tagged with its leg, and its spender", () => {
		const leg = guard(editor);
		const batch: GuardView = {
			...INITIAL_GUARD_VIEW,
			surface: 'batch',
			confirm_allowed: true,
			unlimited_warning: true,
			batch: {
				legs: [
					{
						to: '0xdd',
						approval: null,
						meta: leg.meta,
						editor: null,
						choice: null,
						needs_editor: false,
						needs_choice: false,
						grants_broad: false
					},
					{
						to: '0xdd',
						approval: leg.detected,
						meta: leg.meta,
						editor: {
							...editor,
							mode: 'requested',
							choice: { type: 'unlimited' },
							display_amount_raw: null
						},
						choice: { type: 'unlimited' },
						needs_editor: true,
						needs_choice: false,
						grants_broad: true
					}
				],
				any_uncapped: true,
				any_to_own_token: false,
				all_settled: true
			}
		};
		const model = buildSigningModel(inputs({ guard: batch }))!;
		const cards = model.blocks.filter((b) => b.kind === 'allowance');
		expect(cards).toHaveLength(1);
		const [card] = cards;
		if (card.kind !== 'allowance') throw new Error('kind');
		expect(card.leg).toBe(1);
		expect(card.label.startsWith('#2 ')).toBe(true);
		expect(card.value).toBe(m.valueUnlimited);
		expect(card.chips.find((c) => c.id === 'requested')?.state).toBe('selected');
		expect(
			model.blocks.some((b) => b.kind === 'party' && b.address === leg.detected!.spender)
		).toBe(true);
		expect(model.blocks).toContainEqual({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
	});

	it('increaseAllowance offers no revoke chip', () => {
		const model = buildSigningModel(
			inputs({ guard: guard({ ...editor, revoke_offered: false }) })
		)!;
		const allowance = model.blocks.find((b) => b.kind === 'allowance');
		if (allowance?.kind !== 'allowance') throw new Error('kind');
		expect(allowance.chips.find((c) => c.id === 'revoke')?.state).toBe('disabled');
	});

	it('kept as asked, the decode is the truth and is left alone', () => {
		const kept = guard({
			...editor,
			mode: 'requested',
			choice: { type: 'unlimited' },
			display_amount_raw: null
		});
		expect(cappedApproval(APPROVE, kept)).toBe(APPROVE);
	});
});

describe('the slide control says a phrase, never a template', () => {
	it('falls back to the generic word when the core names no intent', () => {
		// The control renders `hint · action`. Falling back to the TEMPLATE put
		// its own placeholder on screen — a person read "Slide to confirm · Slide
		// to confirm · {{action}}" the first time a real dApp request reached the
		// sheet (spec 027). Same class as 026's `{{bytes}}`.
		const model = buildSigningModel(inputs())!;
		expect(model.confirm.action).not.toContain('{{');
		expect(model.confirm.hint).not.toContain('{{');
	});
});

describe('the deeper rungs of the ladder', () => {
	it('a blind transaction says so, in danger tone, with no invented fields', () => {
		const model = buildSigningModel(
			inputs({ clear: { ...INITIAL_CLEAR_VIEW, resolved: true, surface: 'blind_transaction' } })
		)!;
		expect(model.blocks[0]).toMatchObject({ kind: 'intent', tone: 'danger' });
		expect(model.blocks[1]).toMatchObject({ kind: 'warning', tone: 'danger' });
		expect(model.blocks.some((b) => b.kind === 'rows')).toBe(false);
	});

	it('eth_sign is its own danger, not a generic blind warning', () => {
		const model = buildSigningModel(
			inputs({ clear: { ...INITIAL_CLEAR_VIEW, resolved: true, surface: 'eth_sign' } })
		)!;
		expect(model.blocks[0]).toMatchObject({ text: m.warnEthSign });
	});

	it('a message shows its text and flags a SIWE domain mismatch as danger', () => {
		const model = buildSigningModel(
			inputs({
				clear: {
					...INITIAL_CLEAR_VIEW,
					resolved: true,
					surface: 'message_sign',
					message: {
						payload: 'hello',
						is_hex: false,
						decoded_text: 'hello',
						binary_preview: null,
						non_printable: false,
						siwe: null,
						binding: 'mismatch',
						danger_class: 'siwe_phish'
					}
				}
			})
		)!;
		expect(model.blocks.find((b) => b.kind === 'code')).toMatchObject({ lines: ['hello'] });
		expect(model.blocks.find((b) => b.kind === 'warning')).toMatchObject({
			tone: 'danger',
			text: m.warnSiweMismatch
		});
	});

	it('while the core is still resolving, the sheet waits instead of guessing', () => {
		const model = buildSigningModel(
			inputs({ clear: { ...INITIAL_CLEAR_VIEW, resolving: true, surface: 'loading' } })
		)!;
		expect(model.blocks).toHaveLength(1);
		expect(model.blocks[0].kind).toBe('sentence');
	});
});

describe('siteIconUrls', () => {
	it('names where an https site keeps its icon, best first', async () => {
		const { siteIconUrls } = await import('./live');
		expect(siteIconUrls('https://app.uniswap.org')).toEqual([
			'https://app.uniswap.org/apple-touch-icon.png',
			'https://app.uniswap.org/favicon.ico'
		]);
	});

	it('asks nothing of plain http, or of something that is not an origin', async () => {
		const { siteIconUrls } = await import('./live');
		expect(siteIconUrls('http://app.uniswap.org')).toEqual([]);
		expect(siteIconUrls('not an origin')).toEqual([]);
	});
});

describe('the fee coin can be switched, as it can when sending', () => {
	const option = (over: Partial<FeeView['options'][number]>): FeeView['options'][number] => ({
		symbol: 'ETH',
		contract: null,
		decimals: 18,
		balance: '1500000000000000000',
		recipient: '0x' + '11'.repeat(20),
		usd_balance: '4500',
		usd_price: '3000',
		amount: '2100000000000000',
		insufficient: false,
		selected: true,
		spent_by_operation: false,
		...over
	});
	const two: FeeView = {
		...QUOTED_FEE,
		options: [
			option({}),
			option({
				symbol: 'USDC',
				contract: '0x' + 'a0'.repeat(20),
				decimals: 6,
				balance: '42000000',
				amount: '1270000',
				selected: false
			}),
			option({
				symbol: 'DAI',
				contract: '0x' + '6b'.repeat(20),
				balance: '100000000000000',
				amount: '1270000000000000000',
				insufficient: true,
				selected: false,
				spent_by_operation: false
			})
		]
	};
	const feeOf = (over: Partial<SigningLiveInputs>) => buildSigningModel(inputs(over))!.fee;

	it('closed, the row is the row it always was', () => {
		const fee = feeOf({ fee: two });
		expect(fee.kind === 'onchain' && fee.selector).toBeUndefined();
	});

	it("open, it lists every coin the relay takes — amounts in each coin, the core's verdict on each", () => {
		const fee = feeOf({ fee: two, feeOpen: true });
		if (fee.kind !== 'onchain' || !fee.selector) throw new Error('no selector');
		expect(
			fee.selector.options.map((o) => [o.id, o.name, o.fee, o.selected, o.insufficient])
		).toEqual([
			['native', 'ETH', '~0.0021 ETH', true, false],
			['0x' + 'a0'.repeat(20), 'USDC', '~1.27 USDC', false, false],
			// Drawn, and not pickable: hiding it would be a second filter beside the core's.
			['0x' + '6b'.repeat(20), 'DAI', '~1.27 DAI', false, true]
		]);
		expect(fee.selector.options[1].balance).toBe('42 USDC');
	});

	it('with one coin there is nothing to choose, so nothing opens', () => {
		const fee = feeOf({ fee: { ...two, options: [option({})] }, feeOpen: true });
		expect(fee.kind === 'onchain' && fee.selector).toBeUndefined();
	});

	// Spec 096 F2: the person chose a coin the transaction itself spends (the
	// PancakeSwap USDC swap, fee in USDC). The core flags it; the sheet says
	// so under the fee — and only while that coin is the one paying.
	it('warns when the coin paying is one the transaction spends', () => {
		const spentUsdc: FeeView = {
			...two,
			options: [
				option({ selected: false }),
				option({
					symbol: 'USDC',
					contract: '0x' + 'a0'.repeat(20),
					decimals: 6,
					balance: '42000000',
					amount: '1270000',
					selected: true,
					spent_by_operation: true
				})
			]
		};
		const fee = feeOf({ fee: spentUsdc });
		expect(fee).toMatchObject({
			warning: m.feeCoinSpent.replace('{{sym}}', 'USDC')
		});
		const open = feeOf({ fee: spentUsdc, feeOpen: true });
		expect(open).toMatchObject({ warning: m.feeCoinSpent.replace('{{sym}}', 'USDC') });
		// The same coin listed but not paying: nothing to say.
		const inEth: FeeView = {
			...spentUsdc,
			options: [option({}), { ...spentUsdc.options[1], selected: false }]
		};
		expect(feeOf({ fee: inEth })).not.toHaveProperty('warning', expect.anything());
	});
});

describe('the blind line names the real length', () => {
	it("a batch counts its first leg's calldata, as the other shells do", () => {
		const call = { to: '0xdd', data: '0x095ea7b3' + '00'.repeat(64) };
		expect(calldataBytes(JSON.stringify([call]))).toBe(68);
		expect(calldataBytes(JSON.stringify([{ version: '2.0.0', calls: [call, call] }]))).toBe(68);
		expect(calldataBytes('not json')).toBe(0);
	});
});

describe("localizedTerms — the core names the word, the sheet says it in the reader's language", () => {
	const zh = resolveSigningMessages('zh');
	const approve: ClearSigningView = {
		...DECODED,
		result: {
			...DECODED.result!,
			intent: 'Approve',
			intent_term: 'intentApprove',
			risk: 'danger',
			fields: [
				field({
					label: 'Amount',
					label_term: 'labelAmount',
					value: 'Unlimited',
					value_term: 'valueUnlimited',
					warning: true,
					format: 'tokenAmount'
				}),
				field({
					label: 'Spender',
					label_term: 'labelSpender',
					value: '0x1111…1111',
					role: 'spender'
				}),
				field({ label: 'Referral code', value: 'abc' })
			]
		},
		confirm: { type: 'confirm_intent', intent: 'Approve', intent_term: 'intentApprove' }
	};

	it('swaps every named word and leaves the unnamed ones as the descriptor wrote them', () => {
		const out = localizedTerms(approve, zh);
		expect(out.result?.intent).toBe(zh.terms.intentApprove);
		expect(out.result?.fields.map((f) => [f.label, f.value])).toEqual([
			[zh.terms.labelAmount, zh.terms.valueUnlimited],
			[zh.terms.labelSpender, '0x1111…1111'],
			['Referral code', 'abc']
		]);
		expect(out.confirm).toEqual({
			type: 'confirm_intent',
			intent: zh.terms.intentApprove,
			intent_term: 'intentApprove'
		});
		expect(zh.terms.intentApprove).not.toBe('Approve');
	});

	it('every term the core can name has a word in this locale', () => {
		for (const term of CLEAR_TERMS) expect(zh.terms[term], term).toBeTruthy();
	});
});

/**
 * Spec 082 G14 (RC1–RC6): a dApp's plain value transfer — no calldata — is a
 * SEND, whatever the recipient. It used to be the red "Blind signature" over
 * bytes that did not exist.
 */
describe('a plain send is a send (G14)', () => {
	const TO = '0x7687C0bC1dD2B9d7e9a5b1b4e1B0cBd8e0C3D141';
	const tx = (value: unknown) => ({
		...OPEN_SIGN,
		request: {
			...REQUEST,
			chain_id: 100,
			method: 'eth_sendTransaction',
			params_json: JSON.stringify([{ to: TO, value }])
		}
	});

	/** What the REAL clear-signing core says for one transaction. */
	function coreView(value: unknown): ClearSigningView {
		const core = new ClearSigningCore();
		try {
			const params = JSON.parse(tx(value).request.params_json)[0] as Record<string, unknown>;
			core.dispatch(
				JSON.stringify({
					type: 'resolve_transaction',
					to: params.to,
					data: null,
					value: params.value === undefined ? null : String(params.value),
					chain_id: 100,
					locale: toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' })
				})
			);
			return JSON.parse(core.view()) as ClearSigningView;
		} finally {
			core.free();
		}
	}

	it('−0.001 xDAI to the recipient: "Send", the amount card, the party, "Confirm send"', () => {
		const clear = coreView('0x38d7ea4c68000');
		expect(clear.surface).toBe('plain_send');
		const model = buildSigningModel(inputs({ sign: tx('0x38d7ea4c68000'), clear }))!;
		expect(model.blocks).toEqual([
			{ kind: 'intent', text: m.intentSend, tone: 'neutral' },
			{
				kind: 'amount',
				// U+2212, never an ASCII hyphen (spec 082 G19/RJ15).
				line: { sign: '\u2212', value: '0.001', symbol: 'xDAI', tone: 'neutral' }
			},
			{
				kind: 'party',
				label: m.labelRecipient,
				name: shortenAddress(clear.plain_send!.to),
				// The core's EIP-55 spelling of the recipient (RC1).
				address: clear.plain_send!.to
			}
		]);
		expect(clear.plain_send!.to.toLowerCase()).toBe(TO.toLowerCase());
		expect(model.blocks.some((b) => b.kind === 'intent' && b.text === m.intentBlind)).toBe(false);
		expect(model.confirm.action).toBe(m.confirmSend);
	});

	it('zero moves nothing: the card stays, no minus sign, a neutral Confirm (RC3)', () => {
		const clear = coreView('0x0');
		expect(clear.surface).toBe('plain_send');
		const model = buildSigningModel(inputs({ sign: tx('0x0'), clear }))!;
		const amount = model.blocks.find((b) => b.kind === 'amount');
		expect(amount).toMatchObject({ line: { sign: '', value: '0', symbol: 'xDAI' } });
		expect(model.confirm.action).toBe(m.confirmPlain);
	});

	it('a value the core cannot read as an amount is the blind card with no amount (RC4, RC6)', () => {
		const clear = coreView(1000);
		expect(clear.surface).toBe('blind_transaction');
		const model = buildSigningModel(inputs({ sign: tx(1000), clear }))!;
		expect(model.blocks.find((b) => b.kind === 'amount')).toBeUndefined();
		expect(model.blocks[0]).toMatchObject({ kind: 'intent', text: m.intentBlind });
	});
});

describe('the site is named by the core’s label (spec 082 RE7)', () => {
	it('a name that is its host, in any case, is said once', () => {
		const named = {
			...OPEN_SIGN,
			request: { ...REQUEST, dapp: { name: 'APP.EXAMPLE', url: 'https://app.example' } }
		};
		const model = buildSigningModel(inputs({ sign: named }));
		expect(model?.dapp.host).toBe('');
	});
});

/**
 * Spec 082 round 2 (T231): the endings' words, the amount's fiat, the
 * estimate's warning.
 */
describe('the words after a refusal, the fiat, and the estimate’s warning (spec 082)', () => {
	const failedSign = (refused: boolean): SignView => ({
		...OPEN_SIGN,
		phase: 'idle',
		error: {
			kind: 'submit_failed',
			detail: 'the network refused this transaction; nothing was sent'
		},
		failure_refused: refused
	});

	it('a refusal says the network refused it — no "try again" (RJ3, G36)', () => {
		const refused = signingStatus(failedSign(true), undefined, 'Send · −1 USDC', m);
		expect(refused).toMatchObject({ stage: 'failed', title: m.receipt.failed });
		expect(refused?.captions).toEqual(['Send · −1 USDC', m.receipt.refused]);
		expect(refused?.captions).not.toContain(m.status.failedHint);
		// Not sent for any other reason: the calm "funds are safe" line, as before.
		const notSent = signingStatus(failedSign(false), undefined, undefined, m);
		expect(notSent?.captions).toEqual([m.status.failedHint]);
	});

	it('the fiat is the wallet’s money line — no exponent, no hard-coded $ (G60)', () => {
		const huge: ClearSigningView = {
			...DECODED,
			result: { ...DECODED.result!, fields: [field({ value: '1e30 USDC', usd_value: 1e24 })] }
		};
		const line = buildSigningModel(inputs({ clear: huge }))!.blocks.find(
			(b) => b.kind === 'amount'
		);
		const fiat = line?.kind === 'amount' ? (line.line.fiat ?? '') : '';
		expect(fiat).not.toMatch(/e\+/);
		expect(fiat).toContain('1,000,000,000,000,000,000,000,000');
		// The display currency, converted — not "$" over a euro figure.
		const eur = buildSigningModel(
			inputs({ currency: { code: 'EUR', rate: 0.5, committed: true } })
		)!.blocks.find((b) => b.kind === 'amount');
		const euros = eur?.kind === 'amount' ? (eur.line.fiat ?? '') : '';
		expect(euros).not.toContain('$');
		expect(euros).toContain('50');
	});

	it('an outgoing amount wears U+2212', () => {
		const line = buildSigningModel(inputs())!.blocks.find((b) => b.kind === 'amount');
		expect(line?.kind === 'amount' ? line.line.sign : '').toBe('−');
	});

	it('the relay’s estimate says it reverts → the danger line under the intent; the slide stays live (RJ19, G57)', () => {
		recordEstimateReverts(
			REQUEST.chain_id,
			identity.address,
			'ERC20: transfer amount exceeds balance'
		);
		try {
			const model = buildSigningModel(inputs())!;
			expect(model.blocks[0]).toMatchObject({ kind: 'intent' });
			expect(model.blocks[1]).toEqual({
				kind: 'warning',
				tone: 'danger',
				text: fill(m.warnWillFailReason, { reason: 'ERC20: transfer amount exceeds balance' })
			});
			// A warning informs, never blocks (L-D5).
			expect(model.confirm.enabled).toBe(true);
			// No reason given: the plain sentence.
			recordEstimateReverts(REQUEST.chain_id, identity.address, null);
			expect(buildSigningModel(inputs())!.blocks[1]).toMatchObject({ text: m.warnWillFail });
		} finally {
			clearEstimateReverts(REQUEST.chain_id, identity.address);
		}
		// Another question about the same account: no stale warning.
		expect(
			buildSigningModel(inputs())!.blocks.some(
				(b) => b.kind === 'warning' && b.text === m.warnWillFail
			)
		).toBe(false);
	});
});

/**
 * 089 S1 — what you see is what you sign, for a batch. `[1 wei → A, 1 xDAI →
 * B]` read "Send 0.000…1 xDAI to A"; the 1 xDAI to B was only in the raw
 * JSON. The sheet the REAL core and the REAL kickoff build must list every
 * call, and its headline must not be call 1's.
 */
describe('a batch shows every call (089 S1)', () => {
	const A = '0x7687C0bC1dD2B9d7e9a5b1b4e1B0cBd8e0C3D141';
	const B = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';
	const batchSign = (calls: unknown[]): SignView => ({
		...OPEN_SIGN,
		request: {
			...REQUEST,
			chain_id: 100,
			method: 'wallet_sendCalls',
			kind: 'batch',
			params_json: JSON.stringify([
				{ version: '2.0.0', chainId: '0x64', from: identity.address, calls }
			])
		}
	});

	/** The sheet's own kickoff, answered by the real core. */
	function coreView(sign: SignView): ClearSigningView {
		const core = new ClearSigningCore();
		try {
			const request = sign.request!;
			core.dispatch(
				JSON.stringify(
					txKickoff(
						request.method,
						request.params_json,
						request.chain_id,
						toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' })
					)
				)
			);
			return JSON.parse(core.view()) as ClearSigningView;
		} finally {
			core.free();
		}
	}

	const text = (blocks: unknown) => JSON.stringify(blocks);

	it('[1 wei → A, 1 xDAI → B]: both sends, both recipients, the total — and no "Send" headline', () => {
		const sign = batchSign([
			{ to: A, value: '0x1' },
			{ to: B, value: '0xde0b6b3a7640000' }
		]);
		const clear = coreView(sign);
		const model = buildSigningModel(inputs({ sign, clear }))!;
		const all = text(model.blocks).toLowerCase();
		expect(all).toContain(A.toLowerCase());
		expect(all).toContain(B.toLowerCase());
		expect(all).toContain('0.000000000000000001 xdai');
		expect(all).toContain('\u22121 xdai');
		// Every call its own card, in the order they run.
		const cards = model.blocks.filter((b) => b.kind === 'card');
		expect(cards).toEqual([
			{
				kind: 'card',
				title: fill(m.batchStep, { index: '1', action: m.intentSend }),
				tone: 'neutral',
				rows: [
					{ label: m.labelAmount, value: '\u22120.000000000000000001 xDAI' },
					{ label: m.labelRecipient, value: clear.batch!.calls[0].plain_send!.to, mono: true }
				]
			},
			{
				kind: 'card',
				title: fill(m.batchStep, { index: '2', action: m.intentSend }),
				tone: 'neutral',
				rows: [
					{ label: m.labelAmount, value: '\u22121 xDAI' },
					{ label: m.labelRecipient, value: clear.batch!.calls[1].plain_send!.to, mono: true }
				]
			}
		]);
		// And what the whole batch moves.
		expect(model.blocks).toContainEqual({
			kind: 'rows',
			rows: [{ label: m.labelTotal, value: '\u22121.000000000000000001 xDAI' }]
		});
		// The headline names the batch, not call 1.
		expect(model.blocks[0]).toEqual({ kind: 'intent', text: m.intentBatch, tone: 'neutral' });
		expect(model.blocks[1]).toEqual({
			kind: 'sentence',
			text: fill(m.summaryBatch, { count: '2' }),
			tone: 'accent'
		});
		expect(model.confirm.action).toBe(m.confirmPlain);
	});

	/** A batch view as the core hands it: [plain send, approve(MAX), unreadable]. */
	const APPROVE_FIELDS = [
		field({
			value: 'Unlimited',
			format: 'tokenAmount',
			warning: true,
			usd_value: null,
			label_term: 'labelAmount',
			value_term: 'valueUnlimited'
		}),
		field({
			label: 'Spender',
			value: '0x1111',
			role: 'spender',
			address: '0x1111',
			label_term: 'labelSpender'
		})
	];
	const BATCH: ClearSigningView = {
		...INITIAL_CLEAR_VIEW,
		resolved: true,
		surface: 'batch',
		batch: {
			calls: [
				{
					index: 1,
					surface: 'plain_send',
					result: null,
					plain_send: { to: A, value_wei: '1', amount: '0.000000000000000001', no_value: false },
					to: A,
					to_name: null,
					data_bytes: 0,
					value_wei: '1',
					amount: '0.000000000000000001',
					risk: 'normal'
				},
				{
					index: 2,
					surface: 'clear_sign',
					result: {
						...DECODED.result!,
						intent: 'Approve',
						intent_term: 'intentApprove',
						risk: 'danger',
						fields: APPROVE_FIELDS
					},
					plain_send: null,
					to: '0x' + 'cc'.repeat(20),
					to_name: null,
					data_bytes: 68,
					value_wei: '0',
					amount: '0',
					risk: 'danger'
				},
				{
					index: 3,
					surface: 'blind_transaction',
					result: null,
					plain_send: null,
					to: B,
					to_name: null,
					data_bytes: 36,
					value_wei: '10000000000000000',
					amount: '0.01',
					risk: 'caution'
				}
			],
			total_value_wei: '10000000000000001',
			total_amount: '0.010000000000000001',
			risk: 'danger'
		}
	};
	const unlimitedLeg = (choice: GuardView['editor']): GuardView => ({
		...INITIAL_GUARD_VIEW,
		surface: 'batch',
		confirm_allowed: true,
		// The core's flag follows the leg's choice (`any_uncapped`, spec 094 S8).
		unlimited_warning: choice?.choice?.type === 'unlimited',
		batch: {
			legs: [0, 1, 2].map((index) => ({
				to: index === 2 ? B : A,
				approval:
					index === 1
						? {
								kind: 'erc20_approve' as const,
								token_address: '0xdd',
								spender: '0x1111',
								amount_raw: 'f',
								amount_bits: 256,
								is_unbounded: true,
								is_boolean_grant: false,
								is_reducing: false,
								editable: true,
								block_reason: null,
								deadline: null,
								locus: { type: 'calldata_word' as const, word_index: 1 }
							}
						: null,
				meta: { symbol: 'USDC', decimals: 6, verified: true, loading: false },
				editor: index === 1 ? choice : null,
				choice: index === 1 ? (choice?.choice ?? null) : null,
				needs_editor: index === 1,
				needs_choice: false,
				grants_broad: index === 1 && choice?.choice?.type === 'unlimited'
			})),
			any_uncapped: choice?.choice?.type === 'unlimited',
			any_to_own_token: false,
			all_settled: true
		}
	});
	const kept = {
		mode: 'requested' as const,
		custom_text: '',
		error: null,
		choice: { type: 'unlimited' as const },
		display_amount_raw: null,
		requested_finite: false,
		requested_unlimited: true,
		has_balance_cap: false,
		revoke_offered: true,
		balance_raw: null
	};
	const capped = {
		...kept,
		mode: 'custom' as const,
		custom_text: '250',
		choice: { type: 'amount' as const, amount_raw: '250000000' },
		display_amount_raw: '250000000'
	};
	const batchSignOf = () =>
		batchSign([
			{ to: A, value: '0x1' },
			{ to: '0x' + 'cc'.repeat(20), data: '0x095ea7b3' },
			{ to: B, data: '0xdeadbeef', value: '0x2386f26fc10000' }
		]);

	it('every call is a card: decoded, plain and unreadable — the unreadable one says so', () => {
		const model = buildSigningModel(
			inputs({ sign: batchSignOf(), clear: BATCH, guard: unlimitedLeg(kept) })
		)!;
		expect(model.blocks[0]).toEqual({ kind: 'intent', text: m.intentBatch, tone: 'danger' });
		const cards = model.blocks.filter((b) => b.kind === 'card');
		expect(cards.map((c) => (c.kind === 'card' ? c.title : ''))).toEqual([
			fill(m.batchStep, { index: '1', action: m.intentSend }),
			// The core's term, in the reader's words (`localizedTerms`).
			fill(m.batchStep, { index: '2', action: m.terms.intentApprove }),
			fill(m.batchStep, { index: '3', action: fill(m.warnBlindDecode, { bytes: '36' }) })
		]);
		const [, approve, blind] = cards;
		if (approve.kind !== 'card' || blind.kind !== 'card') throw new Error('kind');
		expect(approve.tone).toBe('danger');
		expect(approve.rows[0]).toMatchObject({ value: m.terms.valueUnlimited, valueTone: 'danger' });
		// …and whom the call goes to: inside a batch nothing else says it.
		expect(approve.rows.at(-1)).toEqual({
			label: m.labelInteracting,
			value: '0x' + 'cc'.repeat(20),
			mono: true
		});
		expect(blind.tone).toBe('caution');
		expect(blind.rows).toEqual([
			{ label: m.labelInteracting, value: B, mono: true },
			{ label: m.labelAmount, value: '\u22120.01 xDAI' }
		]);
		// The second call's unlimited approval keeps its cap card and its warning.
		expect(model.blocks.filter((b) => b.kind === 'allowance')).toHaveLength(1);
		expect(model.blocks).toContainEqual({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
	});

	it("a cap on the second call's approval is what ITS card reads, and the headline calms", () => {
		const model = buildSigningModel(
			inputs({ sign: batchSignOf(), clear: BATCH, guard: unlimitedLeg(capped) })
		)!;
		const approve = model.blocks.filter((b) => b.kind === 'card')[1];
		if (approve.kind !== 'card') throw new Error('kind');
		expect(approve.rows[0]).toMatchObject({ value: '250 USDC', valueTone: undefined });
		// What an approve is anyway once nothing is unlimited (the single rule).
		expect(approve.tone).toBe('caution');
		expect(model.blocks[0]).toMatchObject({ kind: 'intent', tone: 'caution' });
		expect(model.blocks.some((b) => b.kind === 'warning' && b.text === m.warnUnlimited)).toBe(
			false
		);
	});
});

/**
 * Spec 093: the approve copies two more answers, decides neither. The intent a
 * record keeps is `clear_signing`'s `record_intent` — "Send" for a plain send,
 * nothing at all when the core says nothing may be recorded — and the token is
 * the guard's view of it, verbatim.
 */
describe('what the approve carries (spec 093)', () => {
	function plainSendView(): ClearSigningView {
		const core = new ClearSigningCore();
		try {
			core.dispatch(
				JSON.stringify({
					type: 'resolve_transaction',
					to: '0x1111111111111111111111111111111111111111',
					data: null,
					value: '0x38d7ea4c68000',
					chain_id: 100,
					locale: toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' })
				})
			);
			return JSON.parse(core.view()) as ClearSigningView;
		} finally {
			core.free();
		}
	}

	it("the record's intent is the core's record_intent; the token is the guard's meta", () => {
		const clear = plainSendView();
		expect(clear.record_intent).toBe('Send');
		const meta = { symbol: 'USDC', decimals: 6, verified: true, loading: false };
		const guard: GuardView = {
			...INITIAL_GUARD_VIEW,
			meta,
			rewritten_params_json: '[{"to":"0x1"}]',
			unlimited_consented: true
		};
		const opts = approveOptsOf(QUOTED_FEE, clear, guard);
		expect(opts).toMatchObject({
			intent: 'Send',
			token_meta: meta,
			params_override_json: '[{"to":"0x1"}]',
			unlimited_approved: true,
			max_fee_per_gas: '1',
			quoted_fee: null
		});
		expect(opts.token_meta).toBe(guard.meta);
	});

	it('nothing recorded when the core says nothing may be — never a shell guess', () => {
		const opts = approveOptsOf(
			null,
			{ ...INITIAL_CLEAR_VIEW, record_intent: null },
			INITIAL_GUARD_VIEW
		);
		expect(opts.intent).toBeNull();
		expect(opts.max_fee_per_gas).toBeNull();
		// The guard's placeholder token is handed over as it is; whether an
		// unresolved token counts is the core's to say.
		expect(opts.token_meta).toEqual(INITIAL_GUARD_VIEW.meta);
	});
});

/**
 * Spec 096 (part B): what the core now says about a request, drawn — the
 * coin a lone call sends, the order whose terms are off chain, a known
 * contract's name, and a sheet that is still reading.
 */
describe('the readable part says what the call does (096)', () => {
	const BNB = { ...OPEN_SIGN, request: { ...REQUEST, chain_id: 56 } };

	it('still reading: "Loading…", never the cap prompt, and the slide stays shut (F7)', () => {
		const loading: ClearSigningView = {
			...INITIAL_CLEAR_VIEW,
			resolving: true,
			surface: 'loading'
		};
		const model = buildSigningModel(inputs({ clear: loading }))!;
		expect(model.blocks).toEqual([{ kind: 'sentence', text: m.loading, tone: 'neutral' }]);
		expect(model.blocks.some((b) => JSON.stringify(b).includes(m.chipCustom))).toBe(false);
		// Every other machine says yes: the gate, the guard, the fee.
		expect(OPEN_SIGN.confirm_gate_open && INITIAL_GUARD_VIEW.confirm_allowed).toBe(true);
		expect(model.confirm.enabled).toBe(false);
		// Read, the same sheet arms.
		expect(buildSigningModel(inputs())!.confirm.enabled).toBe(true);
	});

	it('a decoded call that sends coin says how much, as a batch call does (F4)', () => {
		const supply: ClearSigningView = {
			...DECODED,
			result: {
				...DECODED.result!,
				intent: 'Supply',
				fields: [
					field({
						label: 'On behalf of',
						value: '0x88cca0...266894',
						role: 'generic',
						address: '0x88'
					})
				]
			},
			native_value: { value_wei: '3000000000000000', amount: '0.003' }
		};
		const model = buildSigningModel(inputs({ sign: BNB, clear: supply }))!;
		expect(model.blocks[1]).toEqual({
			kind: 'amount',
			line: { sign: '\u2212', value: '0.003', symbol: 'BNB', tone: 'neutral' }
		});
		// Beside a decoded amount of its own, the coin is a row.
		const both = buildSigningModel(
			inputs({ sign: BNB, clear: { ...DECODED, native_value: supply.native_value } })
		)!;
		expect(both.blocks).toContainEqual({
			kind: 'rows',
			rows: [{ label: m.labelAmount, value: '\u22120.003 BNB' }]
		});
	});

	it('a call nobody could read still says the coin it sends (F4)', () => {
		const blind: ClearSigningView = {
			...INITIAL_CLEAR_VIEW,
			resolved: true,
			surface: 'blind_transaction',
			native_value: { value_wei: '3000000000000000', amount: '0.003' }
		};
		const model = buildSigningModel(inputs({ sign: BNB, clear: blind }))!;
		expect(model.blocks.slice(0, 2)).toEqual([
			{ kind: 'intent', text: m.intentBlind, tone: 'danger' },
			{ kind: 'amount', line: { sign: '\u2212', value: '0.003', symbol: 'BNB', tone: 'neutral' } }
		]);
	});

	it('an order whose terms are off chain says so, alone and in a batch (F5)', () => {
		const order: ClearSigningView = {
			...DECODED,
			result: { ...DECODED.result!, intent: 'Swap', terms_off_chain: true }
		};
		const warning = { kind: 'warning', tone: 'caution', text: m.warnOrderTerms };
		expect(m.warnOrderTerms).toBeTruthy();
		expect(buildSigningModel(inputs({ clear: order }))!.blocks).toContainEqual(warning);
		const batch: ClearSigningView = {
			...INITIAL_CLEAR_VIEW,
			resolved: true,
			surface: 'batch',
			batch: {
				calls: [
					{
						index: 1,
						surface: 'clear_sign',
						result: order.result,
						plain_send: null,
						to: '0x9008D19f58AAbD9eD0D60971565AA8510560ab41',
						to_name: 'CoW Protocol',
						data_bytes: 164,
						value_wei: '0',
						amount: '0',
						risk: 'caution'
					},
					{
						index: 2,
						surface: 'blind_transaction',
						result: null,
						plain_send: null,
						to: '0x7777777777777777777777777777777777777777',
						to_name: null,
						data_bytes: 4,
						value_wei: '0',
						amount: '0',
						risk: 'caution'
					}
				],
				total_value_wei: '0',
				total_amount: '0',
				risk: 'caution'
			}
		};
		const model = buildSigningModel(inputs({ clear: batch }))!;
		expect(model.blocks).toContainEqual(warning);
		const cards = model.blocks.filter((b) => b.kind === 'card');
		// A known contract by its name; any other by its full address.
		expect(JSON.stringify(cards[0])).toContain(
			JSON.stringify({ label: m.labelInteracting, value: 'CoW Protocol' })
		);
		expect(JSON.stringify(cards[1])).toContain(
			JSON.stringify({
				label: m.labelInteracting,
				value: '0x7777777777777777777777777777777777777777',
				mono: true
			})
		);
	});

	it('the new words resolve in every locale the corpus ships', () => {
		const zh = resolveSigningMessages('zh');
		expect(zh.loading).toBe('加载中...');
		expect(zh.terms.valueAll).toBe('全部');
		expect(zh.terms.labelOrder).toBe('订单');
		expect(zh.warnOrderTerms).toContain('订单');
	});
});

/**
 * Spec 097 part A: the pass's own requests, read by the REAL core and drawn —
 * so what the sheet says is what the core decided. The scripted shell answers
 * as the pass's did: the descriptor service serves the native order's
 * descriptor, the chain names WBNB and USDC (unless it is down).
 */
describe('the sheet never states the false or the unknown as certain (097)', () => {
	const FIXTURES = '../../rust/crates/vela-core/tests/fixtures/dapp097';
	const WALLET = '0x88cca0eedbf2c4426110bbfc998f048689266894';
	const BNB = { ...OPEN_SIGN, request: { ...REQUEST, chain_id: 56 } };
	const TOKENS: Record<string, string> = {
		'0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c': 'WBNB',
		'0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d': 'USDC'
	};
	const DESCRIPTORS: Record<string, string> = {
		'/erc7730/calldata/eip155-56/0xe12e0f117d23a5ccc57f8935cd8c4e80cd91ff01.json':
			'calldata-eip155-56-0xe12e0f117d23a5ccc57f8935cd8c4e80cd91ff01.json'
	};
	const fixture = (name: string) =>
		JSON.parse(readFileSync(`${FIXTURES}/${name}`, 'utf8')) as {
			method: string;
			params: unknown[];
		};
	const word = (n: number) => '0x' + n.toString(16).padStart(64, '0');
	const abiString = (text: string) => {
		const body = Buffer.from(text, 'utf8').toString('hex').padEnd(64, '0');
		return word(32) + word(text.length).slice(2) + body;
	};

	function answer(op: ClearOperation, chainDown: boolean): ClearShellResult {
		switch (op.type) {
			case 'now':
				return { type: 'clock', now_ms: 1_790_957_000_000 };
			case 'http_get': {
				const file = DESCRIPTORS[op.path];
				return {
					type: 'descriptor_fetched',
					path: op.path,
					json: file ? readFileSync(`${FIXTURES}/descriptors/${file}`, 'utf8') : null
				};
			}
			case 'rpc_eth_call': {
				const symbol = chainDown ? undefined : TOKENS[op.to.toLowerCase()];
				const result =
					symbol === undefined ? null : op.probe === 'decimals' ? word(18) : abiString(symbol);
				return {
					type: 'rpc_answer',
					probe: op.probe,
					chain_id: op.chain_id,
					to: op.to,
					result,
					rpc_error: result === null
				};
			}
			case 'selector_db_lookup':
				return { type: 'selector_candidates', sigs: [] };
			case 'timer':
				return { type: 'timed_out', token: op.token };
		}
	}

	/** What the real core reads one of the pass's requests as. */
	function read(name: string, chainDown = false): ClearSigningView {
		const request = fixture(name);
		const locale = toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' });
		const event: ClearSigningEvent =
			request.method === 'eth_signTypedData_v4'
				? {
						type: 'resolve_typed_data',
						typed_data_json: request.params[1] as string,
						chain_id: 56,
						locale
					}
				: request.method === 'wallet_sendCalls'
					? {
							type: 'resolve_batch',
							params_json: JSON.stringify(request.params),
							chain_id: 56,
							locale
						}
					: (() => {
							const tx = request.params[0] as Record<string, string | undefined>;
							return {
								type: 'resolve_transaction' as const,
								to: tx.to ?? null,
								data: tx.data ?? null,
								value: tx.value ?? null,
								chain_id: 56,
								locale
							};
						})();
		const core = new ClearSigningCore();
		try {
			type Out = { effects: { id: number; operation: ClearOperation }[] };
			let queue = (JSON.parse(core.dispatch(JSON.stringify(event))) as Out).effects;
			for (let turn = 0; turn < 300 && queue.length > 0; turn++) {
				const [effect, ...rest] = queue;
				const next = JSON.parse(
					core.resolve_effect(
						BigInt(effect.id),
						JSON.stringify(answer(effect.operation, chainDown))
					)
				) as Out;
				queue = [...rest, ...next.effects];
			}
			const view = JSON.parse(core.view()) as ClearSigningView;
			expect(view.resolving).toBe(false);
			return localizedTerms(view, m);
		} finally {
			core.free();
		}
	}

	it('a single swap says its received amount is a minimum (N3)', () => {
		for (const name of ['pancakeswap-bnb-swap.json', 'uniswap-bnb-swap.json']) {
			const model = buildSigningModel(inputs({ sign: BNB, clear: read(name) }))!;
			const swap = model.blocks.find((b) => b.kind === 'swap');
			expect(swap, name).toMatchObject({
				pay: { sign: '\u2212', value: '0.003 BNB' },
				receive: { sign: '+', caption: m.terms.labelYouReceiveMin }
			});
			// What is paid is exact: no caption on it.
			expect(swap?.kind === 'swap' && swap.pay.caption).toBeUndefined();
		}
	});

	it('a 1inch order names where its proceeds go, the coin that arrives, its minimum and its expiry (N6)', () => {
		const clear = read('oneinch-order.json');
		const model = buildSigningModel(inputs({ sign: BNB, clear }))!;
		expect(model.blocks).toContainEqual({
			kind: 'swap',
			pay: {
				sign: '\u2212',
				value: '1.16 USDC',
				symbol: '',
				fiat: '≈ $1.16',
				caption: undefined,
				tone: 'neutral'
			},
			receive: {
				sign: '+',
				value: '0.001430509396956033 BNB',
				symbol: '',
				fiat: undefined,
				caption: m.terms.labelYouReceiveMin,
				tone: 'success'
			}
		});
		// The zero receiver is the maker: the account itself, short and full.
		expect(model.blocks).toContainEqual({
			kind: 'party',
			label: m.terms.labelRecipient,
			name: '0x88cca0...266894',
			address: WALLET,
			badge: undefined
		});
		expect(JSON.stringify(model.blocks)).toContain(
			JSON.stringify({ label: m.terms.labelValidUntil, value: '2026-10-03, 00:42', mono: false })
		);
		expect(JSON.stringify(model.blocks)).not.toContain('0x00000000');
	});

	it('the native order reads its beneficiary as the account, and an unscaled amount as unknown (N1)', () => {
		const read_ = read('oneinch-native-order.json');
		const model = buildSigningModel(inputs({ sign: BNB, clear: read_ }))!;
		expect(model.blocks).toContainEqual({
			kind: 'party',
			label: 'Beneficiary',
			name: '0x88cca0...266894',
			address: WALLET,
			badge: undefined
		});
		expect(model.blocks.find((b) => b.kind === 'swap')).toMatchObject({
			receive: { value: '2.418146082462759045 USDC', tone: 'success' }
		});
		expect(model.blocks).not.toContainEqual({
			kind: 'warning',
			tone: 'caution',
			text: m.warnPartial
		});

		// The chain is down: no figure, a caution, and the reading says it is incomplete.
		const down = buildSigningModel(
			inputs({ sign: BNB, clear: read('oneinch-native-order.json', true) })
		)!;
		expect(down.blocks.find((b) => b.kind === 'swap')).toMatchObject({
			receive: { sign: '', value: '\u2014 0x8ac7...', tone: 'caution' }
		});
		expect(down.blocks).toContainEqual({ kind: 'warning', tone: 'caution', text: m.warnPartial });
		expect(down.blocks).toContainEqual({
			kind: 'warning',
			tone: 'caution',
			text: m.warnUnverifiedAmount
		});
		expect(m.warnPartial).not.toBe(m.warnBestEffort);
	});

	it('an Aave borrow is money coming in (N2)', () => {
		const model = buildSigningModel(inputs({ sign: BNB, clear: read('aave-borrow.json') }))!;
		expect(model.blocks[1]).toMatchObject({
			kind: 'amount',
			line: { sign: '+', value: '0.3 USDC', tone: 'success' }
		});
	});

	it('a batch call on a token names it, with its address beside a name the chain gave (N8)', () => {
		const model = buildSigningModel(
			inputs({ sign: BNB, clear: read('pancakeswap-usdc-batch.json') })
		)!;
		const cards = model.blocks.filter((b) => b.kind === 'card');
		expect(JSON.stringify(cards[0])).toContain(
			JSON.stringify({ label: m.labelInteracting, value: 'USDC (0x8ac76a...cd580d)' })
		);
	});
});
