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

const { publishExtChains, resetExtChainsForTests, setExtAccountChains } =
	await import('./ext-chains');
const { ACCOUNT_CHAINS_KEY, CHAINS_KEY } = await import('../keys');

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

/**
 * r10 (r9 S4 unfixed): an account holding only Arc USDC was asked to connect
 * on Sepolia — the request window published the catalog as it opened, and
 * the catalog carried where the account lived, which that window never knew.
 */
describe('where an account lives, for the worker', () => {
	const ALICE = '0xD400866e00B055B20752a826CD5C89b811de130b';
	const BOB = '0x031D7D57c99CAF891e1C250554691Fd12D84772b';

	function storageArea() {
		const data: Record<string, unknown> = {};
		const set = vi.fn(async (items: Record<string, unknown>) => void Object.assign(data, items));
		const get = vi.fn(async (key: string) => (key in data ? { [key]: data[key] } : {}));
		vi.stubGlobal('chrome', { storage: { local: { get, set } } });
		return { data, set };
	}

	afterEach(() => {
		vi.unstubAllGlobals();
		resetExtChainsForTests();
	});

	it('is stored per account, apart from the catalog — a publish never says it', async () => {
		const { data } = storageArea();
		releaseRead();
		await setExtAccountChains(ALICE, [5042002]);
		await setExtAccountChains(BOB, [84532, 84532, 0]);
		// Another page — the request window — publishes the catalog: it knows
		// nobody's holdings and says nothing about them.
		resetExtChainsForTests();
		await publishExtChains();
		expect(data[ACCOUNT_CHAINS_KEY]).toEqual({
			[ALICE.toLowerCase()]: [5042002],
			[BOB.toLowerCase()]: [84532]
		});
		expect(data[CHAINS_KEY]).toBeDefined();
		expect(JSON.stringify(data[CHAINS_KEY])).not.toContain('accountChains');
	});

	it('does not store the same list twice, and keeps the other space’s networks', async () => {
		const { data, set } = storageArea();
		// Read earlier in the testnet space; the mainnet space is in front here.
		data[ACCOUNT_CHAINS_KEY] = { [ALICE.toLowerCase()]: [5042002] };
		await setExtAccountChains(ALICE, [100]);
		await setExtAccountChains(ALICE, [100]);
		expect(set).toHaveBeenCalledTimes(1);
		expect(data[ACCOUNT_CHAINS_KEY]).toEqual({ [ALICE.toLowerCase()]: [100, 5042002] });
	});
});
