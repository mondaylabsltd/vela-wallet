/**
 * Read the on-chain Vela wallet registry, one page at a time.
 *
 * Wraps the raw `getKeysByRpId` view into a paginated, display-ready shape:
 * derives each wallet's checksummed Safe address from `walletRef` and converts
 * the creation timestamp to unix milliseconds. Everything here is pure decode +
 * formatting; the network read happens through the viem client passed in.
 */
import { getAddress, type PublicClient } from 'viem';
import { CONTRACT_ADDRESS, REGISTRY_ABI, RP_ID } from './chain';

export interface WalletRecord {
	/** Relying-party id — always `getvela.app` for this registry. */
	rpId: string;
	/** User-chosen passkey label. May be empty; treat as untrusted display text. */
	name: string;
	credentialId: string;
	initialCredentialId: string;
	/** Full bytes32 reference as stored on-chain. */
	walletRef: `0x${string}`;
	/** Checksummed Safe address — the low 20 bytes of `walletRef`. */
	walletAddress: `0x${string}`;
	/** Uncompressed P-256 public key, `0x04`-prefixed (65 bytes). */
	publicKey: `0x${string}`;
	/** Creation time in unix milliseconds. */
	createdAt: number;
}

export interface WalletPage {
	total: number;
	records: WalletRecord[];
}

/** The raw tuple shape viem decodes each record into. */
export interface RawRecord {
	rpId: string;
	credentialId: string;
	walletRef: `0x${string}`;
	publicKey: `0x${string}`;
	name: string;
	initialCredentialId: string;
	metadata: `0x${string}`;
	createdAt: bigint;
}

// Max unix seconds that stays inside the valid JS Date range (±8.64e15 ms).
const MAX_SAFE_SECONDS = 8_640_000_000_000;

/** Turn a raw on-chain tuple into a display-ready record. */
export function formatRecord(r: RawRecord): WalletRecord {
	// createdAt is untrusted RPC data: clamp to the valid Date range so a bogus
	// (e.g. 2^64) timestamp can't throw "Invalid time value" and blank the page.
	const secs = Number(r.createdAt);
	const safeSecs = Number.isFinite(secs) ? Math.min(Math.max(secs, 0), MAX_SAFE_SECONDS) : 0;
	return {
		rpId: r.rpId,
		name: r.name,
		credentialId: r.credentialId,
		initialCredentialId: r.initialCredentialId,
		walletRef: r.walletRef,
		// The Safe address is the low 20 bytes of the 32-byte walletRef.
		walletAddress: getAddress(`0x${r.walletRef.slice(-40)}`),
		publicKey: r.publicKey,
		createdAt: safeSecs * 1000
	};
}

/**
 * Fetch one page of wallet records.
 *
 * `page` is 1-based. `desc` newest-first. Returns the running total (so callers
 * can compute page counts) alongside the formatted records. Throws if every RPC
 * endpoint is unreachable — callers should surface that as a retryable error, not
 * as "no wallets".
 */
export async function fetchWalletPage(
	client: PublicClient,
	page: number,
	pageSize: number,
	desc: boolean
): Promise<WalletPage> {
	const offset = Math.max(0, (page - 1) * pageSize);
	const [total, records] = await client.readContract({
		address: CONTRACT_ADDRESS,
		abi: REGISTRY_ABI,
		functionName: 'getKeysByRpId',
		args: [RP_ID, BigInt(offset), BigInt(pageSize), desc]
	});
	return {
		total: Number(total),
		records: (records as readonly RawRecord[]).map(formatRecord)
	};
}
