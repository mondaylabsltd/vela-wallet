/**
 * Read the on-chain Vela wallet registry, one page at a time.
 *
 * Two sources, one display shape. The current registry
 * (`WebAuthnP256PublicKeyRegistry`) stores each wallet as an immutable group:
 * the wallet address and name live in the group's opaque metadata blob, and its
 * founding passkeys are the group members (each with its P-256 public key and
 * attestation). The legacy index (`WebAuthnP256PublicKeyIndex`) is the older
 * server-signed store, kept readable so pre-migration wallets stay visible.
 *
 * Everything here is pure decode + formatting; the network reads happen through
 * the chain client passed in.
 */
import { getAddress, hexToString, type ChainClient } from './evm';
import {
	CONTRACT_ADDRESS,
	LEGACY_CONTRACT_ADDRESS,
	LEGACY_REGISTRY_ABI,
	REGISTRY_ABI_V2,
	RP_ID
} from './chain';

export type RegistrySource = 'v2' | 'legacy';

/** One wallet, in a shape both registries map onto. */
/** One founding passkey of a wallet (a group member entry), with its own
 *  WebAuthn signals. A wallet has one to {@link MAX_MEMBERS} of these. */
export interface MemberPasskey {
	/** Immutable entry id in the registry. */
	entryId?: number;
	/** Uncompressed P-256 point, `0x04`-prefixed (65 bytes). */
	publicKey: `0x${string}`;
	/** The 20-byte versioned attestation, or `0x` when none was stored. */
	attestation: `0x${string}`;
	/** WebAuthn credential id, base64url-encoded for display (or empty). */
	credentialId: string;
	/** Browser-reported authenticatorAttachment ("platform" / "cross-platform"). */
	authenticatorAttachment: string;
	/** Browser-reported transports (e.g. "hybrid,internal"). */
	transports: string;
}

export interface WalletRecord {
	source: RegistrySource;
	/** Relying-party id — always `getvela.app` for this registry. */
	rpId: string;
	/** User-chosen passkey label. May be empty; treat as untrusted display text. */
	name: string;
	/** Checksummed Safe wallet address. */
	walletAddress: `0x${string}`;
	/** The founding (first) member passkey public key, `0x04`-prefixed. Kept for
	 *  row identity and the legacy index; v2 detail reads {@link members}. */
	publicKey: `0x${string}`;
	/** Creation time in unix milliseconds. */
	createdAt: number;

	// v2 only ---------------------------------------------------------------
	/** Immutable group id in the registry. */
	unitId?: number;
	/** The one-time group key that closed this group, `0x04`-prefixed. */
	groupPublicKey?: `0x${string}`;
	/** Number of founding passkeys in the group. */
	memberCount?: number;
	/** Safe deployment version recorded in the metadata, e.g. `safe-1.4.1`. */
	walletVersion?: string;
	/** Every founding passkey of the wallet, in canonical order — a wallet may
	 *  have more than one (multi-key wallets). */
	members?: MemberPasskey[];

	// legacy only -----------------------------------------------------------
	/** The legacy record's WebAuthn credential id (base64url or raw). */
	credentialId?: string;
	walletRef?: `0x${string}`;
}

export interface WalletPage {
	total: number;
	records: WalletRecord[];
}

// Max unix seconds that stays inside the valid JS Date range (±8.64e15 ms).
const MAX_SAFE_SECONDS = 8_640_000_000_000;

/** createdAt is untrusted RPC data: clamp to the valid Date range so a bogus
 *  (e.g. 2^64) timestamp can't throw "Invalid time value" and blank the page. */
function toMillis(createdAt: bigint): number {
	const secs = Number(createdAt);
	const safe = Number.isFinite(secs) ? Math.min(Math.max(secs, 0), MAX_SAFE_SECONDS) : 0;
	return safe * 1000;
}

// ── Current registry (v2) ───────────────────────────────────────────────────

/** Mirrors the contract's MAX_MEMBERS: the most passkeys one wallet can have. */
const MAX_MEMBERS = 7;

/** The metadata blob a vela wallet writes into its group (see vela-core's
 *  `registry_metadata`). Fields are untrusted display text. */
interface WalletMetadata {
	address?: string;
	walletVersion?: string;
	name?: string;
}

/** Best-effort decode of the opaque group metadata; never throws. */
export function decodeMetadata(metadata: `0x${string}`): WalletMetadata {
	if (!metadata || metadata === '0x') return {};
	try {
		const json = JSON.parse(hexToString(metadata));
		const names = Array.isArray(json.key_names) ? json.key_names : [];
		return {
			address: typeof json.address === 'string' ? json.address : undefined,
			walletVersion: typeof json.wallet_version === 'string' ? json.wallet_version : undefined,
			name: typeof names[0] === 'string' ? names[0] : undefined
		};
	} catch {
		return {};
	}
}

interface RawUnit {
	rpId: string;
	metadata: `0x${string}`;
	groupPublicKey: `0x${string}`;
	contentHash: `0x${string}`;
	memberCount: number;
	createdAt: bigint;
}

interface RawEntry {
	publicKey: `0x${string}`;
	attestation: `0x${string}`;
	/** Store-only WebAuthn signals (see the registry's Entry struct). */
	credentialId: `0x${string}`;
	authenticatorAttachment: `0x${string}`;
	transports: `0x${string}`;
	createdAt: bigint;
}

