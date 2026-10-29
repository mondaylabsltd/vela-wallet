/**
 * The catalog is published for the worker only once the wallet has read its
 * stored networks — the person's own node and "Only use my node" among them —
 * and the same table is not stored twice.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it, vi } from 'vitest';

const OWN_NODE = 'https://my-node.example/gnosis';
let releaseRead: () => void = () => {};

// The stored per-network overrides: Gnosis on the person's node alone. The
// read is held until the test lets it land, as a slow IndexedDB would.
vi.mock('$lib/services/records', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/services/records')>();
	return {
		...actual,
		loadCustomNetworks: async () => [],
		loadNetworkConfigs: () =>
			new Promise((resolve) => {
				releaseRead = () => resolve([{ chainId: 100, rpcURL: OWN_NODE, onlyMine: true }]);
			})
	};
});

const { publishExtChains, resetExtChainsForTests } = await import('./ext-chains');
const { CHAINS_KEY } = await import('../keys');

type Catalog = { chains: Record<string, { rpc: string[] }> };

function fakeArea() {
	const writes: Catalog[] = [];
	const set = vi.fn(async (items: Record<string, unknown>) => {
		writes.push(items[CHAINS_KEY] as Catalog);
	});
	vi.stubGlobal('chrome', { storage: { local: { set } } });
	return { writes, set };
}

describe('publishing the catalog for the worker', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
		resetExtChainsForTests();
	});

	it('waits for the stored networks, so the first table already has the person’s node', async () => {
		const { writes } = fakeArea();
		const published = publishExtChains();
		// Mounted, the read still out: nothing is stored yet — a table built
		// now would send a site's reads to the public nodes.
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(writes).toHaveLength(0);

		releaseRead();
		await published;
		expect(writes).toHaveLength(1);
		expect(writes[0].chains['100'].rpc).toEqual([OWN_NODE]);
	});

	it('does not store the same table again under a new timestamp', async () => {
		const { set } = fakeArea();
		releaseRead();
		await publishExtChains();
		await publishExtChains();
		expect(set).toHaveBeenCalledTimes(1);
	});
});
