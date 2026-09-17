import { describe, expect, it } from 'vitest';
import { rank, search, type ChainHit } from './search';

const idx: ChainHit[] = [
	{ chainId: 1, name: 'Ethereum Mainnet', shortName: 'eth', nativeCurrencySymbol: 'ETH' },
	{ chainId: 5042, name: 'Arc Network', shortName: 'arc', nativeCurrencySymbol: 'USDC' },
	{
		chainId: 5042002,
		name: 'Arc Network Testnet',
		shortName: 'arc-testnet',
		nativeCurrencySymbol: 'USDC'
	},
	{ chainId: 50420, name: 'Somechain', shortName: 'some', nativeCurrencySymbol: 'SOME' },
	{ chainId: 100, name: 'Gnosis', shortName: 'gno', nativeCurrencySymbol: 'XDAI' },
	{ chainId: 42161, name: 'Arbitrum One', shortName: 'arb1', nativeCurrencySymbol: 'ETH' },
	{ chainId: 988, name: 'Stable Mainnet', shortName: 'stable', nativeCurrencySymbol: 'USDT0' }
];

describe('search', () => {
	it('an exact chain id wins over ids that merely start with it, and a testnet sinks below a mainnet', () => {
		expect(search(idx, '5042').map((h) => h.chainId)).toEqual([5042, 50420, 5042002]);
	});
	it('a symbol finds every chain whose gas coin it is, mainnets first', () => {
		expect(search(idx, 'usdc').map((h) => h.chainId)).toEqual([5042, 5042002]);
		expect(search(idx, 'eth').map((h) => h.chainId)).toEqual([1, 42161]);
	});
	it('a name prefix is case-insensitive and ranks above a substring', () => {
		expect(search(idx, 'Arc').map((h) => h.chainId)).toEqual([5042, 5042002]);
		expect(search(idx, 'net').map((h) => h.name)).toContain('Arc Network');
	});
	it('nothing matches an empty or unrelated query', () => {
		expect(search(idx, '')).toEqual([]);
		expect(search(idx, 'zzzz')).toEqual([]);
		expect(rank(idx[0], '')).toBeNull();
	});
});
