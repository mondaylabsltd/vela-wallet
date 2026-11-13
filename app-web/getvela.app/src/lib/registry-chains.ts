/**
 * Where the registries live, and which nodes the site reads them from.
 *
 * Deliberately free of viem: the homepage reads two counts from here with plain
 * JSON-RPC and must not pull a chain library into the landing page's bundle.
 */

/**
 * A fixed set of reputable public Gnosis RPC endpoints. This is deliberately a
 * hardcoded, vetted list rather than a remotely-fetched one: the whole point of
 * the registry page is "don't trust us — verify," so its root of trust (which
 * nodes it reads from) must not be a mutable third-party source that could be
 * repointed to attacker-controlled endpoints. Order is not significant — the
 * fallback transport tries the next one whenever a request fails or times out.
 */
export const GNOSIS_RPCS = [
	'https://rpc.gnosischain.com',
	'https://rpc.gnosis.gateway.fm',
	'https://gnosis-rpc.publicnode.com',
	'https://rpc.ankr.com/gnosis',
	'https://gnosis-mainnet.public.blastapi.io',
	'https://gnosis.blockpi.network/v1/rpc/public',
	'https://gnosis.drpc.org',
	'https://1rpc.io/gnosis',
	'https://gnosis.oat.farm'
] as const;

/**
 * `SafeGenesisRegistry` v4: a neutral, write-once record of how each Safe was
 * created (factory, singleton, `setup(...)` initializer, salt nonce), with a
 * name. Vela records every multisig account here so any member's device can
 * find it. It is deployed through the deterministic CREATE2 deployer, so it has
 * this same address on every chain — which is exactly why "is there code here?"
 * has to be asked per chain before a list is read: an empty answer from an
 * address with no contract is not "no multisigs".
 */
export const GENESIS_REGISTRY_ADDRESS = '0xF412388bF908E510066d3Be5c704bd9A327f945d' as const;

export type MultisigChainKey = 'gnosis' | 'ethereum';

export interface MultisigChain {
	key: MultisigChainKey;
	chainId: number;
	label: string;
	/** A testnet: offered, clearly labelled, never counted as production data. */
	testnet: boolean;
	/** Block explorer base, no trailing slash. */
	explorer: string;
	/** Vetted public RPCs — hardcoded for the same reason as {@link GNOSIS_RPCS}. */
	rpcs: readonly string[];
}

/** Production only (owner, 2026-10-29: the site shows no testnet). Gnosis is
 *  where Vela's service writes; Ethereum is where a wallet's user may write at
 *  their own cost. */
export const MULTISIG_CHAINS: readonly MultisigChain[] = [
	{
		key: 'gnosis',
		chainId: 100,
		label: 'Gnosis',
		testnet: false,
		explorer: 'https://gnosisscan.io',
		rpcs: GNOSIS_RPCS
	},
	{
		key: 'ethereum',
		chainId: 1,
		label: 'Ethereum',
		testnet: false,
		explorer: 'https://etherscan.io',
		rpcs: [
			'https://ethereum-rpc.publicnode.com',
			'https://eth.drpc.org',
			'https://eth-mainnet.public.blastapi.io',
			'https://rpc.mevblocker.io',
			'https://eth.blockrazor.xyz',
			'https://1rpc.io/eth'
		]
	}
];

export const DEFAULT_MULTISIG_CHAIN: MultisigChainKey = 'gnosis';

export function multisigChain(key: MultisigChainKey): MultisigChain {
	return MULTISIG_CHAINS.find((c) => c.key === key) ?? MULTISIG_CHAINS[0];
}

/** Parse a `?chain=` value; anything unknown is the default (production) chain. */
export function readMultisigChain(v: string | null): MultisigChainKey {
	return MULTISIG_CHAINS.some((c) => c.key === v)
		? (v as MultisigChainKey)
		: DEFAULT_MULTISIG_CHAIN;
}