async function fetchWalletPageV2(
	client: ChainClient,
	page: number,
	pageSize: number,
	desc: boolean
): Promise<WalletPage> {
	const offset = Math.max(0, (page - 1) * pageSize);
	const [total, unitIds] = (await client.readContract({
		address: CONTRACT_ADDRESS,
		abi: REGISTRY_ABI_V2,
		functionName: 'getGroupsByRpId',
		args: [RP_ID, BigInt(offset), BigInt(pageSize), desc]
	})) as unknown as [bigint, readonly bigint[]];

	if (unitIds.length === 0) return { total: Number(total), records: [] };

	// One frozen record and ALL founding members per group, all in flight
	// together (the fallback client fans them across the RPC pool). A wallet
	// may have several passkeys, so read up to the contract's member cap.
	const units = (await Promise.all(
		unitIds.map((id) =>
			client.readContract({
				address: CONTRACT_ADDRESS,
				abi: REGISTRY_ABI_V2,
				functionName: 'getUnit',
				args: [id]
			})
		)
	)) as unknown as RawUnit[];

	const memberPages = (await Promise.all(
		unitIds.map((id) =>
			client.readContract({
				address: CONTRACT_ADDRESS,
				abi: REGISTRY_ABI_V2,
				functionName: 'getGroupMembers',
				args: [id, 0n, BigInt(MAX_MEMBERS), false]
			})
		)
	)) as unknown as [bigint, bigint[], RawEntry[]][];

	const records: WalletRecord[] = unitIds.map((id, i) => {
		const unit = units[i];
		const meta = decodeMetadata(unit.metadata);
		const entryIds = memberPages[i]?.[1] ?? [];
		const entries = memberPages[i]?.[2] ?? [];
		const members: MemberPasskey[] = entries.map((e, mi) => ({
			entryId: Number(entryIds[mi] ?? 0n),
			publicKey: (e.publicKey ?? '0x') as `0x${string}`,
			attestation: (e.attestation ?? '0x') as `0x${string}`,
			credentialId: credentialIdToBase64url(e.credentialId),
			authenticatorAttachment: decodeTextHint(e.authenticatorAttachment),
			transports: decodeTextHint(e.transports)
		}));
		const address = meta.address ?? '0x0000000000000000000000000000000000000000';
		return {
			source: 'v2',
			rpId: unit.rpId,
			name: meta.name ?? '',
			walletAddress: safeChecksum(address),
			publicKey: (members[0]?.publicKey ?? '0x') as `0x${string}`,
			createdAt: toMillis(unit.createdAt),
			unitId: Number(id),
			groupPublicKey: unit.groupPublicKey,
			memberCount: Number(unit.memberCount),
			walletVersion: meta.walletVersion,
			members
		};
	});

	return { total: Number(total), records };
}

/** Decode a store-only text hint (authenticatorAttachment / transports) from
 *  its on-chain bytes. Untrusted display text; never throws. */
function decodeTextHint(value: `0x${string}` | undefined): string {
	if (!value || value === '0x') return '';
	try {
		return hexToString(value);
	} catch {
		return '';
	}
}

/** Render the WebAuthn credential id (on-chain raw bytes) as base64url — the
 *  form authenticators and the WebAuthn API use. Never throws. */
function credentialIdToBase64url(value: `0x${string}` | undefined): string {
	if (!value || value === '0x') return '';
	try {
		const hex = value.slice(2);
		let binary = '';
		for (let i = 0; i < hex.length; i += 2) {
			binary += String.fromCharCode(parseInt(hex.slice(i, i + 2), 16));
		}
		return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
	} catch {
		return '';
	}
}

function safeChecksum(address: string): `0x${string}` {
	try {
		return getAddress(address);
	} catch {
		return '0x0000000000000000000000000000000000000000';
	}
}

// ── Legacy index ─────────────────────────────────────────────────────────────

export interface RawLegacyRecord {
	rpId: string;
	credentialId: string;
	walletRef: `0x${string}`;
	publicKey: `0x${string}`;
	name: string;
	initialCredentialId: string;
	metadata: `0x${string}`;
	createdAt: bigint;
}

/** Turn a raw legacy tuple into the unified record. */
export function formatLegacyRecord(r: RawLegacyRecord): WalletRecord {
	return {
		source: 'legacy',
		rpId: r.rpId,
		name: r.name,
		// The Safe address is the low 20 bytes of the 32-byte walletRef.
		walletAddress: getAddress(`0x${r.walletRef.slice(-40)}`),
		publicKey: r.publicKey,
		createdAt: toMillis(r.createdAt),
		credentialId: r.credentialId,
		walletRef: r.walletRef
	};
}

async function fetchLegacyWalletPage(
	client: ChainClient,
	page: number,
	pageSize: number,
	desc: boolean
): Promise<WalletPage> {
	const offset = Math.max(0, (page - 1) * pageSize);
	const [total, records] = await client.readContract<[bigint, readonly RawLegacyRecord[]]>({
		address: LEGACY_CONTRACT_ADDRESS,
		abi: LEGACY_REGISTRY_ABI,
		functionName: 'getKeysByRpId',
		args: [RP_ID, BigInt(offset), BigInt(pageSize), desc]
	});
	return {
		total: Number(total),
		records: records.map(formatLegacyRecord)
	};
}

// ── Public API ───────────────────────────────────────────────────────────────

/**
 * Fetch one page of wallet records from the chosen registry (`v2` is the
 * current possession-proven registry and the default; `legacy` is the older
 * server-signed index, kept for pre-migration wallets).
 *
 * `page` is 1-based, `desc` newest-first. Returns the running total alongside
 * the formatted records. Throws if every RPC endpoint is unreachable — callers
 * should surface that as a retryable error, not as "no wallets".
 */
export function fetchWalletPage(
	client: ChainClient,
	page: number,
	pageSize: number,
	desc: boolean,
	source: RegistrySource = 'v2'
): Promise<WalletPage> {
	return source === 'legacy'
		? fetchLegacyWalletPage(client, page, pageSize, desc)
		: fetchWalletPageV2(client, page, pageSize, desc);
}
