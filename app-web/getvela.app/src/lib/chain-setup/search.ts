/**
 * Chain search over the chain-data service's index — by id, name, short name
 * or gas-coin symbol — so a person can type "arc" or "USDC" instead of
 * knowing the number.
 *
 * The index is ~2,600 small records; ranking them here is simpler and more
 * predictable than a fuzzy library, and it means "5042" finds chain 5042
 * first, not "chain 50420" because it shares more characters.
 */
const INDEX_URL = 'https://ethereum-data.getvela.app/index/fuse-chains.json';

export interface ChainHit {
	chainId: number;
	name: string;
	shortName: string;
	nativeCurrencySymbol: string;
}

let cache: Promise<ChainHit[]> | null = null;

export function loadIndex(fetcher: typeof fetch = fetch): Promise<ChainHit[]> {
	cache ??= fetcher(INDEX_URL, { headers: { accept: 'application/json' } })
		.then(async (res) => {
			if (!res.ok) throw new Error(`index ${res.status}`);
			const json = (await res.json()) as { data?: ChainHit[] };
			return Array.isArray(json.data) ? json.data : [];
		})
		.catch(() => {
			cache = null; // let the next keystroke retry
			return [];
		});
	return cache;
}

/** Rank: exact id, id prefix, exact symbol/short name, name prefix, then substring. Lower is better. */
export function rank(hit: ChainHit, q: string): number | null {
	const query = q.trim().toLowerCase();
	if (!query) return null;
	const id = String(hit.chainId);
	const name = hit.name.toLowerCase();
	const short = hit.shortName.toLowerCase();
	const sym = hit.nativeCurrencySymbol.toLowerCase();
	if (id === query) return 0;
	if (/^\d+$/.test(query)) return id.startsWith(query) ? 1 : null;
	// A chain's own name or short name is a stronger claim than sharing a gas
	// coin: "eth" is Ethereum before it is every chain that pays gas in ETH.
	if (short === query || name === query) return 2;
	if (sym === query) return 3;
	if (name.startsWith(query) || short.startsWith(query)) return 4;
	if (sym.startsWith(query)) return 5;
	if (name.includes(query)) return 6;
	return null;
}

/** Testnets sink below mainnets at equal rank — the page's audience is operators of real chains first. */
function isTestnet(hit: ChainHit): boolean {
	return /testnet|sepolia|devnet|goerli/i.test(hit.name);
}

export function search(index: ChainHit[], q: string, limit = 8): ChainHit[] {
	const scored: { hit: ChainHit; score: number }[] = [];
	for (const hit of index) {
		const score = rank(hit, q);
		if (score !== null) scored.push({ hit, score: score * 2 + (isTestnet(hit) ? 1 : 0) });
	}
	scored.sort(
		(a, b) =>
			a.score - b.score || a.hit.name.length - b.hit.name.length || a.hit.chainId - b.hit.chainId
	);
	return scored.slice(0, limit).map((s) => s.hit);
}
