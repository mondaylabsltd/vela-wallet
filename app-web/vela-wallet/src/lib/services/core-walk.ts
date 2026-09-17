/**
 * The shell's half of a core "walk" — WEB.
 *
 * `vela_core::registry_lookup` and `vela_core::registry_backup` are pure
 * functions of a transcript: the core says what to fetch, the shell fetches it,
 * appends the answer and asks again. Both speak the same request/answer types,
 * so both are carried by this one transport. Every rule — what is asked, in
 * what order, what an answer means — is the core's; nothing here decides.
 */
import { getPasskeyIndexURL } from './endpoints';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';
import { poolRpcCall } from './rpc-pool';

/** `registry_lookup::LookupRequest`, as it crosses the boundary. */
export type WalkRequest =
	| { type: 'eth_call'; id: string; chain_id: number; to: string; data: string }
	| { type: 'index_get'; id: string; path: string };

/** `registry_lookup::LookupAnswer`. `not_found` is the other side SAYING no;
 *  `failed` is nobody saying anything — the core's caching rule hangs on it. */
export interface WalkAnswer {
	id: string;
	outcome: 'ok' | 'not_found' | 'failed';
	body: string | null;
}

/** A walk is a handful of rounds; this only stops a contract bug from spinning. */
const MAX_ROUNDS = 16;

const failed = (id: string): WalkAnswer => ({ id, outcome: 'failed', body: null });

/** Perform one request. Never throws: every failure is `failed`. */
export async function performWalkRequest(request: WalkRequest): Promise<WalkAnswer> {
	const { id } = request;
	try {
		if (request.type === 'eth_call') {
			const response = await poolRpcCall(
				'eth_call',
				[{ to: request.to, data: request.data }, 'latest'],
				request.chain_id
			);
			// A bare `0x` is an ANSWER ("no such contract here") and is passed on as
			// one; only an RPC error or a missing result is a silence.
			return response.error == null && typeof response.result === 'string'
				? { id, outcome: 'ok', body: response.result }
				: failed(id);
		}
		// Read per call, so a settings edit reaches the next lookup.
		const baseUrl = getPasskeyIndexURL().trim().replace(/\/$/, '');
		const response = await fetchWithTimeout(
			baseUrl + request.path,
			{},
			{ timeoutMs: NET_TIMEOUTS.keyIndexRead }
		);
		// "No such key / unit" is an answer; any other non-OK is the index failing.
		if (response.status === 404) return { id, outcome: 'not_found', body: null };
		if (!response.ok) return failed(id);
		return { id, outcome: 'ok', body: await response.text() };
	} catch {
		return failed(id);
	}
}

/**
 * Drive a walk to its end. `step` is the core function closed over its fixed
 * arguments: transcript JSON in, step JSON out. Resolves with the `done` step,
 * or `null` if the core never finished (a contract bug, not a network state).
 */
export async function runWalk<Done extends { type: 'done' }>(
	step: (answersJson: string) => string
): Promise<Done | null> {
	const answers: WalkAnswer[] = [];
	for (let round = 0; round < MAX_ROUNDS; round++) {
		const next = JSON.parse(step(JSON.stringify(answers))) as
			{ type: 'ask'; requests: WalkRequest[] } | Done;
		if (next.type === 'done') return next;
		// "Together, if the shell can."
		answers.push(...(await Promise.all(next.requests.map(performWalkRequest))));
	}
	return null;
}
