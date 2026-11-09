/**
 * Tests: R22's corroborated reads in a simulated world — the multisig
 * guard's reads (`MultisigOperation::Rpc {corroborate}`) answered by the
 * world's one fake node TWICE, as two independent nodes agreeing, in the
 * core's own envelope (`rpcCorroboratedEnvelope`). A test that means "only
 * one node answered" or "the two disagree" passes its own `rpcCorroborated`.
 * The wasm must be initialized first.
 */
import { rpcCorroboratedEnvelope } from '$lib/core/kernels';
import type { MultisigShellDeps } from './handler';

/** `rpc`'s answer, given by two independent nodes alike. Throws as `rpc` throws. */
export function witnessedBy(rpc: MultisigShellDeps['rpc']): MultisigShellDeps['rpcCorroborated'] {
	return async (chainId, method, params, person) => {
		const reply = await rpc(chainId, method, params, person);
		const body = { jsonrpc: '2.0', id: 1, ...reply };
		return JSON.parse(rpcCorroboratedEnvelope(body, body, false)) as unknown;
	};
}

/** A shell whose corroborated reads are its `rpc`'s, witnessed — unless `over` says otherwise. */
export function withWitness(
	deps: Omit<MultisigShellDeps, 'rpcCorroborated'> &
		Partial<Pick<MultisigShellDeps, 'rpcCorroborated'>>,
	over: Partial<MultisigShellDeps> = {}
): MultisigShellDeps {
	return { ...deps, rpcCorroborated: over.rpcCorroborated ?? witnessedBy(deps.rpc) };
}
