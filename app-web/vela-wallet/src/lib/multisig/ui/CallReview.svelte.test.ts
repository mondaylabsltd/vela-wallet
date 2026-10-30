/**
 * cs-12 in a browser: the review a member sees on a multisig proposal (and
 * on the outside-wallet sheet) is drawn by the signing sheet's renderer from
 * the real cores' reading — "Approve" and the unlimited warning for an
 * approve(spender, MAX), the red blind warning for a call nobody can read.
 *
 * The words are their keys (a stand-in catalog), so what is asserted is
 * WHICH sentence is drawn, not its English.
 */
import { beforeAll, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { loadCore } from '$lib/core/client';
import type { SigningMessages } from '$lib/signing/messages';
import type { PartyBadge } from '$lib/signing/model';
import { CallReading } from '../core/call-review.svelte';
import { APPROVE_MAX, BLIND, CHAIN, MULTISIG, SPENDER, read } from '../core/call-review.testing';
import CallReview from './CallReview.svelte';

/** Every message is its own name: `m.warnUnlimited` reads "warnUnlimited". */
const names = { get: (_: object, key: string | symbol) => String(key) };
const m = new Proxy({} as SigningMessages, {
	get: (_, key) => (key === 'certain' ? new Proxy({}, names) : String(key))
});

function shown(
	call: typeof APPROVE_MAX,
	badges?: Record<string, PartyBadge>,
	names?: Record<string, string>
) {
	const reading = new CallReading(CHAIN, MULTISIG, call);
	const views = read(call);
	reading.clear = views.clear;
	reading.guard = views.guard;
	return render(CallReview, {
		reading,
		m,
		...(badges ? { badges } : {}),
		...(names ? { names } : {})
	});
}

describe('a member reads a multisig’s contract call (cs-12)', () => {
	beforeAll(async () => {
		await loadCore();
	});

	it('approve(spender, MAX): the decoded approval and the unlimited warning', async () => {
		const screen = shown(APPROVE_MAX);
		const text = screen.container.textContent ?? '';
		expect(text.toLowerCase()).toContain('approve');
		expect(text).toContain('warnUnlimited');
		expect(text).toContain('valueUnlimited');
		expect(text).not.toContain('warnBlindDecode');
		// What it grants is said, not offered: no cap chips to press.
		expect(screen.container.querySelectorAll('button').length).toBe(0);
	});

	it('a call nobody can read: the blind title and warning, and the coin it moves', async () => {
		const screen = shown(BLIND);
		const text = screen.container.textContent ?? '';
		expect(text).toContain('intentBlind');
		expect(text).toContain('warnBlindDecode');
		expect(text).toContain('1.5');
	});

	it('r5reg-15: its recipient row wears what the review knows of it — said once, here', async () => {
		const send = { to: SPENDER, value: '50000000000000000', data: '0x' };
		const plain = shown(send);
		expect(plain.container.textContent).not.toContain('tagFirstTime');
		await plain.unmount();
		const screen = shown(send, {
			[SPENDER.toLowerCase()]: { text: 'tagFirstTime', tone: 'caution' }
		});
		expect(screen.container.textContent).toContain('tagFirstTime');
	});

	it('r4a-18: its recipient row reads by the name the review knows, the address beneath', async () => {
		const send = { to: SPENDER, value: '50000000000000000', data: '0x' };
		const screen = shown(
			send,
			{ [SPENDER.toLowerCase()]: { text: 'knownOwn', tone: 'neutral' } },
			{ [SPENDER.toLowerCase()]: 'R6 Bob' }
		);
		const text = screen.container.textContent ?? '';
		expect(text).toContain('R6 Bob');
		expect(text).toContain('knownOwn');
		expect(text).toContain(SPENDER);
	});
});
