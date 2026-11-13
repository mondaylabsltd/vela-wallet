/**
 * R22 (review 1-2): `eth_simulateV1` replies (`sim-replies.ts`) in the core's
 * corroborated envelope (`rpcCorroboratedEnvelope`) — as two independent
 * nodes gave them, as the person's own node gave them alone, or as one node
 * gave them with nobody independent answering. For a drawn board and a test
 * to hand the REAL core (`simOutcome`) what the pool now hands it.
 */
import { rpcCorroboratedEnvelope } from '$lib/core/kernels';
/**
 * R22: `reply` as two independent nodes gave it — `witness` the second's
 * (the same answer, unless a test says otherwise) — in the core's envelope.
 */
export function witnessed(reply: string, witness: string = reply): string {
	return rpcCorroboratedEnvelope(JSON.parse(reply), JSON.parse(witness), false);
}

/**
 * R22: `reply` witnessed alike when it is a node's answer at all (a `result`
 * or an `error`); anything else — no reply, `{"unreachable":true}`, text
 * that is no JSON — passes as it is, for the core to read as it reads it.
 */
export function agreed(reply: string): string {
	let parsed: unknown;
	try {
		parsed = JSON.parse(reply);
	} catch {
		return reply;
	}
	const answer = parsed !== null && typeof parsed === 'object' ? parsed : {};
	return 'result' in answer || 'error' in answer ? witnessed(reply) : reply;
}

/** R22: `reply` as the person's own node gave it, alone ("Only use my node"). */
export function ownNodeAlone(reply: string): string {
	return rpcCorroboratedEnvelope(JSON.parse(reply), null, true);
}

/** R22: `reply` as one node gave it, nobody independent answering — "unconfirmed". */
export function singleNode(reply: string): string {
	return rpcCorroboratedEnvelope(JSON.parse(reply), null, false);
}
