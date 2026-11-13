// Ported from src/services/sim-engine-rpc.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/* eslint-disable @typescript-eslint/no-explicit-any -- verbatim port: these walk dynamic wire
   shapes (decoded params, eth_simulateV1 results, selector DB rows) that Expo typed `any`;
   narrowing them is a rewrite, not a port (spec 026 results.md, Phase 2). */
/**
 * RPC simulation engine — `eth_simulateV1`.
 *
 * The primary engine. It executes the inner Safe→target call(s) in a simulated
 * block against live chain state via the user's own RPC pool (no third-party
 * "simulation" service), then derives the wallet's asset deltas from the logs.
 *
 *   - `validation: false`  → don't require the Safe to hold funds/nonce; we're
 *     asking "what would happen", and in the 4337 model the Safe pays no gas.
 *   - `traceTransfers: true` → native value moves come back as synthetic
 *     `Transfer` logs (sender = NATIVE_TRANSFER_SENTINEL), so one log parser
 *     covers both native and ERC-20.
 *
 * Returns `null` when the engine can't answer — the endpoint doesn't implement
 * `eth_simulateV1`, params were rejected, or every RPC was unreachable. The
 * orchestrator treats `null` as "try the next engine / degrade", never as a
 * result. A genuine top-level execution revert still comes back as
 * `{ ok: false }` (per-call `status`/`error`), not `null`.
 */
import { rpcCall } from '../rpc-adapter';
import { corroboratedEnvelope, type CorroboratedCall } from '../corroborated-read';
import {
	deriveAssetDeltas,
	parseRevertReason,
	simValueParam,
	type EngineResult,
	type SimCall,
	type SimLog
} from './sim-assets';

/** A per-call result counts as success only when explicitly status 1. */
function callSucceeded(call: any): boolean {
	const s = call?.status;
	if (s === '0x1' || s === 1) return true;
	if (s === '0x0' || s === 0) return false;
	// Some nodes omit status; absence of an error is our best success signal.
	return call?.error == null;
}

/** The `eth_simulateV1` question: `calls` in one simulated block, from `from`. */
function simulatePayload(from: string, calls: SimCall[], stateOverrides?: unknown) {
	return {
		blockStateCalls: [
			{
				// Real-device S2: an undeployed multisig placed as its deployment
				// leaves it (the core's override), so its first transaction is
				// simulated as it will run — not from a bare address.
				...(stateOverrides ? { stateOverrides } : {}),
				calls: calls.map((c) => ({
					from,
					to: c.to,
					...(c.data && c.data !== '0x' ? { data: c.data } : {}),
					value: simValueParam(c.value)
				}))
			}
		],
		traceTransfers: true,
		validation: false,
		returnFullTransactions: false
	};
}

/**
 * The same read, UNREAD and CORROBORATED (R22, review 1-2): both nodes'
 * replies as they came, in the core's envelope (`rpcCorroboratedEnvelope`)
 * — the first node's JSON-RPC answer, a second, independent node's
 * (`witness`), or `ownNode` when the person's own node answered alone — or
 * `{"unreachable":true}` when every endpoint failed. For the core to say what
 * it means (`simOutcome`): a reply nobody independent confirmed is
 * "unconfirmed", never a plain verdict. Nothing here decides whether a call
 * ran, reverted, could not be checked, or whether the two agree.
 */
export async function rpcSimulateReply(
	from: string,
	calls: SimCall[],
	chainId: number,
	stateOverrides?: unknown,
	call?: CorroboratedCall
): Promise<string> {
	// Nothing was asked: an answer nobody can read, never "nothing moves".
	if (!calls.length || !calls[0]?.to) return '{}';
	return corroboratedEnvelope(
		'eth_simulateV1',
		[simulatePayload(from, calls, stateOverrides), 'latest'],
		chainId,
		call
	);
}

/**
 * Simulate `calls` (executed sequentially in one block, sharing state) sent
 * from `from` (the Safe), and net the resulting transfers for `from`.
 */
export async function rpcSimulate(
	from: string,
	calls: SimCall[],
	chainId: number
): Promise<EngineResult | null> {
	if (!calls.length || !calls[0]?.to) return null;

	const payload = simulatePayload(from, calls);

	let res;
	try {
		res = await rpcCall('eth_simulateV1', [payload, 'latest'], chainId);
	} catch {
		return null; // every endpoint failed — unknown, let the caller degrade
	}

	// A top-level error means the method/params were rejected (unsupported node,
	// bad shape) — not an execution revert. Degrade rather than report a result.
	if (!res || res.error || !Array.isArray(res.result) || res.result.length === 0) {
		return null;
	}

	const callResults: any[] = res.result.flatMap((b: any) =>
		Array.isArray(b?.calls) ? b.calls : []
	);
	if (callResults.length === 0) return null;

	let ok = true;
	let revertReason: string | undefined;
	const logs: SimLog[] = [];

	for (const call of callResults) {
		if (!callSucceeded(call)) {
			ok = false;
			if (!revertReason) revertReason = parseRevertReason(call?.error);
		}
		if (Array.isArray(call?.logs)) {
			for (const lg of call.logs) {
				logs.push({ address: lg?.address ?? '', topics: lg?.topics ?? [], data: lg?.data ?? '0x' });
			}
		}
	}

	// If it reverts, the EVM discards its effects and emits no transfer logs, so
	// there are no honest deltas to show — keep it empty and let `ok:false` speak.
	const deltas = ok ? deriveAssetDeltas(logs, from) : [];
	return { ok, revertReason, deltas };
}
