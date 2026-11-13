/**
 * r11-8 through the web's door: the shared vector
 * (`rust/crates/vela-core/tests/vectors/fee/plain-send.json`, also replayed by
 * vela-core's `app_fee_parity.rs` and every native shell). The relay's and the
 * chain's wire answers go through the web's own readers — they must hand the
 * fee machine exactly the vector's shell results — and the core this build
 * ships then prices the send exactly as the vector says, on every shell the
 * same: 0.12 USDC on the relay's limits, nothing when no simulation came back
 * (the round-11 Android 0.045), the static limits signed when the relay
 * answers the operation fails.
 */
import '$lib/i18n/wasm-init.server';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Answer = { result?: unknown; error?: { code: number; message: string } };
const answers = new Map<string, () => Answer>();

vi.mock('$lib/services/rpc-adapter', () => ({
	rpcCall: vi.fn(async (method: string) => {
		const answer = answers.get(method);
		if (!answer) throw new Error(`transport down: ${method}`);
		return { jsonrpc: '2.0', id: 1, ...answer() };
	})
}));

import { FeePolicyCore } from '$lib/core/client';
import type { FeeView } from '$lib/core/generated/FeeView';
import {
	_resetFeeSignalsCache,
	fetchRawBundlerQuote,
	fetchRawGasSignals,
	invalidateFeeSignals,
	simulateUserOpGas
} from '$lib/services/safe-transaction';

interface Case {
	name: string;
	estimate_wire: { result?: unknown; error?: { code: number; message: string } } | null;
	user_op_gas: Record<string, unknown>;
	expect: Record<string, unknown>;
}

const VECTOR = JSON.parse(
	readFileSync(
		fileURLToPath(
			new URL(
				'../../../../../../rust/crates/vela-core/tests/vectors/fee/plain-send.json',
				import.meta.url
			)
		),
		'utf8'
	)
) as {
	request: {
		chain_id: number;
		account: string;
		calls: { to: string; value: string; data: string }[];
	};
	gas_price: { wire: Record<string, string>; shell_result: Record<string, unknown> };
	bundler_quote: { wire: Record<string, unknown>; shell_result: Record<string, unknown> };
	in_band_quotes: { shell_result: Record<string, unknown> };
	cases: Case[];
};
const CHAIN = VECTOR.request.chain_id;

function chainAnswers(): void {
	const wire = VECTOR.gas_price.wire;
	answers.set('eth_gasPrice', () => ({ result: wire.eth_gasPrice }));
	answers.set('eth_getBlockByNumber', () => ({
		result: { baseFeePerGas: wire.latest_block_baseFeePerGas }
	}));
	answers.set('eth_maxPriorityFeePerGas', () => ({ result: wire.eth_maxPriorityFeePerGas }));
	answers.set('pimlico_getUserOperationGasPrice', () => ({ result: VECTOR.bundler_quote.wire }));
	// The EntryPoint's nonce for the deployed account (the dummy op's).
	answers.set('eth_call', () => ({ result: '0x' + '0'.repeat(63) + '8' }));
}

type Out = { view: FeeView; effects: { id: number; operation: { type: string } }[] };

/** The core this build ships, answered with the shell results the web produced. */
function priced(shell: Record<string, unknown>): FeeView {
	const core = new FeePolicyCore();
	try {
		let out = JSON.parse(core.dispatch(JSON.stringify(VECTOR.request))) as Out;
		const pending = [...out.effects];
		for (let step = 0; step < 32 && pending.length > 0; step += 1) {
			const effect = pending.shift()!;
			const answer = shell[effect.operation.type];
			if (answer === undefined) continue; // timers: nothing here waits on the clock
			out = JSON.parse(core.resolve_effect(BigInt(effect.id), JSON.stringify(answer))) as Out;
			pending.push(...out.effects);
		}
		return out.view;
	} finally {
		core.free();
	}
}

describe('r11-8: the same plain send, priced one way', () => {
	beforeEach(() => {
		answers.clear();
		_resetFeeSignalsCache();
		invalidateFeeSignals(CHAIN);
		chainAnswers();
	});
	afterEach(() => answers.clear());

	it("the web's readers hand the machine the vector's fee inputs", async () => {
		const signals = await fetchRawGasSignals(CHAIN, true, true);
		const gas = VECTOR.gas_price.shell_result;
		expect({
			eth_gas_price: signals.ethGasPrice,
			base_fee: signals.baseFee,
			priority_fee: signals.priorityFee
		}).toEqual({
			eth_gas_price: gas.eth_gas_price,
			base_fee: gas.base_fee,
			priority_fee: gas.priority_fee
		});
		const quote = await fetchRawBundlerQuote(CHAIN, 'fast');
		const want = (VECTOR.bundler_quote.shell_result as { quote: Record<string, unknown> }).quote;
		expect({
			max_fee_per_gas: quote?.maxFeePerGas,
			max_priority_fee_per_gas: quote?.maxPriorityFeePerGas,
			network_fee_per_gas: quote?.networkFeePerGas ?? null,
			relayer_fee_per_gas: quote?.relayerFeePerGas ?? null,
			in_band_fee_per_gas: quote?.inBandFeePerGas ?? null
		}).toEqual(want);
	});

	for (const c of VECTOR.cases) {
		it(c.name, async () => {
			if (c.estimate_wire === null) answers.delete('eth_estimateUserOperationGas');
			else answers.set('eth_estimateUserOperationGas', () => c.estimate_wire as Answer);
			const outcome = await simulateUserOpGas({
				chainId: CHAIN,
				account: VECTOR.request.account,
				deployed: true,
				calls: VECTOR.request.calls.map((call) => ({
					to: call.to,
					value: '0x' + BigInt(call.value).toString(16),
					data: call.data
				}))
			});
			// What the web hands the machine for this body (fee-executor's mapping).
			const userOpGas =
				outcome.kind === 'estimated'
					? {
							type: 'estimated',
							verification_gas_limit: outcome.verificationGasLimit.toString(),
							call_gas_limit: outcome.callGasLimit.toString(),
							pre_verification_gas: outcome.preVerificationGas.toString(),
							settlement_gas: outcome.settlementGas?.toString() ?? null
						}
					: { type: outcome.kind };
			expect(userOpGas).toEqual(c.user_op_gas);

			const view = priced({
				fetch_gas_price: VECTOR.gas_price.shell_result,
				fetch_bundler_quote: VECTOR.bundler_quote.shell_result,
				fetch_in_band_quotes: VECTOR.in_band_quotes.shell_result,
				estimate_user_op_gas: { type: 'user_op_gas', outcome: userOpGas }
			});
			expect(view.failed ?? null).toEqual(c.expect.failed);
			if (c.expect.fee === null) {
				expect(view.fee).toBeNull();
				return;
			}
			expect(view.fee?.total_gas).toBe(c.expect.total_gas);
			expect(view.fee?.in_band_gas_basis).toBe(c.expect.in_band_gas_basis);
			expect(view.fee?.total_wei).toBe(c.expect.total_wei);
			expect(view.fee?.prepared).toMatchObject(c.expect.prepared as object);
		});
	}
});
