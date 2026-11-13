/**
 * Spec 038 #E9 (the founder's second report): a network the person added
 * must be in the snapshot on a FRESH load, before anything fetches — the
 * balance walk and the sidebar both read `getAllNetworksSync()`.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: true }));

import { setItem } from './storage';
import {
	chainName,
	ensureCustomNetworks,
	explorerBaseURL,
	explorerTxURL,
	getAllNetworksSync,
	getCustomChainIdsSync,
	networkForChainId,
	refreshCustomNetworks
} from './networks';

// Linea: a real chain that is NOT built in, which is what makes it a custom one.
// (This fixture was Celo until Celo became built in, spec 061.)
const LINEA = {
	id: 'custom-59144',
	displayName: 'Linea',
	chainId: 59144,
	iconLabel: 'CEL',
	iconColor: '#fff',
	iconBg: '#35d07f',
	isL2: false,
	rpcURL: 'https://rpc.linea.build',
	explorerURL: 'https://lineascan.build',
	nativeSymbol: 'ETH'
};

describe('the custom-network snapshot', () => {
	beforeEach(async () => {
		await setItem('vela.customNetworks', '[]');
	});

	it('reads what the person added, once per document, before the first fetch', async () => {
		await setItem('vela.customNetworks', JSON.stringify([LINEA]));
		const first = ensureCustomNetworks();
		expect(ensureCustomNetworks()).toBe(first);
		await first;
		expect(getCustomChainIdsSync()).toContain(59144);
		expect(getAllNetworksSync().some((n) => n.chainId === 59144)).toBe(true);
	});

	// set-2: `[null]` in storage threw a TypeError in the snapshot build, and
	// the home sat on its skeleton for good. A bad record is dropped.
	it('drops a malformed record instead of throwing, and keeps the good ones', async () => {
		await setItem(
			'vela.customNetworks',
			JSON.stringify([null, 7, {}, { ...LINEA, chainId: 'x' }, { chainId: -3 }, LINEA])
		);
		await expect(refreshCustomNetworks()).resolves.toBeUndefined();
		expect(getCustomChainIdsSync()).toEqual([59144]);
		expect(chainName(59144)).toBe('Linea');
	});

	it('a record with mistyped fields still names its chain', async () => {
		await setItem(
			'vela.customNetworks',
			JSON.stringify([{ chainId: 59144, displayName: 42, rpcURL: null }])
		);
		await refreshCustomNetworks();
		expect(chainName(59144)).toBe('Chain 59144');
		expect(networkForChainId(59144)?.rpcURL).toBe('');
	});

	it('a later write still refreshes', async () => {
		await ensureCustomNetworks();
		await setItem('vela.customNetworks', JSON.stringify([]));
		await refreshCustomNetworks();
		expect(getCustomChainIdsSync()).not.toContain(59144);
	});
});

// set-7: the explorer a person sets is the one the links use — for a built-in
// network too — and an address the URL rule refuses is never a link.
describe('explorer links', () => {
	beforeEach(async () => {
		await setItem('vela.customNetworks', '[]');
		await setItem('vela.networkConfig', '[]');
	});

	it('use the explorer set for a built-in network', async () => {
		await setItem(
			'vela.networkConfig',
			JSON.stringify([{ chainId: 1, rpcURL: '', explorerURL: 'https://eth.blockscout.com/' }])
		);
		await refreshCustomNetworks();
		expect(explorerBaseURL(1)).toBe('https://eth.blockscout.com');
		expect(explorerTxURL(1, '0xabc')).toBe('https://eth.blockscout.com/tx/0xabc');
	});

	it('fall back to the network’s own explorer when the set one is refused or blank', async () => {
		await setItem(
			'vela.networkConfig',
			JSON.stringify([
				{ chainId: 1, explorerURL: 'javascript:alert(1)' },
				{ chainId: 10, explorerURL: '' }
			])
		);
		await refreshCustomNetworks();
		expect(explorerBaseURL(1)).toBe('https://etherscan.io');
		expect(explorerBaseURL(10)).toBe('https://optimistic.etherscan.io');
	});

	it('never link a custom network’s javascript: or data: explorer', async () => {
		await setItem(
			'vela.customNetworks',
			JSON.stringify([
				{ ...LINEA, explorerURL: 'javascript:alert(document.domain)' },
				{ ...LINEA, id: 'custom-7', chainId: 7, explorerURL: 'data:text/html,<script>1</script>' }
			])
		);
		await refreshCustomNetworks();
		expect(explorerBaseURL(59144)).toBeNull();
		expect(explorerBaseURL(7)).toBeNull();
		expect(explorerTxURL(59144, '0xabc').startsWith('https://')).toBe(true);
	});
});
