// Ported from src/services/wallet-state-core/token-trust-executor.ts @ c13e89d4 (spec 025).
/**
 * The only place the `token_trust` core touches the outside world (spec 017,
 * group G7).
 *
 * Seven operations, one existing service call each — the RPC pool
 * (`eth_blockNumber` / `eth_getLogs` / `eth_getBlockByNumber`), the batched
 * Multicall3 metadata resolver, and the two `vela.customTokens` accessors plus
 * the fetchTokens cache invalidation. No branching on business meaning: the
 * allowlist, the one capped retry, the metadata gate, the admission filter and
 * the asymmetric sim judgment are all decided in Rust.
 *
 * Three contracts this file must honour, or the core stalls / mis-decides:
 *
 *  ① `eth_getLogs` failures are classified by wording ONCE, here, through the
 *    existing `getLogsRangeCap` — a range/result cap becomes
 *    `RangeCapped{cap}` (`cap = 0` when no number could be parsed; the core
 *    then conservatively uses 100), anything else becomes `Failed`. The core
 *    owns what to do with each; this file owns only the wording→axis mapping.
 *  ② `MulticallErc20Meta` answers EVERY requested address, resolved or not.
 *    An omitted address leaves the core's metadata gate permanently unmet and
 *    the scan chain (or the admission session) never finishes.
 *  ③ `BlockTimestamp` carries the block's own time or `null`, and NO clock
 *    (PR 3). A transfer's time is its block's time: this file used to hand
 *    the core `Date.now()` beside the answer, the core stamped a transfer
 *    whose block it could not read with it, and the store kept that for
 *    good — receipts eleven days old under "Today". A block that cannot be
 *    read is `timestamp_sec: null`; the core withholds its transfers and
 *    asks again on a later poll.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with.
 */

import { chainName } from '$lib/services/networks';
import { getLogsRangeCap, poolRpcCall } from '$lib/services/rpc-pool';
import { loadCustomTokens, saveCustomToken } from '$lib/services/records';
import { resolveTokenMetadata } from '$lib/services/token-metadata';
import { clearTokenCache } from '$lib/services/wallet-api';
import type { CustomToken } from '$lib/services/tokens-model';

import type { TrustCustomToken } from '$lib/core/generated/TrustCustomToken';
import type { TrustMetaEntry } from '$lib/core/generated/TrustMetaEntry';
import type { TrustRawLog } from '$lib/core/generated/TrustRawLog';
import type { TrustLogsOutcome } from '$lib/core/generated/TrustLogsOutcome';
import type { TrustShellResult } from '$lib/core/generated/TrustShellResult';
import type { TrustEffect } from './token-trust-types';

/**
 * keccak256("Transfer(address,address,uint256)") — transport vocabulary, not a
 * decision: the core pins the *recipient* topic and the contract allowlist and
 * leaves topic0 to the shell's filter, exactly as `transferLogsFilter`
 * (`transfer-monitor.ts:188-196`) builds it today. The core carries the same
 * constant for its local re-verification (`token_trust.rs TRANSFER_TOPIC`).
 */
const TRANSFER_TOPIC = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';

const asString = (value: unknown): string => (typeof value === 'string' ? value : '');

/** `parseInt(hex, 16)`, the TS `hexToNumber`; unusable input reads as null. */
function hexToNumber(hex: string): number | null {
	const n = parseInt(hex, 16);
	return Number.isFinite(n) ? n : null;
}

/**
 * One raw `eth_getLogs` entry, carried through untouched. A codec, not a
 * policy: whether a log means anything is the core's call (it re-verifies
 * `topics[2]` locally — invariant ①). Only the wire shape is normalised, since
 * serde would reject a record with a missing/mistyped field and a rejected
 * result would strand the scan chain.
 */
function toWireLog(raw: unknown): TrustRawLog {
	const record = (raw ?? {}) as Record<string, unknown>;
	const topics = Array.isArray(record.topics) ? (record.topics as unknown[]).map(asString) : [];
	return {
		address: asString(record.address),
		topics,
		data: asString(record.data),
		transaction_hash: asString(record.transactionHash),
		// `?? '0x0'` is applied in the core; `null` means "absent", verbatim.
		block_number: typeof record.blockNumber === 'string' ? record.blockNumber : null,
		log_index: typeof record.logIndex === 'string' ? record.logIndex : null
	};
}

/** A stored custom token as the core reads it (`networkName` is display vocabulary). */
function toWireToken(token: CustomToken): TrustCustomToken {
	return {
		id: token.id,
		chain_id: token.chainId,
		contract_address: token.contractAddress,
		symbol: token.symbol,
		name: token.name,
		decimals: token.decimals
	};
}

/**
 * The core's token in the shape `services/storage.ts` persists. `networkName`
 * is re-derived from the chain exactly where `token-autoadd.ts:63` derives it.
 */
function fromWireToken(token: TrustCustomToken): CustomToken {
	return {
		id: token.id,
		chainId: token.chain_id,
		contractAddress: token.contract_address,
		symbol: token.symbol,
		name: token.name,
		decimals: token.decimals,
		networkName: chainName(token.chain_id)
	};
}

/** One `eth_getLogs`, classified as the core reads it. */
async function logsOutcome(
	filter: Record<string, unknown>,
	chainId: number
): Promise<TrustLogsOutcome> {
	const res = await poolRpcCall('eth_getLogs', [filter], chainId);
	if (res?.error) {
		const cap = getLogsRangeCap(res.error);
		// ① the ONLY wording→axis mapping in the machine.
		return cap === null ? { type: 'failed' } : { type: 'range_capped', cap };
	}
	return {
		type: 'ok',
		logs: (Array.isArray(res?.result) ? res.result : []).map(toWireLog)
	};
}

