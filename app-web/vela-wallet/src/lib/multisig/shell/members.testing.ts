/**
 * Tests: the core's members machine (the real wasm) on the host, over a
 * simulated world of chains — each network's Safe (deployed or not, its
 * owners, threshold and nonce) and what each member's address is: a Vela
 * account (Vela's passkey signer configured for it), a smart contract
 * account (code, no signer), a plain wallet (neither), or nobody answering.
 * What a screen's test stands up instead of seeding `proposals.chains`. The
 * wasm must be initialized first (`loadCore()` in a browser test).
 */
import { multisigGenesis, type Genesis } from '../core/kernel';
import type { MultisigShellDeps, RpcReply } from './handler';
import { memoryStorage, offlineDeps } from './accounts.testing';
import { machines, type MembersMachine } from './machines.svelte';

/** What the chains say of an address being added. */
export type SignerSays = 'vela' | 'contract' | 'wallet' | null;

export interface WorldSafe {
	deployed: boolean;
	owners: string[];
	threshold: number;
	nonce?: number;
}

export interface MembersWorld {
	/** The multisig's address. */
	safe: string;
	/** Its Safe on each network this world answers for. */
	chains: Record<number, WorldSafe>;
	/** What each other address is (lowercase or any case); unlisted: nobody answers. */
	signers: Record<string, SignerSays>;
	/** Every call asked, in order. */
	asked: { chainId: number; method: string; params: unknown }[];
}

const NONCE = '0xaffed0e0';
const GET_OWNERS = '0xa0e67e2b';
const GET_THRESHOLD = '0xe75235b8';

const word = (value: bigint | number | string) => BigInt(value).toString(16).padStart(64, '0');
const addressWord = (address: string) => address.slice(2).toLowerCase().padStart(64, '0');

function says(world: MembersWorld, address: string): SignerSays | undefined {
	const key = Object.keys(world.signers).find((k) => k.toLowerCase() === address.toLowerCase());
	return key === undefined ? undefined : world.signers[key];
}

/** The world's answer to one call through the RPC pool; a throw is a node that did not answer. */
export function membersRpc(world: MembersWorld): MultisigShellDeps['rpc'] {
	return async (chainId, method, params): Promise<RpcReply> => {
		world.asked.push({ chainId, method, params });
		const safe = world.chains[chainId];
		if (method === 'eth_getCode') {
			const [address] = params as [string];
			if (address.toLowerCase() === world.safe.toLowerCase()) {
				if (!safe) throw new Error(`no network ${chainId}`);
				return { result: safe.deployed ? '0x6080604052' : '0x' };
			}
			const kind = says(world, address);
			if (!kind) throw new Error('nobody answered');
			return { result: kind === 'contract' ? '0x6080604052' : '0x' };
		}
		if (method === 'eth_call') {
			const [{ to, data }] = params as [{ to: string; data: string }];
			if (to.toLowerCase() === world.safe.toLowerCase()) {
				if (!safe?.deployed) throw new Error(`not deployed on ${chainId}`);
				if (data.startsWith(NONCE)) return { result: `0x${word(safe.nonce ?? 0)}` };
				if (data.startsWith(GET_THRESHOLD)) return { result: `0x${word(safe.threshold)}` };
				if (data.startsWith(GET_OWNERS))
					return {
						result: `0x${word(32)}${word(safe.owners.length)}${safe.owners.map(addressWord).join('')}`
					};
				throw new Error(`no such call: ${data.slice(0, 10)}`);
			}
			// Vela's passkey signer, asked whether it is configured for an address.
			const asked = Object.keys(world.signers).find((k) =>
				data.toLowerCase().includes(k.slice(2).toLowerCase())
			);
			const kind = asked === undefined ? null : world.signers[asked];
			if (!kind) throw new Error('nobody answered');
			return { result: `0x${(kind === 'vela' ? '1' : '0').repeat(128)}` };
		}
		throw new Error(`not in this world: ${method}`);
	};
}

/** A multisig of `owners` (a real genesis: a network it is not deployed on starts with these). */
export function worldMultisig(
	owners: string[],
	threshold: number
): { address: string; genesis: Genesis } {
	const made = multisigGenesis(owners, threshold, 'Test', '0x1');
	return { address: made.address, genesis: made.genesis };
}

/** The host's members machine, over `world` (and `over`, the rest of the shell). */
export function startMembers(
	world: MembersWorld,
	over: Partial<MultisigShellDeps> = {}
): MembersMachine {
	return machines.startMembers(offlineDeps(memoryStorage(), { rpc: membersRpc(world), ...over }));
}

/** Settles the machine's reads (each answers on a later turn). */
export async function membersSettled(): Promise<void> {
	for (let i = 0; i < 20; i += 1) await new Promise((r) => setTimeout(r, 0));
}
