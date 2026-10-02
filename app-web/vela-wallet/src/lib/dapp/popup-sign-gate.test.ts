/**
 * The request window asks the core the questions every in-app browser's sign
 * gate asks (spec 089), on the real core:
 *
 *   - the address a request names is read by the core (`requested_address`):
 *     a transaction's `from` too, which the window's by-shape guess never saw —
 *     a transaction from another account was signed by the granted one;
 *   - a public plain-http origin is not asked to sign for.
 */
// The same one-shot Node init every build-time core consumer uses.
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DAppGrant } from './grants';

const ALICE = `0x${'a1'.repeat(20)}`;
const BOB = `0x${'b2'.repeat(20)}`;
const grant = (origin: string): DAppGrant => ({
	origin,
	address: ALICE,
	chainId: 100,
	grantedAt: 1_700_000_000_000
});

const stored = new Map<string, DAppGrant>();
const answers: { rid: string; answer: unknown }[] = [];

vi.mock('$lib/core/client', async (real) => ({
	...(await real<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('./grants', () => ({
	getGrant: async (origin: string) => stored.get(origin) ?? null,
	setGrant: async () => {}
}));
vi.mock('./transport', () => ({
	answerRequest: async (rid: string, answer: unknown) => {
		answers.push({ rid, answer });
		return true;
	}
}));

const { evaluate } = await import('./request');

const ask = (origin: string, method: string, params: unknown[]) =>
	evaluate(
		{ rid: '7:1', id: '1', method, params, origin, tabId: 7, at: 0 },
		{ activeAddress: ALICE, addresses: [ALICE] }
	);

beforeEach(() => {
	stored.clear();
	answers.length = 0;
});

describe('the address a request names is the core’s reading (089)', () => {
	it('refuses a transaction from another account — never re-signed by the grant', async () => {
		stored.set('https://dapp.example', grant('https://dapp.example'));
		for (const method of ['eth_sendTransaction', 'wallet_sendCalls']) {
			const stage = await ask('https://dapp.example', method, [
				{ from: BOB, to: BOB, value: '0x1' }
			]);
			expect(stage, method).toEqual({
				kind: 'refused',
				code: 4100,
				message: 'The requested account is no longer authorized'
			});
		}
		expect(answers).toHaveLength(2);
	});

	it('forwards a transaction from the granted account, or naming nobody', async () => {
		stored.set('https://dapp.example', grant('https://dapp.example'));
		for (const params of [
			[{ from: ALICE.toUpperCase().replace('0X', '0x'), to: BOB }],
			[{ to: BOB }]
		]) {
			expect(await ask('https://dapp.example', 'eth_sendTransaction', params)).toEqual({
				kind: 'signing',
				grantedAddress: expect.stringMatching(new RegExp(ALICE, 'i'))
			});
		}
		expect(answers).toHaveLength(0);
	});

	it('reads personal_sign’s account by position: a 20-byte message is a message', async () => {
		stored.set('https://dapp.example', grant('https://dapp.example'));
		expect(await ask('https://dapp.example', 'personal_sign', [BOB, ALICE])).toMatchObject({
			kind: 'signing'
		});
	});
});

describe('a public plain-http origin (089)', () => {
	it('is not asked to sign for, in the in-app browsers’ words', async () => {
		stored.set('http://dapp.example', grant('http://dapp.example'));
		const stage = await ask('http://dapp.example', 'personal_sign', ['0x68656c6c6f', ALICE]);
		expect(stage).toEqual({
			kind: 'refused',
			code: 4100,
			message: 'Signing requires a secure origin'
		});
		expect(answers[0].answer).toEqual({
			error: { code: 4100, message: 'Signing requires a secure origin' }
		});
	});

	it('may still be connected by a person, and loopback signs as before', async () => {
		expect(await ask('http://dapp.example', 'eth_requestAccounts', [])).toMatchObject({
			kind: 'consent'
		});
		stored.set('http://localhost:5173', grant('http://localhost:5173'));
		expect(
			await ask('http://localhost:5173', 'personal_sign', ['0x68656c6c6f', ALICE])
		).toMatchObject({ kind: 'signing' });
	});
});
