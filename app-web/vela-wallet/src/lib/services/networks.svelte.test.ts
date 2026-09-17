/**
 * Spec 038 #E9 (the founder's second report): a network the person added
 * must be in the snapshot on a FRESH load, before anything fetches — the
 * balance walk and the sidebar both read `getAllNetworksSync()`.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: true }));

import { setItem } from './storage';
import {
	ensureCustomNetworks,
	getAllNetworksSync,
	getCustomChainIdsSync,
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

	it('a later write still refreshes', async () => {
		await ensureCustomNetworks();
		await setItem('vela.customNetworks', JSON.stringify([]));
		await refreshCustomNetworks();
		expect(getCustomChainIdsSync()).not.toContain(59144);
	});
});
