/**
 * Vela Connect's wire (spec 109): what a site's message is read as, and what
 * it is answered with — for the popup and for the redirect.
 */
import { describe, expect, it } from 'vitest';
import {
	INVALID_REQUEST,
	MAX_REQUEST_BYTES,
	REDIRECT_REQUEST_ID,
	base64UrlDecodeJson,
	base64UrlEncodeJson,
	readInbound,
	readRedirect,
	readyMessage,
	redirectBack,
	redirectRefusal,
	responseMessage,
	returnOrigin
} from './protocol';

const STATE = 'abcdefghijklmnop_1234';

function hashFor(request: unknown, returnUrl: string, state = STATE): string {
	const params = new URLSearchParams({
		request: base64UrlEncodeJson(request),
		return: returnUrl,
		state
	});
	return `#${params.toString()}`;
}

describe('readInbound', () => {
	it('takes a v1 request with its id, method and params', () => {
		expect(
			readInbound({ vela: 1, type: 'request', id: 'r1', method: 'personal_sign', params: ['0x1'] })
		).toEqual({
			kind: 'request',
			request: { id: 'r1', method: 'personal_sign', params: ['0x1'] }
		});
	});

	it('reads absent params as none', () => {
		expect(readInbound({ vela: 1, type: 'request', id: 'r1', method: 'eth_chainId' })).toEqual({
			kind: 'request',
			request: { id: 'r1', method: 'eth_chainId', params: [] }
		});
	});

	it('ignores what is not ours: other traffic, other wallets, answers', () => {
		for (const data of [
			null,
			'hello',
			[1],
			{ type: 'request', id: 'r1', method: 'eth_accounts' },
			{ vela: 1, type: 'response', id: 'r1' },
			{ target: 'metamask-contentscript', data: {} }
		]) {
			expect(readInbound(data)).toEqual({ kind: 'ignore' });
		}
	});

	it('ignores a request with no usable id — there is nothing to answer it under', () => {
		for (const id of [undefined, 1, '', 'x'.repeat(65), 'a b', 'é']) {
			expect(readInbound({ vela: 1, type: 'request', id, method: 'eth_accounts' })).toEqual({
				kind: 'ignore'
			});
		}
	});

	it('answers another version -32600, naming the one it speaks', () => {
		const read = readInbound({ vela: 2, type: 'request', id: 'r1', method: 'eth_accounts' });
		expect(read.kind).toBe('invalid');
		if (read.kind === 'invalid') {
			expect(read.error.code).toBe(INVALID_REQUEST);
			expect(read.error.message).toContain('v1');
		}
	});

	it('refuses a missing method, params that are not an array, and an oversized request', () => {
		const bad = [
			{ vela: 1, type: 'request', id: 'r1' },
			{ vela: 1, type: 'request', id: 'r1', method: 'eth_call', params: { to: '0x' } },
			{ vela: 1, type: 'request', id: 'r1', method: 'x', params: ['a'.repeat(MAX_REQUEST_BYTES)] }
		];
		for (const data of bad) {
			const read = readInbound(data);
			expect(read.kind).toBe('invalid');
			if (read.kind === 'invalid') expect(read.error.code).toBe(INVALID_REQUEST);
		}
	});
});

describe('the answers', () => {
	it('ready carries nothing but the version', () => {
		expect(readyMessage()).toEqual({ vela: 1, type: 'ready' });
	});

	it('a result, and null for an answer that has none', () => {
		expect(responseMessage('r1', { result: ['0xab'] })).toEqual({
			vela: 1,
			type: 'response',
			id: 'r1',
			result: ['0xab']
		});
		expect(responseMessage('r2', {})).toEqual({
			vela: 1,
			type: 'response',
			id: 'r2',
			result: null
		});
	});

	it('an error keeps code, message and data, and nothing else', () => {
		const extra = { code: 4001, message: 'no', data: { reason: 'x' }, stack: 'leak' };
		expect(responseMessage('r1', { error: extra })).toEqual({
			vela: 1,
			type: 'response',
			id: 'r1',
			error: { code: 4001, message: 'no', data: { reason: 'x' } }
		});
	});
});

describe('base64url', () => {
	it('round-trips UTF-8 JSON without padding', () => {
		const value = { method: 'personal_sign', params: ['登录 sub3 ✓', '0xab'] };
		const text = base64UrlEncodeJson(value);
		expect(text).not.toMatch(/[+/=]/);
		expect(base64UrlDecodeJson(text)).toEqual(value);
	});

	it('refuses what is not base64url', () => {
		expect(() => base64UrlDecodeJson('a+b/')).toThrow();
	});
});

