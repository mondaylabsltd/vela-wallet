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
import { rpcCooldownMs, rpcReadTimeoutMs, signNotConfirmedDetail } from '$lib/core/kernels';
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
	droppedChannelAnswer,
	HELD_BUCKETS,
	hostLabel,
	isWellFormedRequest,
	maybeSentPayload,
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

	it('refuses a typed-data request that is not exactly one document in its method\'s order (audit 2026-10-01)', () => {
		const account = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
		const doc = (primaryType: string) =>
			JSON.stringify({ types: { EIP712Domain: [] }, primaryType, domain: {}, message: {} });
		const req = (method: string, params: unknown[]) => ({ id: 'td', method, params });
		// The four methods, well-formed.
		expect(isWellFormedRequest(req('eth_signTypedData_v4', [account, doc('Mail')]))).toBe(true);
		expect(isWellFormedRequest(req('eth_signTypedData_v3', [account, JSON.parse(doc('Mail'))]))).toBe(true);
		expect(isWellFormedRequest(req('eth_signTypedData', [doc('Mail'), account]))).toBe(true);
		expect(isWellFormedRequest(req('eth_signTypedData_v1', [doc('Mail'), account]))).toBe(true);
		// The audit's two shapes.
		expect(isWellFormedRequest(req('eth_signTypedData_v4', [doc('Mail'), doc('Permit')]))).toBe(false);
		expect(isWellFormedRequest(req('eth_signTypedData', [doc('Permit'), doc('Mail')]))).toBe(false);
		expect(isWellFormedRequest(req('eth_signTypedData_v1', [doc('Permit'), doc('Mail')]))).toBe(false);
		// The wrong order, one param, three, and a name that only resembles one.
		expect(isWellFormedRequest(req('eth_signTypedData_v4', [doc('Mail'), account]))).toBe(false);
		expect(isWellFormedRequest(req('eth_signTypedData', [account, doc('Mail')]))).toBe(false);
		expect(isWellFormedRequest(req('eth_signTypedData_v4', [doc('Mail')]))).toBe(false);
		expect(isWellFormedRequest(req('eth_signTypedData_v4', [account, doc('Mail'), doc('Mail')]))).toBe(false);
		expect(isWellFormedRequest(req('eth_signTypedData_v2', [account, doc('Mail')]))).toBe(false);
		// A document without its EIP-712 fields.
		expect(isWellFormedRequest(req('eth_signTypedData_v4', [account, '{"primaryType":"Mail"}']))).toBe(false);
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
		// Spec 100: a person answers it — held, and retried once on a drop, like a connect.
		expect(HELD_BUCKETS.has('addChain')).toBe(true);
		expect(droppedChannelAnswer('addChain', 'wallet_addEthereumChain', 0)).toEqual({ retry: true });
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
	it('a lost claimed submit is told the core\'s "not confirmed yet"; a batch, its id (RJ2, 083)', () => {
		const op = `0x${'ab'.repeat(32)}`;
		expect(maybeSentPayload('eth_sendTransaction', op)).toEqual({
			error: { code: -32603, message: signNotConfirmedDetail(op) }
		});
		expect(maybeSentPayload('wallet_sendCalls', op)).toEqual({ result: op });
		// Spec 097 G: in the shape the batch declared — 2.0.0's `{ id }`.
		const v2 = [{ version: '2.0.0', calls: [] }];
		expect(maybeSentPayload('wallet_sendCalls', op, v2)).toEqual({ result: { id: op } });
		const v1 = [{ version: '1.0', calls: [] }];
		expect(maybeSentPayload('wallet_sendCalls', op, v1)).toEqual({ result: op });
		expect(maybeSentPayload('eth_sendTransaction', op, v2)).toEqual({
			error: { code: -32603, message: signNotConfirmedDetail(op) }
		});
	});


	it('cools a failing endpoint exactly as the core’s pool does', () => {
		for (let n = 1; n <= 5; n += 1) expect(cooldownMs(n), `n=${n}`).toBe(rpcCooldownMs(n));
		expect(cooldownMs(0)).toBe(0);
		expect(cooldownMs(9)).toBe(300_000);
	});

	it('skips cooled endpoints while one is live, and a success forgets the failure', () => {
		const now = 1_000_000;
		let health = endpointFailed({}, 'https://a', now);
		health = endpointFailed(health, 'https://a', now);
		health = endpointFailed(health, 'https://b', now);
		expect(health['https://a']).toEqual({ failures: 2, until: now + 60_000 });
		expect(health['https://b']).toEqual({ failures: 1, until: now + 30_000 });
		// The second call goes straight to the node that did not fail — and ONLY
		// to it: re-paying 8 s on each dead node every call was G64.
		expect(orderEndpoints(['https://a', 'https://b', 'https://c'], health, now + 1)).toEqual([
			'https://c'
		]);
		// A cool-down that has run out puts the endpoint back in its place.
		expect(orderEndpoints(['https://a', 'https://b'], health, now + 60_001)).toEqual([
			'https://a',
			'https://b'
		]);
		expect(endpointAnswered(health, 'https://a')).toEqual({ 'https://b': health['https://b'] });
		expect(ENDPOINTS_KEY).toBe('vela.ext.endpoints');
	});

	it('with every endpoint cooled, tries only the one that comes back first (RJ20, G64)', () => {
		const now = 1_000_000;
		const health = {
			'https://a': { failures: 3, until: now + 120_000 },
			'https://b': { failures: 1, until: now + 30_000 },
			'https://c': { failures: 2, until: now + 60_000 }
		};
		expect(orderEndpoints(['https://a', 'https://b', 'https://c'], health, now + 1)).toEqual([
			'https://b'
		]);
		// One endpoint and it is cooled: it is still asked — a read is never refused unasked.
		expect(orderEndpoints(['https://a'], health, now + 1)).toEqual(['https://a']);
		expect(orderEndpoints([], health, now)).toEqual([]);
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

	it('an abort by the 8 s timer is a timeout, whatever the engine threw (G64)', () => {
		// Chrome's worker rejected some timer aborts with a plain "Failed to
		// fetch": the device log said kind=network for four 8 s waits.
		expect(readFailureKind(new TypeError('Failed to fetch'), undefined, true)).toBe('timeout');
		expect(
			readFailureKind(Object.assign(new Error('x'), { name: 'AbortError' }), undefined, true)
		).toBe('timeout');
		// A status is an answer, however long it took.
		expect(readFailureKind(new Error('http'), 504, true)).toBe('http_504');
		expect(readFailureKind(new TypeError('Failed to fetch'), undefined, false)).toBe('network');
	});
});