export async function executeTokenTrustOperation(effect: TrustEffect): Promise<TrustShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'rpc_block_number': {
			const res = await poolRpcCall('eth_blockNumber', [], operation.chain_id);
			return {
				type: 'block_number',
				address: operation.address,
				chain_id: operation.chain_id,
				// `bn.error || typeof bn.result !== 'string'` → the chain yields
				// nothing this tick; the core makes that call, not this file.
				block_hex: !res?.error && typeof res?.result === 'string' ? res.result : null
			};
		}

		case 'rpc_get_logs': {
			const filter: Record<string, unknown> = {
				fromBlock: operation.from_block,
				toBlock: operation.to_block,
				topics: [TRANSFER_TOPIC, null, operation.recipient_topic]
			};
			// `if (contracts && contracts.length)` — an empty allowlist means "no
			// address restriction" in the JSON-RPC filter, so it is omitted rather
			// than sent as `[]` (which every endpoint reads as "match nothing").
			if (operation.contracts.length > 0) filter.address = operation.contracts;
			return {
				type: 'logs',
				address: operation.address,
				chain_id: operation.chain_id,
				outcome: await logsOutcome(filter, operation.chain_id)
			};
		}

		// Issue #443: the logs the wallet ITSELF emitted with one topic — the
		// Safe's `SafeReceived`, its record of native money paid to it. The core
		// re-checks every one; this only asks.
		case 'rpc_get_safe_received_logs': {
			const filter = {
				fromBlock: operation.from_block,
				toBlock: operation.to_block,
				topics: [operation.topic],
				address: operation.address
			};
			return {
				type: 'safe_received_logs',
				address: operation.address,
				chain_id: operation.chain_id,
				outcome: await logsOutcome(filter, operation.chain_id)
			};
		}

		case 'rpc_get_block_by_number': {
			const blockNumber = hexToNumber(operation.block);
			const res = await poolRpcCall(
				'eth_getBlockByNumber',
				[operation.block, false],
				operation.chain_id
			);
			const timestamp = !res?.error
				? (res?.result as { timestamp?: unknown })?.timestamp
				: undefined;
			return {
				type: 'block_timestamp',
				address: operation.address,
				chain_id: operation.chain_id,
				// The core answers by block, so a block hex it cannot read back is
				// reported as 0 — the core drops a block it is not waiting for, and
				// the transfer stays withheld until its block is read (③).
				block_number: blockNumber ?? 0,
				// ③ the block's own time, or `null` — never a clock's.
				timestamp_sec: typeof timestamp === 'string' ? hexToNumber(timestamp) : null
			};
		}

		case 'multicall_erc20_meta': {
			const meta = await resolveTokenMetadata(operation.chain_id, operation.addrs);
			return {
				type: 'erc_meta',
				chain_id: operation.chain_id,
				// ② one entry per requested address. `resolveTokenMetadata` simply
				// omits what it could not resolve; the core needs that stated as
				// `meta: null` (its negative memo — invariant ⑦: no invented defaults).
				entries: operation.addrs.map((addr): TrustMetaEntry => {
					const resolved = meta.get(addr.toLowerCase());
					return {
						addr,
						meta: resolved ? { symbol: resolved.symbol, decimals: resolved.decimals } : null
					};
				})
			};
		}

		case 'read_custom_tokens': {
			const tokens = await loadCustomTokens();
			return { type: 'custom_tokens', tokens: tokens.map(toWireToken) };
		}

		case 'write_custom_token':
			await saveCustomToken(fromWireToken(operation.token));
			return { type: 'token_written', ok: true };

		case 'invalidate_token_cache':
			clearTokenCache(operation.address);
			return { type: 'cache_invalidated' };
	}
}

export function tokenTrustOperationFailure(effect: TrustEffect, error: unknown): TrustShellResult {
	void error; // classification is the core's; the twin answers by operation
	const operation = effect.operation;
	switch (operation.type) {
		case 'rpc_block_number':
			// `scanChain`'s catch: an unreachable endpoint yields nothing this tick.
			return {
				type: 'block_number',
				address: operation.address,
				chain_id: operation.chain_id,
				block_hex: null
			};
		case 'rpc_get_logs':
			// A thrown pool call has no `error` object to word-match, so it can only
			// be the non-range arm — `scanRecentTransfers`' `throw` → `catch { [] }`.
			return {
				type: 'logs',
				address: operation.address,
				chain_id: operation.chain_id,
				outcome: { type: 'failed' }
			};
		case 'rpc_get_safe_received_logs':
			// The same: a failure costs only the native receipts (issue #443).
			return {
				type: 'safe_received_logs',
				address: operation.address,
				chain_id: operation.chain_id,
				outcome: { type: 'failed' }
			};
		case 'rpc_get_block_by_number':
			// The rejected `Promise.allSettled` arm: the block was not read. No
			// time is made up for it (③): its transfers wait for a later poll.
			return {
				type: 'block_timestamp',
				address: operation.address,
				chain_id: operation.chain_id,
				block_number: hexToNumber(operation.block) ?? 0,
				timestamp_sec: null
			};
		case 'multicall_erc20_meta':
			// ② still one entry per address — silence would wedge the metadata gate.
			return {
				type: 'erc_meta',
				chain_id: operation.chain_id,
				entries: operation.addrs.map((addr) => ({ addr, meta: null }))
			};
		case 'read_custom_tokens':
			// `null` is "the read itself failed": the poll degrades to no customs
			// (`loadCustomTokens().catch(() => [])`) and an admission aborts
			// fail-closed. Distinct from an empty list, and the core needs both.
			return { type: 'custom_tokens', tokens: null };
		case 'write_custom_token':
			return { type: 'token_written', ok: false };
		case 'invalidate_token_cache':
			return { type: 'cache_invalidated' };
	}
}
