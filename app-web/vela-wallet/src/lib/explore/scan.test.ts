/**
 * Issue 273: the Explore scan button opens a scanned WEB ADDRESS and refuses
 * everything else — a WalletConnect pairing code is not supported and a
 * payment code is send's. The same cases as iOS's `ExploreScanTests`.
 */
import { describe, expect, it } from 'vitest';
import { exploreScanUrl } from './scan';

describe('exploreScanUrl', () => {
	it('opens an http(s) URL as it is', () => {
		expect(exploreScanUrl('https://app.uniswap.org/swap?chain=base')).toBe(
			'https://app.uniswap.org/swap?chain=base'
		);
		expect(exploreScanUrl('  http://localhost:5173/\n')).toBe('http://localhost:5173/');
	});

	it('reads an uppercase scheme as the same address', () => {
		expect(exploreScanUrl('HTTPS://EXAMPLE.COM/A')).toBe('https://EXAMPLE.COM/A');
	});

	it('puts https in front of a bare domain, as the search field would', () => {
		expect(exploreScanUrl('app.aave.com')).toBe('https://app.aave.com');
		expect(exploreScanUrl('app.aave.com/markets')).toBe('https://app.aave.com/markets');
	});

	it('refuses a WalletConnect code', () => {
		expect(
			exploreScanUrl(
				'wc:7f6e504bfad60b485450578e05678ed3e8e8c4751d3c6160be17160d63ec90f9@2?relay-protocol=irn&symKey=587d5484ce2a2a6ee3ba1962fdd7e8588e06200c46823bd18fbd67def96ad303'
			)
		).toBeNull();
	});

	it('refuses payment codes and bare addresses', () => {
		expect(exploreScanUrl('ethereum:0x88cCA0EeDbF2C4426110bbFc998F048689266894@100')).toBeNull();
		expect(exploreScanUrl('0x88cCA0EeDbF2C4426110bbFc998F048689266894')).toBeNull();
	});

	it.each([
		'',
		'hello world',
		'uniswap',
		'mailto:someone@example.com',
		'https://',
		'user:pass@example.com'
	])('refuses %j', (text) => {
		expect(exploreScanUrl(text)).toBeNull();
	});
});
