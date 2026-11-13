/**
 * R22 (review 1-2, KNOWN-BUGS SEC-PARENT-3): one RPC never decides what the
 * signer believes. A read a signing verdict stands on — the sheet's own
 * simulation, a multisig guard's reads, §31's isOwner — is a corroborated
 * pool call ({@link poolCorroboratedCall}): two independent endpoints, each
 * of which said its chain first, or the person's own node alone by their
 * choice. This file only carries both answers to the core in ITS envelope
 * (`rpcCorroboratedEnvelope`, `vela_core::app::corroborate`) — what they
 * mean, and whether they agree, is the core's.
 */
import { rpcCorroboratedEnvelope } from '$lib/core/kernels';
import { poolCorroboratedCall, type CorroboratedAnswer } from './rpc-pool';

/** A corroborated read, as a seam (a test's two nodes). Throws when no node answered. */
export type CorroboratedCall = (
	method: string,
	params: unknown[],
	chainId: number
) => Promise<CorroboratedAnswer>;

/** The core's envelope for an answer — `null`: the pool gave up (`{"unreachable":true}`). */
export function envelopeOf(answer: CorroboratedAnswer | null): string {
	return answer === null
		? rpcCorroboratedEnvelope(null, null, false)
		: rpcCorroboratedEnvelope(
				answer.body,
				answer.witnessBody,
				answer.ownNode,
				// §39 (r22-2): the pool holds no other provider to ask.
				answer.soleProvider
			);
}

/**
 * The multisig shell's corroborated read (`MultisigOperation::Rpc {corroborate}`):
 * the core's envelope as a JSON object. Throws when no node answered — the
 * shell answers `rpc_unanswered` with the pool's words (a rate limit said so).
 */
export async function corroboratedRpc(
	chainId: number,
	method: string,
	params: unknown,
	person = false,
	call: (
		method: string,
		params: unknown[],
		chainId: number,
		options: { person?: boolean }
	) => Promise<CorroboratedAnswer> = poolCorroboratedCall
): Promise<unknown> {
	const answer = await call(
		method,
		(Array.isArray(params) ? params : [params]) as unknown[],
		chainId,
		{ person }
	);
	return JSON.parse(envelopeOf(answer)) as unknown;
}

/**
 * `method(params)` on `chainId`, corroborated, as the core's envelope JSON:
 * the first node's answer, the witness's, `ownNode` — or
 * `{"unreachable":true}` when no node answered. Never rejects.
 */
export async function corroboratedEnvelope(
	method: string,
	params: unknown[],
	chainId: number,
	call: CorroboratedCall = poolCorroboratedCall
): Promise<string> {
	let answer: CorroboratedAnswer | null;
	try {
		answer = await call(method, params, chainId);
	} catch {
		answer = null;
	}
	return envelopeOf(answer);
}
