/**
 * The channel's promises, as vectors (spec 027 T323).
 *
 * These import the REAL page-side module — `extension/lib/protocol.js`, the one
 * bundled into the script that runs in a stranger's page — rather than a copy.
 * A constant that disagrees with the shipped one is exactly the bug this file
 * exists to prevent.
 */
// The core is loaded the way every build-time consumer loads it, so the
// worker's constants can be pinned to the core's own numbers (spec 082 §13).
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { rpcCooldownMs, rpcReadTimeoutMs } from '$lib/core/kernels';
import {
	ENDPOINTS_KEY,
	ERR,
	MAX_REQUEST_BYTES,
	READ_TIMEOUT_MS,
	chainNameOf,
	cooldownMs,
	endpointAnswered,
	endpointFailed,
	orderEndpoints,
	readFailureKind,
	unreachableChainMessage,
	chainEndpoints,
	chainKnown,
	classifyMethod,
	hostLabel,
	isWellFormedRequest,
	originOfUrl,
	parseChainId,
	pickSignAddress,
	switchChainParam,
	toHexChainId
} from '../../../extension/lib/protocol.js';

describe('the routing bucket', () => {
	it('sends everything that needs a passkey to the sheet', () => {
		for (const method of [
			'eth_sendTransaction',
			'wallet_sendCalls',
			'personal_sign',
			'eth_signTypedData_v4'
		])
			expect(classifyMethod(method), method).toBe('sign');
	});

	it('refuses eth_sign outright — it signs an opaque digest', () => {
		expect(classifyMethod('eth_sign')).toBe('unsupported');
	});

	it('is an ALLOWLIST, so an unknown method is refused rather than forwarded', () => {
		// The failure this pins is a denylist failing OPEN: `eth_signTransaction`
		// is not caught by the signing predicate, so a catch-all "read" bucket
		// would proxy it to a public node and make the extension an open relay.
		expect(classifyMethod('eth_signTransaction')).toBe('unsupported');
		expect(classifyMethod('vela_pleaseDoAnything')).toBe('unsupported');
		expect(classifyMethod('eth_call')).toBe('read');
	});

	it('separates asking to connect from asking about state', () => {
		expect(classifyMethod('eth_requestAccounts')).toBe('connect');
		expect(classifyMethod('wallet_requestPermissions')).toBe('connect');
		expect(classifyMethod('eth_accounts')).toBe('state');
		expect(classifyMethod('eth_chainId')).toBe('state');
	});
});

describe('what may reach a screen', () => {
	const ok = { id: 'abc', method: 'eth_accounts', params: [] };

	it('accepts an ordinary request', () => {
		expect(isWellFormedRequest(ok)).toBe(true);
	});

	it('refuses anything missing its shape', () => {
		expect(isWellFormedRequest(null)).toBe(false);
		expect(isWellFormedRequest({ ...ok, id: '' })).toBe(false);
		expect(isWellFormedRequest({ ...ok, method: '' })).toBe(false);
		expect(isWellFormedRequest({ ...ok, params: 'not-an-array' })).toBe(false);
	});

	it('refuses a payload too large to be a request', () => {
		const huge = 'x'.repeat(MAX_REQUEST_BYTES + 1);
		expect(isWellFormedRequest({ ...ok, params: [huge] })).toBe(false);
	});

	it('refuses a cyclic payload instead of throwing on it', () => {
		const cyclic: Record<string, unknown> = {};
		cyclic.self = cyclic;
		expect(isWellFormedRequest({ ...ok, params: [cyclic] })).toBe(false);
	});
});

describe('the small things that are wrong everywhere else', () => {
	it('finds the signing address by SHAPE, not by position', () => {
		const address = `0x${'ab'.repeat(20)}`;
		// personal_sign is [message, address]; typed data is [address, data].
		expect(pickSignAddress('personal_sign', ['0xdead', address])).toBe(address);
		expect(pickSignAddress('eth_signTypedData_v4', [address, '{}'])).toBe(address);
		expect(pickSignAddress('personal_sign', ['0xdead'])).toBeNull();
	});

	it('writes a chain id the minimal way EIP-1193 asks for', () => {
		expect(toHexChainId(1)).toBe('0x1');
		expect(toHexChainId(100)).toBe('0x64');
		expect(toHexChainId('0x64')).toBe('0x64');
		// Nonsense becomes mainnet rather than `0xNaN` on a dApp's screen.
		expect(toHexChainId(0)).toBe('0x1');
	});

	it('reads a chain id from every shape a dApp sends', () => {
		expect(parseChainId(100)).toBe(100);
		expect(parseChainId('0x64')).toBe(100);
		expect(parseChainId('100')).toBe(100);
		expect(parseChainId('nonsense')).toBe(0);
	});

	it('shortens an origin to what a person reads, and never throws', () => {
		expect(hostLabel('https://app.uniswap.org/swap')).toBe('app.uniswap.org');
		expect(hostLabel('not a url')).toBe('not a url');
	});

	it('keeps a distinct code for "pending / unknown"', () => {
		// A stuck-but-submitted transaction must never look like a clean decline:
		// a dApp that reads 4001 may safely retry, which is a double-spend.
		expect(ERR.UNKNOWN_PENDING).not.toBe(ERR.USER_REJECTED);
	});
});

