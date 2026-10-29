/**
 * Who can act for a multisig on a chain (docs/multisig-vault-design.md §3.5)
 * — read here with the wallet's own RPC, decided by the core. Neither the
 * genesis service nor the mailbox decides it:
 *
 * - deployed on a chain, the chain does: its owners and threshold, and
 *   anything else that can act — a module, a guard, a fallback handler or
 *   singleton that is not Safe's (`deployedCheck`);
 * - not deployed there yet, only the core's counterfactual rule does
 *   (`counterfactualAuthority`): a genesis is Verified only if its setup
 *   runs nothing Vela cannot read in full, and the Safe contracts it names
 *   hold Safe's own code on that chain. A chain that does not answer leaves
 *   it unverified — never Verified.
 */
import { poolRpcCall } from '$lib/services/rpc-pool';
import {
	counterfactualAuthority,
	counterfactualReads,
	deployedCheck,
	deployedReads,
	type Authority,
	type ChainFacts,
	type DeployedCheck,
	type Genesis
} from './kernel';

export { GenesisRefusedError, type Finding } from './refusal';

/** An answer of the chain, or `null` when it gave none. */
async function read(chainId: number, method: string, params: unknown[]): Promise<string | null> {
	try {
		const answer = await poolRpcCall(method, params, chainId);
		return !answer.error && typeof answer.result === 'string' ? answer.result : null;
	} catch {
		return null;
	}
}

/** `eth_getCode` of each address on `chainId`, as the core takes it. */
async function codesOf(
	chainId: number,
	addresses: string[]
): Promise<Record<string, string | null>> {
	const codes = await Promise.all(
		addresses.map(async (a) => [a, await read(chainId, 'eth_getCode', [a, 'latest'])] as const)
	);
	return Object.fromEntries(codes);
}

/**
 * The code on `chainId` of everything `genesis` depends on (the core names
 * it), read now — never kept: the executor asks right before it deploys.
 */
export async function counterfactualFacts(
	chainId: number,
	safe: string,
	genesis: Genesis
): Promise<ChainFacts> {
	return { codes: await codesOf(chainId, counterfactualReads(genesis, safe)) };
}

/** The core's verdict on `safe`, not deployed on `chainId`, deployed there from `genesis`. */
export async function counterfactualOn(
	chainId: number,
	safe: string,
	genesis: Genesis
): Promise<Authority> {
	return counterfactualAuthority(genesis, safe, await counterfactualFacts(chainId, safe, genesis));
}

/**
 * What can act for `safe`, deployed on `chainId`, besides its members: its
 * modules, guards, fallback handler and singleton, read from its storage,
 * then the code of what they name — judged by the core.
 */
export async function deployedOn(chainId: number, safe: string): Promise<DeployedCheck> {
	const reads = deployedReads();
	const slot = (s: string) => read(chainId, 'eth_getStorageAt', [safe, s, 'latest']);
	const [modules, guard, moduleGuard, handler, singleton] = await Promise.all([
		read(chainId, 'eth_call', [{ to: safe, data: reads.modulesCall }, 'latest']),
		slot(reads.guardSlot),
		slot(reads.moduleGuardSlot),
		slot(reads.handlerSlot),
		slot(reads.singletonSlot)
	]);
	const facts = { modules, guard, moduleGuard, handler, singleton, codes: {} };
	const first = deployedCheck(facts);
	if (first.needCode.length === 0) return first;
	return deployedCheck({ ...facts, codes: await codesOf(chainId, first.needCode) });
}
