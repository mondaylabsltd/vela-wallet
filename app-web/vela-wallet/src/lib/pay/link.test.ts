/**
 * A payment link, read by the REAL core (spec 039 Part B): what the `/pay`
 * page and the wallet do with a link is the core's ruling on it — accepted,
 * or refused with a reason the corpus words in every language.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';
import { resolvePayMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { validatePayQuery } from '$lib/flows/core/payment-request';
import { PAY_PARAMS, payPrefill, payQueryOf, refusalBody } from './link';

vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));

const TO = '0x52908400098527886E0F7030069857D2E4169EE7';
const USDC = '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913';
const en = resolvePayMessages('en');

describe('payQueryOf', () => {
	it('reads the builder’s seven parameters and nothing else', () => {
		const query = payQueryOf(
			`?to=${TO}&chain=8453&token=${USDC}&amount=1.5&sym=USDC&dec=6&net=Base%20Mainnet&utm_source=x`
		);
		expect(query).toEqual({
			to: TO,
			chain: '8453',
			token: USDC,
			amount: '1.5',
			sym: 'USDC',
			dec: '6',
			net: 'Base Mainnet'
		});
		expect(Object.keys(query).every((k) => (PAY_PARAMS as readonly string[]).includes(k))).toBe(
			true
		);
	});

	it('reads a repeated parameter by its first value, as every client does', () => {
		expect(payQueryOf(`?to=${TO}&chain=1&amount=1&amount=9999`).amount).toBe('1');
	});

	it('leaves out what the link does not say', () => {
		expect(payQueryOf(`?to=${TO}`)).toEqual({ to: TO });
		expect(payQueryOf('')).toEqual({});
	});
});

describe('the core’s ruling', () => {
	it('accepts a well-formed link and gives its request', async () => {
		const verdict = await validatePayQuery(
			payQueryOf(`?to=${TO}&chain=8453&token=${USDC}&amount=1.5&sym=USDC&dec=6&net=Base`)
		);
		expect(verdict.valid).toBe(true);
		if (!verdict.valid) return;
		expect(verdict.pay).toMatchObject({
			recipient: TO,
			chain_id: 8453,
			token_address: USDC,
			amount: '1.5',
			amount_base: '1500000'
		});
	});

	it.each([
		['a missing recipient', `?chain=1&amount=1`, 'recipient'],
		['a tampered recipient', `?to=${TO.slice(0, 40)}&chain=1`, 'recipient'],
		['a chain that is not a number', `?to=${TO}&chain=base`, 'network'],
		['a token that is not an address', `?to=${TO}&chain=1&token=USDC`, 'token'],
		['a hex amount', `?to=${TO}&chain=1&amount=0x10`, 'amount'],
		['a scientific amount', `?to=${TO}&chain=1&amount=1e18`, 'amount'],
		['an amount finer than its decimals', `?to=${TO}&chain=1&dec=6&amount=1.2345678`, 'amount']
	])('refuses %s, and says which', async (_, search, refusal) => {
		const verdict = await validatePayQuery(payQueryOf(search));
		expect(verdict).toEqual({ valid: false, refusal });
	});

	it('leaves a network or a token it cannot know to Send', async () => {
		// Well-formed, unknowable here: whether this wallet has chain 59144, or
		// a token at this address on Ethereum, is the send core's to find.
		const unknownChain = await validatePayQuery(payQueryOf(`?to=${TO}&chain=59144`));
		expect(unknownChain.valid).toBe(true);
		const strayToken = await validatePayQuery(
			payQueryOf(`?to=${TO}&chain=1&token=0x00000000000000000000000000000000deadbeef&amount=1`)
		);
		expect(strayToken.valid).toBe(true);
	});
});

describe('refusalBody', () => {
	it('words each reason, and the general sentence for recipient and network', () => {
		expect(refusalBody('recipient', en)).toBe(en.invalidBody);
		expect(refusalBody('network', en)).toBe(en.invalidBody);
		expect(refusalBody('token', en)).toBe(en.invalidToken);
		expect(refusalBody('amount', en)).toBe(en.invalidAmount);
		expect(refusalBody(null, en)).toBe(en.invalidBody);
	});

	it('has every word in every locale — none falls back to its key', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const m = resolvePayMessages(locale);
			for (const [name, value] of Object.entries(m)) {
				expect(value, `${locale}.${name}`).not.toMatch(/^(receive|onboarding)\./);
				expect(value.length, `${locale}.${name}`).toBeGreaterThan(0);
			}
			// Four sentences, four different things to say.
			expect(new Set([m.invalidBody, m.invalidToken, m.invalidAmount, m.noWalletBody]).size).toBe(
				4
			);
		}
	});
});

describe('payPrefill', () => {
	it('opens Send locked on the core’s request — the scanned-code door', async () => {
		const verdict = await validatePayQuery(
			payQueryOf(`?to=${TO}&chain=8453&token=${USDC}&amount=1.5&dec=6`)
		);
		if (!verdict.valid) throw new Error('refused');
		expect(payPrefill(verdict.pay)).toEqual({
			prefilled_recipient: TO,
			prefilled_chain_id: '8453',
			prefilled_token_address: USDC,
			prefilled_amount_base: '1500000',
			locked: true
		});
	});

	it('an open request names no amount', async () => {
		const verdict = await validatePayQuery(payQueryOf(`?to=${TO}&chain=1`));
		if (!verdict.valid) throw new Error('refused');
		expect(payPrefill(verdict.pay)).toMatchObject({
			prefilled_token_address: null,
			prefilled_amount_base: null,
			locked: true
		});
	});
});
