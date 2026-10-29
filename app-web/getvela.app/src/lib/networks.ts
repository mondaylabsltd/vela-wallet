/**
 * The networks built into the wallet, as the landing page lists them.
 *
 * This mirrors vela-core's `BUILTIN_CHAINS` (rust/crates/vela-core/src/app/
 * network_admin.rs) — its mainnets, in the same order, the order every list in
 * the wallet shows them (owner decision 2026-10-29). Twenty are in every
 * person's lists from the start (`shownByDefault`, the core's
 * `shown_by_default`); the last four are Vela networks all the same, added
 * back in Settings → Add network. `networks.test.ts` reads that file and fails
 * on any difference — and on a `home.networks.heading` whose number no longer
 * matches the shown ones. The site said "12 networks" for a week after the
 * wallet shipped 24 (spec 080); this is the check that would have said so.
 */
export const BUILT_IN_NETWORKS = [
	{ name: 'Ethereum', chainId: 1, shownByDefault: true },
	{ name: 'BNB Chain', chainId: 56, shownByDefault: true },
	{ name: 'Base', chainId: 8453, shownByDefault: true },
	{ name: 'Polygon', chainId: 137, shownByDefault: true },
	{ name: 'Arbitrum', chainId: 42161, shownByDefault: true },
	{ name: 'Avalanche', chainId: 43114, shownByDefault: true },
	{ name: 'Optimism', chainId: 10, shownByDefault: true },
	{ name: 'Monad', chainId: 143, shownByDefault: true },
	{ name: 'Robinhood Chain', chainId: 4663, shownByDefault: true },
	{ name: 'Arc', chainId: 5042, shownByDefault: true },
	{ name: 'X Layer', chainId: 196, shownByDefault: true },
	{ name: 'Mantle', chainId: 5000, shownByDefault: true },
	{ name: 'Gnosis', chainId: 100, shownByDefault: true },
	{ name: 'Tempo', chainId: 4217, shownByDefault: true },
	{ name: 'World Chain', chainId: 480, shownByDefault: true },
	{ name: 'Celo', chainId: 42220, shownByDefault: true },
	{ name: 'Unichain', chainId: 130, shownByDefault: true },
	{ name: 'Ink', chainId: 57073, shownByDefault: true },
	{ name: 'Soneium', chainId: 1868, shownByDefault: true },
	{ name: 'Kaia', chainId: 8217, shownByDefault: true },
	{ name: 'Stable', chainId: 988, shownByDefault: false },
	{ name: 'MegaETH', chainId: 4326, shownByDefault: false },
	{ name: 'Plume', chainId: 98866, shownByDefault: false },
	{ name: 'XRPL EVM', chainId: 1440000, shownByDefault: false }
] as const;

/** The networks in every person's lists from the start: the ones the landing page draws and counts. */
export const SHOWN_NETWORKS = BUILT_IN_NETWORKS.filter((n) => n.shownByDefault);

/** Logos from the same chain-data host the wallet itself uses. */
export const chainLogo = (chainId: number) =>
	`https://ethereum-data.getvela.app/chainlogos/eip155-${chainId}.png`;