describe('what the worker now routes itself (reads, switching)', () => {
	it('gives chain switching and asset watching their own buckets', () => {
		expect(classifyMethod('wallet_switchEthereumChain')).toBe('switch');
		expect(classifyMethod('wallet_addEthereumChain')).toBe('addChain');
		expect(classifyMethod('wallet_watchAsset')).toBe('watchAsset');
		expect(classifyMethod('eth_estimateGas')).toBe('read');
		expect(classifyMethod('eth_sendUserOperation')).toBe('read');
	});

	it('reads the chain a switch names, in either spelling, and refuses the rest', () => {
		expect(switchChainParam([{ chainId: '0x64' }])).toBe(100);
		expect(switchChainParam([{ chainId: '8453' }])).toBe(8453);
		expect(switchChainParam([{ chainId: 1 }])).toBe(1);
		expect(switchChainParam([])).toBe(0);
		expect(switchChainParam([{ chainId: 'mainnet' }])).toBe(0);
		expect(switchChainParam(null)).toBe(0);
	});

	it('keys a tab by its ORIGIN, and only for web pages', () => {
		expect(originOfUrl('https://app.uniswap.org/swap?x=1')).toBe('https://app.uniswap.org');
		expect(originOfUrl('http://localhost:8812/')).toBe('http://localhost:8812');
		expect(originOfUrl('chrome-extension://abc/en/wallet.html')).toBeNull();
		expect(originOfUrl('chrome://extensions')).toBeNull();
		expect(originOfUrl(undefined)).toBeNull();
	});

	it('takes endpoints ONLY from the catalog the wallet published', () => {
		const catalog = {
			version: 1,
			chains: {
				'100': { rpc: ['https://a', 'https://b'], bundler: 'https://relay/100' },
				'1': { rpc: [], bundler: '' }
			}
		};
		expect(chainEndpoints(catalog, 100, false)).toEqual(['https://a', 'https://b']);
		expect(chainEndpoints(catalog, 100, true)).toEqual(['https://relay/100']);
		expect(chainEndpoints(catalog, 1, true)).toEqual([]);
		// An unknown chain has NO endpoint — never a guess at a public node.
		expect(chainEndpoints(catalog, 8453, false)).toEqual([]);
		expect(chainEndpoints(null, 100, false)).toEqual([]);
		expect(chainKnown(catalog, 100)).toBe(true);
		expect(chainKnown(catalog, 8453)).toBe(false);
		expect(chainKnown(undefined, 1)).toBe(false);
	});
});

describe('the worker’s chain reads (spec 082 RF2, G20, G33)', () => {
	it('gives one endpoint the core’s read budget, not 20 s', () => {
		expect(READ_TIMEOUT_MS).toBe(rpcReadTimeoutMs());
		expect(READ_TIMEOUT_MS).toBe(8_000);
	});

	it('cools a failing endpoint exactly as the core’s pool does', () => {
		for (let n = 1; n <= 5; n += 1) expect(cooldownMs(n), `n=${n}`).toBe(rpcCooldownMs(n));
		expect(cooldownMs(0)).toBe(0);
		expect(cooldownMs(9)).toBe(300_000);
	});

	it('tries cooled endpoints last, soonest-back first, and a success forgets the failure', () => {
		const now = 1_000_000;
		let health = endpointFailed({}, 'https://a', now);
		health = endpointFailed(health, 'https://a', now);
		health = endpointFailed(health, 'https://b', now);
		expect(health['https://a']).toEqual({ failures: 2, until: now + 60_000 });
		expect(health['https://b']).toEqual({ failures: 1, until: now + 30_000 });
		// The second call skips straight to the node that did not fail.
		expect(orderEndpoints(['https://a', 'https://b', 'https://c'], health, now + 1)).toEqual([
			'https://c',
			'https://b',
			'https://a'
		]);
		// A cool-down that has run out puts the endpoint back in its place.
		expect(orderEndpoints(['https://a', 'https://b'], health, now + 60_001)).toEqual([
			'https://a',
			'https://b'
		]);
		expect(endpointAnswered(health, 'https://a')).toEqual({ 'https://b': health['https://b'] });
		expect(ENDPOINTS_KEY).toBe('vela.ext.endpoints');
	});

	it('names the chain in plain words, with no engine text', () => {
		const catalog = { chains: { '100': { name: 'Gnosis', rpc: [] } } };
		expect(unreachableChainMessage(chainNameOf(catalog, 100), 100)).toBe(
			'Vela could not reach a node for chain Gnosis (100)'
		);
		expect(unreachableChainMessage(chainNameOf(catalog, 5), 5)).toBe(
			'Vela could not reach a node for chain unknown (5)'
		);
		expect(unreachableChainMessage('Gnosis', 100)).not.toMatch(/fetch|signal|abort/i);
	});

	it('names a failure by its kind for the log', () => {
		expect(readFailureKind(Object.assign(new Error('x'), { name: 'TimeoutError' }))).toBe(
			'timeout'
		);
		expect(readFailureKind(new TypeError('Failed to fetch'))).toBe('network');
		expect(readFailureKind(new Error('http'), 502)).toBe('http_502');
	});
});
