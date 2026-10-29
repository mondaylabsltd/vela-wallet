/**
 * Direct, browser-side access to the Gnosis chain.
 *
 * Every Vela wallet writes its passkey (WebAuthn P-256) public key to an on-chain
 * index when it is created. This module reads that index straight from public
 * Gnosis RPCs — no Vela server sits in the path — so the numbers and records the
 * site shows are checkable against the chain by anyone.
 *
 * viem's `fallback` transport rotates through the RPC list on error/timeout, so a
 * single dead endpoint never breaks a read.
 */
import { createPublicClient, fallback, http, type Abi, type PublicClient } from 'viem';
import { gnosis, mainnet } from 'viem/chains';
import { GNOSIS_RPCS, multisigChain, type MultisigChainKey } from './registry-chains';

/**
 * The current registry: `WebAuthnP256PublicKeyRegistry`, a possession-proven
 * public-key registry. Every entry is written with a WebAuthn signature that
 * proves the writer holds the passkey — unlike the legacy index below, which a
 * server wrote on the wallet's behalf. A vela wallet is one immutable "group"
 * (unit) here: its founding passkeys are the members, and its opaque metadata
 * blob carries the wallet address, name, and Safe version.
 *
 * Each entry also records the WebAuthn signals that identify the authenticator:
 * the versioned attestation (AAGUID + authenticatorData flags), the credential
 * id, and the browser-reported hints (authenticatorAttachment, transports).
 * These are store-only display data — never part of any signed binding — so
 * the wallet address a passkey derives is unaffected by them.
 */
export const CONTRACT_ADDRESS = '0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9' as const;

/**
 * The legacy index (`WebAuthnP256PublicKeyIndex`): a server-signed store, kept
 * readable so wallets created before the migration are still visible. New
 * wallets are written only to {@link CONTRACT_ADDRESS}.
 */
export const LEGACY_CONTRACT_ADDRESS = '0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3' as const;

/** Relying-party id for Vela wallets. All getvela.app wallets are indexed under this. */
export const RP_ID = 'getvela.app';

/**
 * The current registry's read surface used by the site: list a page of the
 * groups (wallets) under an rpId, read one group's frozen record, and read one
 * group's members (each an entry with its public key + attestation). Counts:
 * `getTotalUnits` is the wallet count, `getTotalEntries` the passkey count.
 */
export const REGISTRY_ABI_V2 = [
	{
		type: 'function',
		name: 'getTotalUnits',
		stateMutability: 'view',
		inputs: [],
		outputs: [{ name: '', type: 'uint256' }]
	},
	{
		type: 'function',
		name: 'getTotalEntries',
		stateMutability: 'view',
		inputs: [],
		outputs: [{ name: '', type: 'uint256' }]
	},
	{
		type: 'function',
		name: 'getGroupsByRpId',
		stateMutability: 'view',
		inputs: [
			{ name: 'rpId', type: 'string' },
			{ name: 'offset', type: 'uint256' },
			{ name: 'limit', type: 'uint256' },
			{ name: 'desc', type: 'bool' }
		],
		outputs: [
			{ name: 'total', type: 'uint256' },
			{ name: 'unitIds', type: 'uint256[]' }
		]
	},
	{
		type: 'function',
		name: 'getUnit',
		stateMutability: 'view',
		inputs: [{ name: 'unitId', type: 'uint256' }],
		outputs: [
			{
				name: '',
				type: 'tuple',
				components: [
					{ name: 'rpId', type: 'string' },
					{ name: 'metadata', type: 'bytes' },
					{ name: 'groupPublicKey', type: 'bytes' },
					{ name: 'contentHash', type: 'bytes32' },
					{ name: 'memberCount', type: 'uint32' },
					{ name: 'createdAt', type: 'uint256' }
				]
			}
		]
	},
	{
		type: 'function',
		name: 'getGroupMembers',
		stateMutability: 'view',
		inputs: [
			{ name: 'unitId', type: 'uint256' },
			{ name: 'offset', type: 'uint256' },
			{ name: 'limit', type: 'uint256' },
			{ name: 'desc', type: 'bool' }
		],
		outputs: [
			{ name: 'total', type: 'uint256' },
			{ name: 'entryIds', type: 'uint256[]' },
			{
				name: 'entries',
				type: 'tuple[]',
				components: [
					{ name: 'publicKey', type: 'bytes' },
					{ name: 'attestation', type: 'bytes' },
					// Store-only WebAuthn signals captured at first sight (see the
					// registry's Entry struct); order MUST mirror the contract.
					{ name: 'credentialId', type: 'bytes' },
					{ name: 'authenticatorAttachment', type: 'bytes' },
					{ name: 'transports', type: 'bytes' },
					{ name: 'createdAt', type: 'uint256' }
				]
			}
		]
	}
] as const satisfies Abi;

