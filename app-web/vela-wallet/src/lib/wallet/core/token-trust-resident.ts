// Ported from src/services/wallet-state-core/token-trust-resident.ts @ c13e89d4 (spec 025).
/**
 * The resident `token_trust` session — WEB (spec 017, group G7).
 *
 * `token_trust` is an app-lifetime machine, not a screen-lifetime one: the
 * trusted-token set it builds is shared by the receive scanner, the auto-add
 * admission and the sign-sheet preview, and the whole point of unifying them
 * is that they see ONE set. So a single module-level session is created on
 * first use and never disposed — the `use-display-currency.web.ts` pattern,
 * scaled up.
 *
 * This module owns no rules. It is the shell half of the machine:
 *  - it pushes the four snapshot events the core cannot fetch for itself
 *    (held chains, held tokens, registry tokens, custom tokens),
 *  - it turns two of the core's view fields into the promise-shaped calls the
 *    existing TypeScript call sites expect, and
 *  - it serialises callers per pipeline so the core's "latest wins" sim
 *    session and single-flight poll are never raced from this side.
 *
 * Nothing here decides what happens next; every branch below is either a wire
 * translation or a wait.
 */

import { fetchChainTokens } from '$lib/services/chain-tokens';
import { getCachedHeldTokens } from '$lib/services/wallet-api';
import type { CustomToken } from '$lib/services/tokens-model';

import { createTokenTrustSession, type TokenTrustSession } from './token-trust-session';
import type { TrustAssetDelta } from '$lib/core/generated/TrustAssetDelta';
import type { TrustCustomToken } from '$lib/core/generated/TrustCustomToken';
import type { TrustEvent } from '$lib/core/generated/TrustEvent';
import type { TrustIncomingView } from '$lib/core/generated/TrustIncomingView';
import type { TrustReceiptLog } from '$lib/core/generated/TrustReceiptLog';
import type { TrustSimJudgment } from '$lib/core/generated/TrustSimJudgment';
import type { TrustSimView } from '$lib/core/generated/TrustSimView';
import type { TrustView } from '$lib/core/generated/TrustView';

/**
 * Safety valve for the poll wrapper only. Every operation this machine issues
 * is answered by the executor (that is the effect loop's contract, and the
 * `multicall_erc20_meta` arm answers every address precisely so a scan chain
 * cannot wedge), so this deadline should never fire. It exists because the
 * caller is `activity.ts`'s sync, which a screen awaits: an unanswered effect
 * must degrade to "this tick found nothing", never to a spinner that never
 * stops. It is a give-up, not a decision — the feed returned is whatever the
 * core has already committed.
 */
const POLL_DEADLINE_MS = 60_000;

const EMPTY_VIEW: TrustView = { address: null, scanning: false, incoming: [], sim: null };

let session: TokenTrustSession | null = null;
let started = false;
let view: TrustView = EMPTY_VIEW;
const listeners = new Set<(next: TrustView) => void>();

function ensureSession(): TokenTrustSession {
	if (!session) {
		session = createTokenTrustSession({
			onView: (next) => {
				view = next;
				listeners.forEach((listener) => listener(next));
			},
			onError: (error) => console.error('[token-trust] core fault:', error)
		});
	}
	return session;
}

/** `start` commits the core's initial view; every later event is a `dispatch`. */
function send(event: TrustEvent): void {
	const loop = ensureSession();
	if (!started) {
		started = true;
		loop.start(event);
		return;
	}
	loop.dispatch(event);
}

/** Resolve as soon as a committed view satisfies `ready`, else at the deadline. */
function waitForView(
	ready: (next: TrustView) => boolean,
	deadlineMs: number | null
): Promise<TrustView> {
	if (ready(view)) return Promise.resolve(view);
	return new Promise((resolve) => {
		let timer: ReturnType<typeof setTimeout> | null = null;
		const listener = (next: TrustView) => {
			if (!ready(next)) return;
			listeners.delete(listener);
			if (timer) clearTimeout(timer);
			resolve(next);
		};
		listeners.add(listener);
		if (deadlineMs !== null) {
			timer = setTimeout(() => {
				listeners.delete(listener);
				resolve(view);
			}, deadlineMs);
		}
	});
}

/**
 * One in-flight caller per pipeline. The core is single-flight for polls and
 * latest-wins for sims; queueing here means a second caller waits for the
 * first's answer instead of observing it, which is what the promise-shaped TS
 * call sites assume.
 */
function serialiser() {
	let tail: Promise<unknown> = Promise.resolve();
	return function run<T>(task: () => Promise<T>): Promise<T> {
		const next = tail.then(task, task);
		tail = next.catch(() => undefined);
		return next;
	};
}

const pollQueue = serialiser();
const simQueue = serialiser();

// ---------------------------------------------------------------------------
// Snapshot events — facts the core cannot fetch for itself
// ---------------------------------------------------------------------------

