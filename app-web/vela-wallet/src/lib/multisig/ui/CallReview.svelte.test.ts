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
import { CallReading } from '../core/call-review.svelte';
import { APPROVE_MAX, BLIND, CHAIN, MULTISIG, read } from '../core/call-review.testing';
import CallReview from './CallReview.svelte';

/** Every message is its own name: `m.warnUnlimited` reads "warnUnlimited". */
const names = { get: (_: object, key: string | symbol) => String(key) };
const m = new Proxy({} as SigningMessages, {
	get: (_, key) => (key === 'certain' ? new Proxy({}, names) : String(key))
});

function shown(call: typeof APPROVE_MAX) {
	const reading = new CallReading(CHAIN, MULTISIG, call);
	const views = read(call);
	reading.clear = views.clear;
	reading.guard = views.guard;
	return render(CallReview, { reading, m });
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
});
