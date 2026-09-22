/**
 * The networks built into the wallet, as the landing page lists them.
 *
 * This mirrors vela-core's `BUILTIN_CHAINS` (rust/crates/vela-core/src/app/
 * network_admin.rs), in the same order. `networks.test.ts` reads that file and
 * fails on any difference — and on a `home.networks.heading` whose number no
 * longer matches. The site said "12 networks" for a week after the wallet
 * shipped 24 (spec 080); this is the check that would have said so.
 */
export const BUILT_IN_NETWORKS = [
	{ name: 'Ethereum', chainId: 1 },
	{ name: 'BNB Chain', chainId: 56 },
	{ name: 'Polygon', chainId: 137 },
	{ name: 'Arbitrum', chainId: 42161 },
	{ name: 'Optimism', chainId: 10 },
	{ name: 'Base', chainId: 8453 },
	{ name: 'Avalanche', chainId: 43114 },
	{ name: 'Gnosis', chainId: 100 },
	{ name: 'Unichain', chainId: 130 },
	{ name: 'Tempo', chainId: 4217 },
	{ name: 'Monad', chainId: 143 },
	{ name: 'World Chain', chainId: 480 },
	{ name: 'Arc', chainId: 5042 },
	{ name: 'X Layer', chainId: 196 },
	{ name: 'Stable', chainId: 988 },
	{ name: 'Soneium', chainId: 1868 },
	{ name: 'MegaETH', chainId: 4326 },
	{ name: 'Robinhood Chain', chainId: 4663 },
	{ name: 'Mantle', chainId: 5000 },
	{ name: 'Kaia', chainId: 8217 },
	{ name: 'Celo', chainId: 42220 },
	{ name: 'Ink', chainId: 57073 },
	{ name: 'Plume', chainId: 98866 },
	{ name: 'XRPL EVM', chainId: 1440000 }
] as const;

/** Logos from the same chain-data host the wallet itself uses. */
export const chainLogo = (chainId: number) =>
	`https://ethereum-data.getvela.app/chainlogos/eip155-${chainId}.png`;
