/**
 * The signing sheet's builder (spec 026 T244).
 *
 * Two properties matter more than any layout question, and both are asserted
 * here: the confirm gate is an AND of three separate answers, and the
 * never-unlimited mandate reaches the screen as a DISABLED chip plus a shut
 * slider — not as a warning somebody can slide past.
 */
import { describe, expect, it } from 'vitest';
import type { ClearSignField } from '$lib/core/generated/ClearSignField';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { FeeSpeedEvent } from '$lib/core/generated/FeeSpeedEvent';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { FeeSpeedCore } from '$lib/core/client';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { SignView } from '$lib/core/generated/SignView';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import type { WalletIdentity } from '$lib/wallet/identity';
import { INITIAL_CLEAR_VIEW, INITIAL_GUARD_VIEW } from './core/sheet.svelte';
import { INITIAL_SIGN_VIEW } from './core/sign-resident.svelte';
import { buildSigningModel, type SigningLiveInputs } from './live';

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
		...over
	};
}

const DECODED: ClearSigningView = {
	...INITIAL_CLEAR_VIEW,
	resolved: true,
	surface: 'clear_sign',
	result: {
		intent: 'Send USDC',
		contract_name: 'USD Coin',
		owner: null,
		fields: [
			field(),
			field({ label: 'To', value: 'alice.eth', role: 'recipient', address: '0xab' })
		],
		risk: 'normal',
		contract_address: '0x' + 'cc'.repeat(20),
		verified: true,
		sign_type: 'transaction',
		partial: false,
		best_effort: false,
		to_own_token: false
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
					selected: true
				}
			]
		};
		const model = buildSigningModel(inputs({ fee: priced }));
		expect(model?.fee).toEqual({
			kind: 'onchain',
			label: m.feeLabel,
			value: '0.0021 ETH · ≈$6.30'
		});
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
		expect(model.dapp.host).toBe('app.example');
		expect(model.network.name).toBe('Ethereum');
	});

	it('carries the core’s risk grade into the intent’s tone, never a guess', () => {
		const danger = buildSigningModel(
			inputs({ clear: { ...DECODED, result: { ...DECODED.result!, risk: 'danger' } } })
		)!;
		expect(danger.blocks[0]).toMatchObject({ tone: 'danger' });
	});

	it('says every flag the core raised: a burn, an unverified selector, best effort', () => {
		const flagged = buildSigningModel(
			inputs({
				clear: {
					...DECODED,
					result: { ...DECODED.result!, to_own_token: true, verified: false, best_effort: true }
				}
			})
		)!;
		const warnings = flagged.blocks.filter((b) => b.kind === 'warning');
		expect(warnings).toHaveLength(3);
		expect(warnings[0]).toMatchObject({ tone: 'danger' });
		// …and says them in words, not in template slots: "Calling {{fn}} —"
		// shipped once (spec 062 found it on the registry backup).
		for (const warning of warnings) {
			expect(JSON.stringify(warning)).not.toContain('{{');
		}
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

describe('the never-unlimited mandate reaches the screen', () => {
	const unbounded: GuardView = {
		...INITIAL_GUARD_VIEW,
		surface: 'approval_editor',
		confirm_allowed: false,
		meta: { symbol: 'USDC', decimals: 6, verified: true, loading: false },
		editor: {
			mode: null,
			custom_text: '',
			error: null,
			choice: null,
			display_amount_raw: null,
			requested_finite: false,
			has_balance_cap: true,
			balance_raw: '1000'
		}
	};

	it('an unbounded request disables its own chip AND the slider', () => {
		const model = buildSigningModel(inputs({ guard: unbounded }))!;
		const allowance = model.blocks.find((b) => b.kind === 'allowance');
		expect(allowance).toBeDefined();
		if (allowance?.kind !== 'allowance') throw new Error('kind');
		expect(allowance.value).toBe(m.valueUnlimited);
		expect(allowance.valueTone).toBe('danger');
		expect(allowance.chips.find((c) => c.id === 'requested')?.state).toBe('disabled');
		// The gate: nothing can be signed until a finite cap is chosen.
		expect(model.confirm.enabled).toBe(false);
	});

	it('choosing a finite cap re-arms the slider', () => {
		const capped: GuardView = {
			...unbounded,
			confirm_allowed: true,
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
		expect(allowance.value).toBe('1000');
		expect(model.confirm.enabled).toBe(true);
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
				selected: false
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
});
