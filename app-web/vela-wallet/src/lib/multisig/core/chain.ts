/**
 * What a multisig is on a chain RIGHT NOW, read through the wallet's own RPC
 * pool — never through the genesis service, which is only how other devices
 * find things. Who may approve, from which nonce, decides what the wallet
 * signs, so it is read from the chain itself.
 */
import { checksumAddress } from '$lib/core/kernels';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { genesisMembers, type Genesis } from './kernel';

const GET_OWNERS = '0xa0e67e2b';
const GET_THRESHOLD = '0xe75235b8';
const NONCE = '0xaffed0e0';

/** The chain did not answer: nothing about the multisig can be said. */
export class ChainUnavailableError extends Error {
	constructor(
		readonly chainId: number,
		detail: string
	) {
		super(`chain ${chainId} unavailable: ${detail}`);
		this.name = 'ChainUnavailableError';
	}
}

async function rpc<T>(chainId: number, method: string, params: unknown[]): Promise<T> {
	const answer = await poolRpcCall(method, params, chainId);
	if (answer.error) throw new ChainUnavailableError(chainId, answer.error.message);
	return answer.result as T;
}

async function hasCode(chainId: number, address: string): Promise<boolean> {
	const code = await rpc<string>(chainId, 'eth_getCode', [address, 'latest']);
	return typeof code === 'string' && code !== '0x' && code !== '0x0';
}

function ethCall(chainId: number, to: string, data: string): Promise<string> {
	return rpc<string>(chainId, 'eth_call', [{ to, data }, 'latest']);
}

function word(hex: string, index: number): string {
	const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
	return clean.slice(index * 64, index * 64 + 64);
}

function decodeAddresses(hex: string): string[] {
	const offset = parseInt(word(hex, 0), 16) / 32;
	const count = parseInt(word(hex, offset), 16);
	const out: string[] = [];
	for (let i = 0; i < count && i < 64; i += 1) {
		// Checksummed, as every other address the wallet shows (ms-26).
		const raw = `0x${word(hex, offset + 1 + i).slice(24)}`;
		let shown = raw;
		try {
			shown = checksumAddress(raw);
		} catch {
			// the core not loaded: the address as the chain gave it
		}
		out.push(shown);
	}
	return out;
}

export interface ChainMembers {
	/** Whether the multisig has code on this chain. When it has not, the rest is what its genesis creates there. */
	deployed: boolean;
	owners: string[];
	threshold: number;
	nonce: bigint;
}

/**
 * Multisigs seen deployed, per chain (r4D-18): code does not go away, so a
 * deployed Safe's code is not read again on every refresh — nor, kept on
 * this device (round 5), on every page.
 */
const DEPLOYED_KEY = 'vela.multisigDeployed';
const DEPLOYED_CAP = 200;
const deployedSeen = new Set<string>(readDeployed());

function readDeployed(): string[] {
	try {
		const kept = JSON.parse(localStorage.getItem(DEPLOYED_KEY) ?? '[]');
		return Array.isArray(kept) ? kept.filter((k): k is string => typeof k === 'string') : [];
	} catch {
		return [];
	}
}

async function deployed(chainId: number, safe: string): Promise<boolean> {
	const key = `${chainId}:${safe.toLowerCase()}`;
	if (deployedSeen.has(key)) return true;
	const yes = await hasCode(chainId, safe);
	if (yes) {
		deployedSeen.add(key);
		try {
			localStorage.setItem(DEPLOYED_KEY, JSON.stringify([...deployedSeen].slice(-DEPLOYED_CAP)));
		} catch {
			// no storage: read again next page
		}
	}
	return yes;
}

/**
 * The owners and threshold last read per deployed multisig, at the nonce
 * they were read at (round 5, r4D-18): a Safe's own transactions move its
 * members, and every one of them moves its nonce — an unmoved nonce is
 * unmoved members, not read again. A module can change them without a
 * transaction (§3.5 says when one is enabled), so they are read again a
 * minute on regardless.
 */
const membersAt = new Map<
	string,
	{ nonce: bigint; owners: string[]; threshold: number; at: number }
>();
export const MEMBERS_FRESH_MS = 60_000;

/** The multisig's owners, threshold and nonce on `chainId`, from the chain (or, undeployed, its genesis). */
export async function readMembers(
	chainId: number,
	safe: string,
	genesis: Genesis
): Promise<ChainMembers> {
	if (!(await deployed(chainId, safe))) {
		const { owners, threshold } = genesisMembers(genesis);
		return { deployed: false, owners, threshold, nonce: 0n };
	}
	const key = `${chainId}:${safe.toLowerCase()}`;
	const nonce = BigInt(await ethCall(chainId, safe, NONCE));
	const kept = membersAt.get(key);
	if (kept && kept.nonce === nonce && now() - kept.at < MEMBERS_FRESH_MS) {
		return { deployed: true, owners: [...kept.owners], threshold: kept.threshold, nonce };
	}
	const [owners, threshold] = await Promise.all([
		ethCall(chainId, safe, GET_OWNERS),
		ethCall(chainId, safe, GET_THRESHOLD)
	]);
	const read = {
		nonce,
		owners: decodeAddresses(owners),
		threshold: Number(BigInt(threshold)),
		at: now()
	};
	membersAt.set(key, read);
	return { deployed: true, owners: [...read.owners], threshold: read.threshold, nonce };
}

let now = () => Date.now();

/** For tests: forget what was read (and read the clock from `clock`). */
export function resetChainReadsForTests(clock: () => number = () => Date.now()): void {
	now = clock;
	deployedSeen.clear();
	membersAt.clear();
	try {
		localStorage.removeItem(DEPLOYED_KEY);
	} catch {
		// no storage
	}
}