/**
 * Legacy index ABI: `getKeysByRpId` returns the running total plus a page of
 * records; `getTotalCredentialsByRpId` is the cheap count-only read.
 */
export const LEGACY_REGISTRY_ABI = [
	{
		type: 'function',
		name: 'getKeysByRpId',
		stateMutability: 'view',
		inputs: [
			{ name: 'rpId', type: 'string' },
			{ name: 'offset', type: 'uint256' },
			{ name: 'limit', type: 'uint256' },
			{ name: 'desc', type: 'bool' }
		],
		outputs: [
			{ name: 'total', type: 'uint256' },
			{
				name: 'records',
				type: 'tuple[]',
				components: [
					{ name: 'rpId', type: 'string' },
					{ name: 'credentialId', type: 'string' },
					{ name: 'walletRef', type: 'bytes32' },
					{ name: 'publicKey', type: 'bytes' },
					{ name: 'name', type: 'string' },
					{ name: 'initialCredentialId', type: 'string' },
					{ name: 'metadata', type: 'bytes' },
					{ name: 'createdAt', type: 'uint256' }
				]
			}
		]
	},
	{
		type: 'function',
		name: 'getTotalCredentialsByRpId',
		stateMutability: 'view',
		inputs: [{ name: 'rpId', type: 'string' }],
		outputs: [{ name: '', type: 'uint256' }]
	}
] as const satisfies Abi;

/** Build a Gnosis client that fails over across the vetted RPC endpoints. */
export function makeGnosisClient(rpcs: readonly string[] = GNOSIS_RPCS): PublicClient {
	return createPublicClient({
		chain: gnosis,
		transport: fallback(
			rpcs.map((url) => http(url, { timeout: 8_000 })),
			{ retryCount: 1 }
		)
	});
}

// ── Safe Genesis Registry (multisig accounts) ───────────────────────────────
// Address and chains: see registry-chains.ts.

/**
 * One `Genesis` record. The name is declared `bytes` (the ABI encoding is the
 * same as `string`) so a name that is not valid UTF-8 — the contract does not
 * check — is shown for what it is instead of silently "repaired" by the decoder.
 * Field order MUST mirror the contract's struct.
 */
const GENESIS_TUPLE = [
	{ name: 'factory', type: 'address' },
	{ name: 'singleton', type: 'address' },
	{ name: 'renamedAt', type: 'uint64' },
	{ name: 'saltNonce', type: 'uint256' },
	{ name: 'initializer', type: 'bytes' },
	{ name: 'proxyCreationCodeHash', type: 'bytes32' },
	{ name: 'recorder', type: 'address' },
	{ name: 'recordedAt', type: 'uint64' },
	{ name: 'index', type: 'uint32' },
	{ name: 'name', type: 'bytes' }
] as const;

/** The read surface of the registry the site uses. */
export const GENESIS_REGISTRY_ABI = [
	{
		type: 'function',
		name: 'totalRecords',
		stateMutability: 'view',
		inputs: [],
		outputs: [{ name: '', type: 'uint256' }]
	},
	{
		type: 'function',
		name: 'isRecorded',
		stateMutability: 'view',
		inputs: [{ name: 'safe', type: 'address' }],
		outputs: [{ name: '', type: 'bool' }]
	},
	{
		type: 'function',
		name: 'genesisPage',
		stateMutability: 'view',
		inputs: [
			{ name: 'offset', type: 'uint256' },
			{ name: 'limit', type: 'uint256' },
			{ name: 'desc', type: 'bool' }
		],
		outputs: [
			{ name: 'total', type: 'uint256' },
			{ name: 'safes', type: 'address[]' },
			{ name: 'records', type: 'tuple[]', components: GENESIS_TUPLE }
		]
	}
] as const satisfies Abi;

const VIEM_CHAINS = { gnosis, ethereum: mainnet } as const;

/** A failover client for one multisig chain. JSON-RPC batching is on so a
 *  page's block-timestamp lookups go out as one request, not a hundred. */
export function makeChainClient(key: MultisigChainKey): PublicClient {
	const chain = multisigChain(key);
	return createPublicClient({
		chain: VIEM_CHAINS[chain.key],
		transport: fallback(
			chain.rpcs.map((url) => http(url, { timeout: 10_000, batch: { batchSize: 50 } })),
			{ retryCount: 1 }
		)
	}) as PublicClient;
}
