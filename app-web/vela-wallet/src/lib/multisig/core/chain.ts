/**
 * What a multisig is on a chain RIGHT NOW, read through the wallet's own RPC
 * pool — never through the genesis service, which is only how other devices
 * find things. Who may approve, from which nonce, decides what the wallet
 * signs, so it is read from the chain itself.
 */
import { checksumAddress } from '$lib/core/kernels';
import { poolRpcCall } from '$lib/services/rpc-pool';
import {
	genesisMembers,
	multisigContracts,
	safeContracts,
	type Genesis,
	type SafeContracts
} from './kernel';

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

export async function hasCode(chainId: number, address: string): Promise<boolean> {
	const code = await rpc<string>(chainId, 'eth_getCode', [address, 'latest']);
	return typeof code === 'string' && code !== '0x' && code !== '0x0';
}

/**
 * Whether `address` is a smart account on `chainId` — code there that is not
 * an EIP-7702 delegation (`0xef0100 ++ implAddr`, 23 bytes: a person's own
 * wallet with smart-account features, which still signs with its key, as
 * `recipient-risk.ts` reads it). Who signs how is decided on this, not on
 * `hasCode`: a delegated EOA's wallet signs for it.
 */
export async function hasContractCode(chainId: number, address: string): Promise<boolean> {
	const code = await rpc<string>(chainId, 'eth_getCode', [address, 'latest']);
	if (typeof code !== 'string' || code === '0x' || code === '0x0') return false;
	return !/^0xef0100[0-9a-fA-F]{40}$/i.test(code);
}

/** Verdicts read this session: a contract's code does not come and go. */
const contractVerdicts = new Map<number, SafeContracts>();

/**
 * Whether `chainId` holds the Safe v1.4.1 contracts a new multisig needs —
 * read here (`eth_getCode` of each), judged by the core (`safeContracts`).
 * A chain that does not answer is `unknown`, and asked again next time.
 */
export async function safeContractsOn(chainId: number): Promise<SafeContracts> {
	const known = contractVerdicts.get(chainId);
	if (known) return known;
	const codes = await Promise.all(
		multisigContracts().map(async ({ address }) => {
			try {
				const code = await rpc<string>(chainId, 'eth_getCode', [address, 'latest']);
				return typeof code === 'string' ? code : null;
			} catch {
				return null;
			}
		})
	);
	const verdict = safeContracts(codes);
	if (verdict.kind !== 'unknown') contractVerdicts.set(chainId, verdict);
	return verdict;
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
 * Multisigs seen deployed, per chain, this session (r4D-18): code does not go
 * away, so a deployed Safe's code is not read again on every refresh.
 */
const deployedSeen = new Set<string>();

async function deployed(chainId: number, safe: string): Promise<boolean> {
	const key = `${chainId}:${safe.toLowerCase()}`;
	if (deployedSeen.has(key)) return true;
	const yes = await hasCode(chainId, safe);
	if (yes) deployedSeen.add(key);
	return yes;
}

/** The multisig's nonce on `chainId` now (0 where it is not deployed). */
export async function readNonce(chainId: number, safe: string): Promise<bigint> {
	if (!(await deployed(chainId, safe))) return 0n;
	return BigInt(await ethCall(chainId, safe, NONCE));
}

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
	const [owners, threshold, nonce] = await Promise.all([
		ethCall(chainId, safe, GET_OWNERS),
		ethCall(chainId, safe, GET_THRESHOLD),
		ethCall(chainId, safe, NONCE)
	]);
	return {
		deployed: true,
		owners: decodeAddresses(owners),
		threshold: Number(BigInt(threshold)),
		nonce: BigInt(nonce)
	};
}
