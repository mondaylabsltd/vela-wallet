/**
 * The sheet's own simulation of a signing request: one `eth_simulateV1` read,
 * read by the core, for the fee machine and for the sheet (spec 083 fee,
 * issue 411; PR 3 device round, item 3).
 *
 * **For the fee.** The fee machine picks the coin nobody chose from what the
 * operation LEAVES of each one. The calls say part of that; a router's swap
 * path only NAMES the coins it trades, so until a simulation says how much of
 * them the swap leaves, the machine cannot count on either (spec 096 F2), and
 * with a contract call it falls back to the chain's coin — held or not. On
 * Android a Uniswap swap on Polygon (pUSD → USDC) opened with POL, held at 0,
 * as the fee coin, while pUSD and USDC could each pay. The desktop and the
 * phones simulate the request for their balance block and tell the machine
 * what it moves; this sheet draws no balance rows (spec 082 RG6), so it runs
 * the same read for the fee.
 *
 * **For the sheet.** The same read says when nothing of the person's moves,
 * and the sheet says so in the core's line ("No asset changes",
 * `SimVerdict.no_change_key`) — the one thing it draws from this simulation.
 *
 * What the reply MEANS is the core's (`simOutcome` → `sim_outcome::verdict`),
 * the same on every client: the moves are native value from the node's trace
 * and each token by its OWN `Transfer` logs (the contract that emitted them)
 * — nothing a site's contract can emit on a coin's behalf. Only a check
 * (`kind: 'deltas'`) is a measurement: a run that reverted, or that nobody
 * could check — a status neither 0 nor 1 among them, which this shell's own
 * port read as a success — tells the fee machine nothing and the sheet
 * nothing.
 */
import { simOutcome } from '$lib/core/kernels';
import type { FeeBalanceChange } from '$lib/core/generated/FeeBalanceChange';
import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { SimVerdict } from '$lib/core/generated/SimVerdict';
import type { TrustAssetDelta } from '$lib/core/generated/TrustAssetDelta';
import type { SimCall } from '$lib/services/sim/sim-assets';

/**
 * A check's moves as the fee machine reads them: native → `token: null`; an
 * ERC-20 by its contract. A token move with no contract names no coin and is
 * dropped — never read as the native one.
 */
export function feeBalanceChanges(deltas: TrustAssetDelta[]): FeeBalanceChange[] {
	const changes: FeeBalanceChange[] = [];
	for (const delta of deltas) {
		if (delta.kind === 'native') {
			changes.push({ token: null, delta: delta.delta });
		} else if (delta.token) {
			changes.push({ token: delta.token, delta: delta.delta });
		}
	}
	return changes;
}

/**
 * The `eth_simulateV1` read, as a seam: the node's reply as it came — the
 * JSON-RPC envelope, or `{"unreachable":true}` when the pool gave up.
 */
export type Simulate = (from: string, calls: SimCall[], chainId: number) => Promise<string>;

/** The pool gave up (the core's wire form for "no node answered"). */
const UNREACHABLE_REPLY = '{"unreachable":true}';

/**
 * The pool's `eth_simulateV1` read, loaded when a request is first measured
 * rather than with the sheet: the RPC pool is not the signing host's to pull
 * in at import.
 */
const rpcSimulate: Simulate = async (from, calls, chainId) =>
	(await import('$lib/services/sim/sim-engine-rpc')).rpcSimulateReply(from, calls, chainId);

/**
 * Simulate `calls` from `from` and hand back what the core says of the reply.
 * `null` when there was nothing to ask (no calls), or the core's answer does
 * not read. A read that threw is the pool giving up: "unreachable".
 */
export async function simulateVerdict(
	from: string,
	calls: FeeCall[],
	chainId: number,
	simulate: Simulate = rpcSimulate
): Promise<SimVerdict | null> {
	if (calls.length === 0) return null;
	let reply: string;
	try {
		reply = await simulate(
			from,
			calls.map((call) => ({ to: call.to, value: call.value, data: call.data })),
			chainId
		);
	} catch {
		reply = UNREACHABLE_REPLY;
	}
	return simOutcome(from, reply);
}

/**
 * What a verdict tells the fee machine: a check's moves, or `null` — a revert
 * or a could-not-check is no measurement.
 */
export function measuredChanges(verdict: SimVerdict | null): FeeBalanceChange[] | null {
	return verdict !== null && verdict.kind === 'deltas' ? feeBalanceChanges(verdict.deltas) : null;
}

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
	return measuredChanges(await simulateVerdict(from, calls, chainId, simulate));
}

/** Where the measurement goes — the surface's speed control (`SpeedControl.balanceChanges`). */
export interface BalanceChangesSink {
	balanceChanges(calls: FeeCall[], changes: FeeBalanceChange[]): void;
}

/**
 * Simulate a request's calls ONCE, for both readers: tell `sink` what they
 * move (a check only), and hand the core's verdict back for the sheet —
 * unless the request it was measured for is gone by the time the node
 * answers (`current` says so): then nobody is told, and there is no verdict.
 */
export async function checkRequest(
	sink: BalanceChangesSink,
	request: { from: string; calls: FeeCall[]; chainId: number },
	current: () => boolean,
	simulate: Simulate = rpcSimulate
): Promise<SimVerdict | null> {
	const verdict = await simulateVerdict(request.from, request.calls, request.chainId, simulate);
	if (verdict === null || !current()) return null;
	const changes = measuredChanges(verdict);
	if (changes !== null) sink.balanceChanges(request.calls, changes);
	return verdict;
}

/**
 * Measure `calls` and tell `sink` — unless the request it was measured for is
 * gone by the time the node answers. Resolves to whether anything was told,
 * for the log and the tests.
 */
export async function tellBalanceChanges(
	sink: BalanceChangesSink,
	request: { from: string; calls: FeeCall[]; chainId: number },
	current: () => boolean,
	simulate: Simulate = rpcSimulate
): Promise<boolean> {
	return measuredChanges(await checkRequest(sink, request, current, simulate)) !== null;
}
