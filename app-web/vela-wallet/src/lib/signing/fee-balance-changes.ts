/**
 * What a signing request's calls move, for the fee machine (spec 083 fee,
 * issue 411).
 *
 * The fee machine picks the coin nobody chose from what the operation LEAVES
 * of each one. The calls say part of that; a router's swap path only NAMES
 * the coins it trades, so until a simulation says how much of them the swap
 * leaves, the machine cannot count on either (spec 096 F2), and with a
 * contract call it falls back to the chain's coin — held or not. On Android a
 * Uniswap swap on Polygon (pUSD → USDC) opened with POL, held at 0, as the
 * fee coin, while pUSD and USDC could each pay. The desktop and the phones
 * simulate the request for their balance block and now tell the machine what
 * it moves; this sheet draws no balance block (spec 082 RG6), so it runs the
 * same `eth_simulateV1` read for the fee alone.
 *
 * What it hands over is the desktop's `fee_balance_changes`, rule for rule:
 * native value from the node's trace, each token by its OWN `Transfer` logs
 * (the contract that emitted them) — nothing a site's contract can emit on a
 * coin's behalf. A run that reverted, or that nobody could check, is no
 * measurement, and the machine is told nothing.
 */
import type { FeeBalanceChange } from '$lib/core/generated/FeeBalanceChange';
import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { AssetDelta, EngineResult, SimCall } from '$lib/services/sim/sim-assets';

/**
 * A clean run's deltas as the fee machine reads them: native → `token: null`;
 * an ERC-20 by its contract. A token move with no contract names no coin and
 * is dropped — never read as the native one.
 */
export function feeBalanceChanges(deltas: AssetDelta[]): FeeBalanceChange[] {
	const changes: FeeBalanceChange[] = [];
	for (const delta of deltas) {
		if (delta.kind === 'native') {
			changes.push({ token: null, delta: delta.delta.toString() });
		} else if (delta.token) {
			changes.push({ token: delta.token, delta: delta.delta.toString() });
		}
	}
	return changes;
}

/** The `eth_simulateV1` engine, as a seam. */
export type Simulate = (
	from: string,
	calls: SimCall[],
	chainId: number
) => Promise<EngineResult | null>;

/**
 * The pool's `eth_simulateV1` engine, loaded when a request is first measured
 * rather than with the sheet: the RPC pool is not the signing host's to pull
 * in at import.
 */
const rpcSimulate: Simulate = async (from, calls, chainId) =>
	(await import('$lib/services/sim/sim-engine-rpc')).rpcSimulate(from, calls, chainId);

/**
 * Simulate `calls` from `from` and say what they move, or `null` when there is
 * nothing to tell: no node could simulate them, or they revert.
 */
export async function measureBalanceChanges(
	from: string,
	calls: FeeCall[],
	chainId: number,
	simulate: Simulate = rpcSimulate
): Promise<FeeBalanceChange[] | null> {
	if (calls.length === 0) return null;
	let result: EngineResult | null;
	try {
		result = await simulate(
			from,
			calls.map((call) => ({ to: call.to, value: call.value, data: call.data })),
			chainId
		);
	} catch {
		return null;
	}
	if (!result || !result.ok) return null;
	return feeBalanceChanges(result.deltas);
}

/** Where the measurement goes — the surface's speed control (`SpeedControl.balanceChanges`). */
export interface BalanceChangesSink {
	balanceChanges(calls: FeeCall[], changes: FeeBalanceChange[]): void;
}

/**
 * Measure `calls` and tell `sink` — unless the request it was measured for is
 * gone by the time the node answers (`current` says so). Resolves to whether
 * anything was told, for the log and the tests.
 */
export async function tellBalanceChanges(
	sink: BalanceChangesSink,
	request: { from: string; calls: FeeCall[]; chainId: number },
	current: () => boolean,
	simulate: Simulate = rpcSimulate
): Promise<boolean> {
	const changes = await measureBalanceChanges(
		request.from,
		request.calls,
		request.chainId,
		simulate
	);
	if (changes === null || !current()) return false;
	sink.balanceChanges(request.calls, changes);
	return true;
}