/**
 * The ERC-20 addresses this account holds on one chain (`getCachedHeldTokens`).
 * Never snapshotting a chain leaves its held set empty, which is the safe
 * direction: everything received there stays unverified.
 */
export function notifyHeldTokens(address: string, chainId: number, tokens: string[]): void {
	send({ type: 'held_tokens_snapshot', address, chain_id: chainId, tokens });
}

/**
 * The user's custom tokens changed elsewhere (the manage-tokens panel). The
 * poll re-reads them at poll start anyway, so this is the warm-copy refresh —
 * see the auto-add note in the integration report for why nothing calls it yet.
 */
export function notifyCustomTokensLoaded(tokens: CustomToken[]): void {
	send({ type: 'custom_tokens_loaded', tokens: tokens.map(toWireToken) });
}

/**
 * AUTHENTIC receipt logs of a confirmed transaction — the ONLY auto-add entry
 * point (invariant ⑤). It must be reached from `tx_tracker`'s `NotifyConfirmed`
 * and from nowhere else; a sign-time simulation must NEVER be routed here.
 *
 * The held-token snapshot is pushed first because the admission filter reads
 * it (`token-autoadd.ts:53` takes the same synchronous cache read at exactly
 * this moment); folding it in here means a future caller cannot forget it and
 * silently re-list a token the user already holds.
 */
export function notifyReceiptLogsConfirmed(
	from: string,
	chainId: number,
	logs: TrustReceiptLog[]
): void {
	notifyHeldTokens(from, chainId, getCachedHeldTokens(from, chainId));
	send({ type: 'receipt_logs_confirmed', from, chain_id: chainId, logs });
}

/**
 * Push the token registry's facts for each chain — the same `fetchChainTokens`
 * read `transferAllowlist` and `trustedReceiveSet` do today, with the same
 * "registry unreachable → no facts" degradation (which leaves the chain's
 * stables out of the allowlist and its tokens out of the trusted set).
 */
export async function primeRegistry(chainIds: number[]): Promise<void> {
	await Promise.all(
		chainIds.map(async (chainId) => {
			const data = await fetchChainTokens(chainId).catch(() => null);
			if (!data) return;
			send({
				type: 'registry_tokens_snapshot',
				chain_id: chainId,
				stables: data.stables
					.map((stable) => stable?.contract)
					.filter((contract): contract is string => !!contract),
				wrapped_native: data.wrappedNativeToken ?? null
			});
		})
	);
}

// ---------------------------------------------------------------------------
// Promise-shaped reads
// ---------------------------------------------------------------------------

/**
 * Run one scan poll and answer with the judged incoming feed.
 *
 * The feed is the machine's cumulative, id-de-duped memory of this account —
 * not just this tick's window — which is why the answer is the whole
 * `incoming` list. `activity.ts` merges it into the local store by the same
 * stable id, so a repeat costs nothing and a row that only became renderable
 * on a later poll still lands.
 */
export function pollIncoming(address: string, chainIds: number[]): Promise<TrustIncomingView[]> {
	return pollQueue(async () => {
		await primeRegistry(chainIds);
		send({ type: 'held_chains_snapshot', address, chain_ids: chainIds });
		send({ type: 'poll_requested', address });
		const settled = await waitForView((next) => !next.scanning, POLL_DEADLINE_MS);
		return settled.incoming;
	});
}

/**
 * Judge one simulation's deltas (invariant ⑥'s asymmetric trust) and answer
 * with the machine's whole judged view: the judgments, and its own word when
 * nothing of the person's moves (`no_change_key`). UNTRUSTED input: this path
 * can reach no write in the core at all.
 *
 * `null` when the view the machine committed is not this simulation's — it
 * never should be (the queue below runs one at a time, and the core's session
 * is the latest asked), and a caller told `null` says "could not check"
 * rather than drawing somebody else's judgment.
 *
 * No deadline here on purpose. The judgment costs at most one metadata
 * multicall, the executor answers every requested address, and a give-up would
 * have to invent a verdict — which is exactly the decision this machine exists
 * to own.
 */
export function judgeSimView(
	address: string,
	chainId: number,
	deltas: TrustAssetDelta[]
): Promise<TrustSimView | null> {
	return simQueue(async () => {
		send({ type: 'sim_deltas_computed', address, chain_id: chainId, deltas });
		const settled = await waitForView((next) => next.sim?.ready === true, null);
		const sim = settled.sim;
		if (!sim || sim.chain_id !== chainId || sim.address !== address.toLowerCase()) return null;
		return sim;
	});
}

/** {@link judgeSimView}'s judgments alone — what Send's own preview reads. */
export async function judgeSimDeltas(
	address: string,
	chainId: number,
	deltas: TrustAssetDelta[]
): Promise<TrustSimJudgment[]> {
	return (await judgeSimView(address, chainId, deltas))?.judgments ?? [];
}

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