describe('returnOrigin', () => {
	it('takes https, and http only on this machine', () => {
		expect(returnOrigin('https://sub3.dev/login?x=1#old')).toBe('https://sub3.dev');
		expect(returnOrigin('http://localhost:5317/login')).toBe('http://localhost:5317');
		expect(returnOrigin('http://127.0.0.1:8080/')).toBe('http://127.0.0.1:8080');
	});

	it('refuses public http, other schemes, credentials and garbage', () => {
		for (const url of [
			'http://sub3.dev/login',
			'http://192.168.1.2/',
			'javascript:alert(1)',
			'data:text/html,hi',
			'https://user:pass@sub3.dev/',
			'sub3.dev',
			''
		]) {
			expect(returnOrigin(url)).toBeNull();
		}
	});
});

describe('readRedirect', () => {
	it('is none for the popup page (no fragment, or one that is not a request)', () => {
		expect(readRedirect('')).toEqual({ ok: false, reason: 'none' });
		expect(readRedirect('#')).toEqual({ ok: false, reason: 'none' });
		expect(readRedirect('#section-2')).toEqual({ ok: false, reason: 'none' });
	});

	it('reads the request, the return address, its origin and the state', () => {
		const read = readRedirect(
			hashFor({ method: 'eth_requestAccounts', params: [] }, 'https://sub3.dev/login?a=1')
		);
		expect(read).toEqual({
			ok: true,
			redirect: {
				request: { id: REDIRECT_REQUEST_ID, method: 'eth_requestAccounts', params: [] },
				returnUrl: 'https://sub3.dev/login?a=1',
				origin: 'https://sub3.dev',
				state: STATE
			}
		});
	});

	it('refuses a bad return address, a missing or short state, and an unreadable request', () => {
		const request = { method: 'eth_requestAccounts' };
		for (const hash of [
			hashFor(request, 'http://evil.example/'),
			hashFor(request, 'https://sub3.dev/', 'short'),
			hashFor(request, 'https://sub3.dev/', 'has spaces in it!!!!'),
			'#request=%%%&return=https%3A%2F%2Fsub3.dev%2F&state=' + STATE,
			hashFor([1, 2], 'https://sub3.dev/'),
			hashFor({ params: [] }, 'https://sub3.dev/')
		]) {
			expect(readRedirect(hash)).toEqual({ ok: false, reason: 'invalid' });
		}
	});
});

describe('redirectBack', () => {
	it('replaces the fragment, keeping path and query, and carries the state', () => {
		const url = redirectBack('https://sub3.dev/login?a=1#old', STATE, { result: ['0xab'] });
		const parsed = new URL(url);
		expect(parsed.origin + parsed.pathname + parsed.search).toBe('https://sub3.dev/login?a=1');
		expect(parsed.hash.startsWith('#vela=')).toBe(true);
		expect(base64UrlDecodeJson(parsed.hash.slice('#vela='.length))).toEqual({
			vela: 1,
			state: STATE,
			result: ['0xab']
		});
	});

	it('carries an error instead of a result', () => {
		const url = redirectBack('https://sub3.dev/login', STATE, {
			error: { code: 4001, message: 'User rejected the request' }
		});
		expect(base64UrlDecodeJson(new URL(url).hash.slice('#vela='.length))).toEqual({
			vela: 1,
			state: STATE,
			error: { code: 4001, message: 'User rejected the request' }
		});
	});
});

describe('redirectRefusal', () => {
	it('refuses message signatures and what changes a site’s standing', () => {
		for (const method of [
			'personal_sign',
			'eth_sign',
			'eth_signTypedData',
			'eth_signTypedData_v4',
			'wallet_revokePermissions',
			'wallet_switchEthereumChain',
			'wallet_addEthereumChain'
		]) {
			expect(redirectRefusal(method)).toMatch(/popup mode/);
		}
	});

	it('takes a connect, a transaction, a batch and the reads', () => {
		for (const method of [
			'eth_requestAccounts',
			'eth_accounts',
			'eth_chainId',
			'eth_sendTransaction',
			'wallet_sendCalls',
			'wallet_getCallsStatus',
			'wallet_getCapabilities'
		]) {
			expect(redirectRefusal(method)).toBeNull();
		}
	});
});
