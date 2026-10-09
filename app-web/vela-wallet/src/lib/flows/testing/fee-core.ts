/**
 * TESTS ONLY — never imported by product code.
 *
 * The REAL `fee_policy` core, answered by hand the way an executor answers
 * it, to the fee views a surface must draw — so a test of the drawing reads
 * what the machine publishes, not a hand-built guess at it. The caller loads
 * the wasm (`$lib/i18n/wasm-init.server`).
 *
 * `would_fail` is reached the way the core's own suite reaches it
 * (`app_fee_policy.rs` `a_fee_that_would_fail_says_what_a_tap_does`): a
 * router call the relay refuses to simulate (`FeeGasOutcome::Refused`).
 */
import { FeePolicyCore } from '$lib/core/client';
import type { FeeView } from '$lib/core/generated/FeeView';

export const FEE_TEST_USDC = '0x' + 'cc'.repeat(20);
const ACCOUNT = '0x' + '11'.repeat(20);
const RECIPIENT = '0x' + 'fe'.repeat(20);

type Out = { view: FeeView; effects: { id: number; operation: { type: string } }[] };

const ROWS = [
	{
		recipient: RECIPIENT,
		asset: 'native',
		fee_token: null,
		balance: '1000000000000000000',
		decimals: 18,
		symbol: 'ETH',
		usd_balance: '3000',
		usd_price: '3000',
		native_usd_floor_price: null,
		minimum_amount: null
	},
	{
		recipient: RECIPIENT,
		asset: 'erc20',
		fee_token: FEE_TEST_USDC,
		balance: '100000000',
		decimals: 6,
		symbol: 'USDC',
		usd_balance: '100',
		usd_price: '1',
		native_usd_floor_price: null,
		minimum_amount: null
	}
];

/**
 * The fee view after the relay refused to simulate the operation with every
 * coin the run tried. `chosen`: the person picked USDC, so only it was tried
 * and ETH is still on offer — `tap: choose_coin`. Otherwise the machine chose
 * and tried both — `tap: nothing`.
 */
export function wouldFailView(chainId: number, chosen: boolean): FeeView {
	const core = new FeePolicyCore();
	try {
		let out = JSON.parse(
			core.dispatch(
				JSON.stringify({
					type: 'quote_requested',
					chain_id: chainId,
					account: ACCOUNT,
					deployed: true,
					public_key_available: true,
					tier: 'standard',
					// A router call: one the relay's simulation decides, never a
					// transfer priced from defaults.
					calls: [
						{ to: '0x' + '3f'.repeat(20), value: '0', data: '0x3593564c' + 'ab'.repeat(1200) }
					],
					fee_token: chosen ? FEE_TEST_USDC : null,
					auto_fee_token: !chosen,
					number: 'comma_dot'
				})
			)
		) as Out;
		const pending = [...out.effects];
		for (let step = 0; step < 64 && pending.length > 0; step += 1) {
			const effect = pending.shift()!;
			const answer = answerFor(effect.operation.type);
			// Timers stay out: nothing here waits on the clock.
			if (answer === null) continue;
			out = JSON.parse(core.resolve_effect(BigInt(effect.id), JSON.stringify(answer))) as Out;
			pending.push(...out.effects);
		}
		return out.view;
	} finally {
		core.free();
	}
}

function answerFor(type: string): unknown {
	switch (type) {
		case 'fetch_gas_price':
			return {
				type: 'gas_price',
				eth_gas_price: '1000000000',
				base_fee: '1000000000',
				priority_fee: '1000000000'
			};
		case 'fetch_bundler_quote':
			return {
				type: 'bundler_quote',
				quote: {
					max_fee_per_gas: '1000000000',
					max_priority_fee_per_gas: '1000000000',
					network_fee_per_gas: null,
					relayer_fee_per_gas: null,
					in_band_fee_per_gas: null
				}
			};
		case 'fetch_in_band_quotes':
			return { type: 'in_band_quotes', quotes: ROWS };
		case 'fetch_fee_recipient':
			return { type: 'fee_recipient', recipient: RECIPIENT };
		case 'estimate_user_op_gas':
			return { type: 'user_op_gas', outcome: { type: 'refused' } };
		case 'measure_inner_calls':
			return { type: 'inner_calls_measured', gas: [null] };
		default:
			return null;
	}
}
