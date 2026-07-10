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
import { gnosis } from 'viem/chains';

/** WebAuthn P-256 public-key index — the registry every Vela wallet writes to. */
export const CONTRACT_ADDRESS = '0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3' as const;

/** Relying-party id for Vela wallets. All getvela.app wallets are indexed under this. */
export const RP_ID = 'getvela.app';

/**
 * A fixed set of reputable public Gnosis RPC endpoints. This is deliberately a
 * hardcoded, vetted list rather than a remotely-fetched one: the whole point of
 * this page is "don't trust us — verify," so its root of trust (which nodes it
 * reads from) must not be a mutable third-party source that could be repointed
 * to attacker-controlled endpoints. Order is not significant — the fallback
 * transport tries the next one whenever a request fails or times out.
 */
const GNOSIS_RPCS = [
	'https://rpc.gnosischain.com',
	'https://rpc.gnosis.gateway.fm',
	'https://gnosis-rpc.publicnode.com',
	'https://rpc.ankr.com/gnosis',
	'https://gnosis-mainnet.public.blastapi.io',
	'https://gnosis.blockpi.network/v1/rpc/public',
	'https://gnosis.drpc.org',
	'https://1rpc.io/gnosis',
	'https://gnosis.oat.farm'
];

/**
 * Minimal ABI: the two read functions the site calls. `getKeysByRpId` returns the
 * running total plus a page of records; `getTotalCredentialsByRpId` is the cheap
 * count-only read.
 */
export const REGISTRY_ABI = [
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
