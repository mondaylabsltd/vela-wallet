/**
 * cs-12's fixtures: a multisig's call, read by the REAL `clear_signing` and
 * `approval_guard` cores to rest, with what they would fetch answered here
 * (a plain ERC-20 of 6 decimals called USDC; nothing else answers). Callers
 * have the core loaded (`wasm-init.server` in node, `loadCore()` in a browser).
 */
import { ApprovalGuardCore, ClearSigningCore } from '$lib/core/client';
import type { ClearShellResult } from '$lib/core/generated/ClearShellResult';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { GuardShellResult } from '$lib/core/generated/GuardShellResult';
import type { GuardView } from '$lib/core/generated/GuardView';
import { clearOperationFailure } from '$lib/signing/core/clear-executor';
import { toClearLocale, type ClearEffect } from '$lib/signing/core/clear-types';
import { guardOperationFailure } from '$lib/signing/core/guard-executor';
import type { GuardEffect } from '$lib/signing/core/guard-types';
import { txKickoff } from '$lib/signing/core/tx-params';
import { batchParamsJson, callParamsJson, type InnerCall } from './call-review.svelte';

export const CHAIN = 100;
export const MULTISIG = '0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe';
export const TOKEN = '0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83';
export const SPENDER = '0x1111111254EEB25477B68fb85Ed929f73A960582';

/** approve(SPENDER, 2^256 − 1) on TOKEN. */
export const APPROVE_MAX: InnerCall = {
	to: TOKEN,
	value: '0',
	data: `0x095ea7b3${SPENDER.slice(2).toLowerCase().padStart(64, '0')}${'f'.repeat(64)}`
};

/** Eight bytes nobody can read, sending 1.5 of the chain's coin. */
export const BLIND: InnerCall = {
	to: SPENDER,
	value: '1500000000000000000',
	data: '0x12345678deadbeef'
};

type Core = {
	dispatch(event: string): string;
	resolve_effect(id: bigint, result: string): string;
	view(): string;
	free(): void;
};

/** Run `core` on `event`, answering every effect it asks with `answer`, to rest. */
function drive<View, Effect extends { id: number }, Result>(
	core: Core,
	event: unknown,
	answer: (effect: Effect) => Result
): View {
	try {
		let out = JSON.parse(core.dispatch(JSON.stringify(event))) as { effects: Effect[] };
		const queue = [...out.effects];
		while (queue.length > 0) {
			const effect = queue.shift()!;
			out = JSON.parse(core.resolve_effect(BigInt(effect.id), JSON.stringify(answer(effect)))) as {
				effects: Effect[];
			};
			queue.push(...out.effects);
		}
		return JSON.parse(core.view()) as View;
	} finally {
		core.free();
	}
}

/** What both machines say about `call` (or a batch of calls) from MULTISIG on CHAIN. */
export function read(calls: InnerCall | InnerCall[]) {
	const list = Array.isArray(calls) ? calls : [calls];
	const method = list.length > 1 ? 'wallet_sendCalls' : 'eth_sendTransaction';
	const paramsJson =
		list.length > 1 ? batchParamsJson(list, CHAIN, MULTISIG) : callParamsJson(list[0]);
	const clear = drive<ClearSigningView, ClearEffect, ClearShellResult>(
		new ClearSigningCore(),
		txKickoff(
			method,
			paramsJson,
			CHAIN,
			toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' })
		),
		(effect) => {
			const op = effect.operation;
			// A plain ERC-20: its ERC-165 probe reverts, its decimals say 6.
			if (op.type === 'rpc_eth_call') {
				const decimals = op.data.startsWith('0x313ce567');
				return {
					type: 'rpc_answer',
					probe: op.probe,
					chain_id: op.chain_id,
					to: op.to,
					result: decimals ? `0x${'6'.padStart(64, '0')}` : null,
					rpc_error: !decimals
				};
			}
			return clearOperationFailure(effect);
		}
	);
	const guard = drive<GuardView, GuardEffect, GuardShellResult>(
		new ApprovalGuardCore(),
		{
			type: 'approval_detected',
			method,
			params_json: paramsJson,
			chain_id: CHAIN,
			wallet_address: MULTISIG,
			read_only: true,
			now_ms: Date.now()
		},
		(effect) =>
			effect.operation.type === 'read_token_metadata'
				? {
						type: 'meta_resolved',
						metas: [{ token: TOKEN.toLowerCase(), symbol: 'USDC', decimals: 6 }]
					}
				: guardOperationFailure(effect)
	);
	return { chainId: CHAIN, multisig: MULTISIG, method, paramsJson, clear, guard };
}
