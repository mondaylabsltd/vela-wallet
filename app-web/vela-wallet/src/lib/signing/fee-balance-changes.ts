/**
 * The sheet's own simulation of a signing request: one `eth_simulateV1` read,
 * read by the core, for the fee machine and for the sheet (spec 083 fee,
 * issue 411; PR 3 device round, item 3; the verdict, drawn whole).
 *
 * **For the fee.** The fee machine picks the coin nobody chose from what the
 * operation LEAVES of each one. The calls say part of that; a router's swap
 * path only NAMES the coins it trades, so until a simulation says how much of
 * them the swap leaves, the machine cannot count on either (spec 096 F2), and
 * with a contract call it falls back to the chain's coin — held or not. On
 * Android a Uniswap swap on Polygon (pUSD → USDC) opened with POL, held at 0,
 * as the fee coin, while pUSD and USDC could each pay. It is told what a
 * check moves BEFORE the tokens are judged: which coin can pay does not wait
 * on token names.
 *
 * **For the sheet.** The same read is the sheet's verdict — the one part of
 * it the site being signed for cannot write: what moves, that nothing does,
 * that the call fails and why, or that nothing could be checked
 * (`simulateRequest` → `verdict.ts`, the phones' and the desktop's block).
 *
 * What the reply MEANS is the core's (`simOutcome` → `sim_outcome::verdict`),
 * the same on every client: the moves are native value from the node's trace
 * and each token by its OWN `Transfer` logs (the contract that emitted them)
 * — nothing a site's contract can emit on a coin's behalf. Only a check
 * (`kind: 'deltas'`) is a measurement: a run that reverted, or that nobody
 * could check — a status neither 0 nor 1 among them, which this shell's own
 * port read as a success — tells the fee machine nothing. And what a check's
 * moves may be PRINTED as is the core's too (`token_trust`, through
 * `judgeCheck`): this file carries records between the two and reads neither.
 */
import { simOutcome } from '$lib/core/kernels';
import type { FeeBalanceChange } from '$lib/core/generated/FeeBalanceChange';
import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { SimVerdict } from '$lib/core/generated/SimVerdict';
import type { TrustAssetDelta } from '$lib/core/generated/TrustAssetDelta';
import type { TrustSimView } from '$lib/core/generated/TrustSimView';
import type { SimCall } from '$lib/services/sim/sim-assets';
import type { SimEnded } from './verdict';

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
 * A check's moves, judged by the trust machine, as a seam: its whole judged
 * view, or `null` when it gave none for this simulation.
 */
export type Judge = (
	from: string,
	chainId: number,
	deltas: TrustAssetDelta[]
) => Promise<TrustSimView | null>;

/**
 * The resident `token_trust` session's judgment, loaded when a check is
 * first judged rather than with the sheet — as the pool's read is.
 */
const trustJudge: Judge = async (from, chainId, deltas) =>
	(await import('$lib/services/sim/sim-trust')).judgeCheck(deltas, chainId, from);

/**
 * A request's simulation from start to END, for the sheet: the one read
 * (`checkRequest` — which tells the fee machine what a check moves, before
 * anything is judged), then, for a check, the trust machine's judgment of its
 * moves. What comes back is everything the verdict's place draws from.
 *
 * The verdict is on the sheet when THIS resolves, not when the node replied:
 * a check's rows need its tokens judged, and the host tells the core its
 * wait is over in the step it hands this to the sheet. A notice needs no
 * judgment and resolves at once.
 *
 * - `null` — the request it was run for is gone: nobody is told, nothing is
 *   said.
 * - `verdict: null` — there was no reply to read (nothing to ask, or an
 *   answer that is no record): a simulation that has ended with no check.
 * - `judged: null` under a check — the trust machine gave no view for it, or
 *   judging threw: a check this sheet cannot report. Never "nothing moves".
 */
export async function simulateRequest(
	sink: BalanceChangesSink,
	request: { from: string; calls: FeeCall[]; chainId: number },
	current: () => boolean,
	simulate: Simulate = rpcSimulate,
	judge: Judge = trustJudge
): Promise<SimEnded | null> {
	const verdict = await checkRequest(sink, request, current, simulate);
	if (!current()) return null;
	if (verdict === null || verdict.kind !== 'deltas') return { verdict, judged: null };
	let judged: TrustSimView | null;
	try {
		judged = await judge(request.from, request.chainId, verdict.deltas);
	} catch {
		judged = null;
	}
	return current() ? { verdict, judged } : null;
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
