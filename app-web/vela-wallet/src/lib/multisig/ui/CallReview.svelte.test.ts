/**
 * r8-1 in a browser: the review a member sees on a multisig proposal (and
 * on the outside-wallet sheet) draws the core's own reading of every call —
 * the shared vectors every platform replays — with the signing sheet's
 * renderer: "Approve" and "Unlimited" in red for an approve(spender, MAX),
 * every leg of a nested batch, the raw amount of a token whose decimals are
 * unknown.
 *
 * The words are their keys (a stand-in catalog), so what is asserted is
 * WHICH sentence is drawn, not its English.
 */
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { PartyBadge } from '$lib/signing/model';
import type { MultisigMessages } from '../messages';
import type { MultisigReviewOut } from '../core/kernel';
import CallReview from './CallReview.svelte';
import VECTORS from '../../../../../../rust/crates/vela-core/tests/vectors/multisig/reading.json';

/** Every message is its own name: `m.review.reading.unlimited` reads "unlimited". */
const words = (): object => new Proxy({}, { get: (_, key) => String(key) });
const m = {
	review: { callNumber: 'call {{n}}', reading: words() }
} as unknown as MultisigMessages;

type Case = (typeof VECTORS.cases)[number];
const named = (start: string): Case => VECTORS.cases.find((c) => c.name.startsWith(start))!;
const reviewOf = (c: Case) => c.expect as unknown as MultisigReviewOut;

function shown(c: Case, badges?: Record<string, PartyBadge>, names?: Record<string, string>) {
	return render(CallReview, {
		review: reviewOf(c),
		m,
		nativeSymbol: 'USDC',
		...(badges ? { badges } : {}),
		...(names ? { names } : {})
	});
}

describe('a member reads a multisig’s calls, as the core read them (r8-1)', () => {
	it('approve(spender, MAX): Approve, Unlimited, and the unlimited line', async () => {
		const screen = shown(named("approve(spender, 2^256-1) of Arc's USDC"));
		const text = screen.container.textContent ?? '';
		expect(text).toContain('approve');
		expect(text).toContain('unlimited');
		expect(text).toContain('unlimitedLine');
		// What it grants is said, not offered: no cap chips to press.
		expect(screen.container.querySelectorAll('button').length).toBe(0);
	});

	it('a nested batch: every leg of every level, and the nested line (r8-5)', async () => {
		const screen = shown(named('a batch whose leg calls multiSend'));
		const text = screen.container.textContent ?? '';
		expect(text).toContain('call 1');
		expect(text).toContain('call 1.1');
		expect(text).toContain('call 1.2');
		expect(text).toContain('nestedLine');
	});

	it('a token nobody read: its raw amount and decimals unknown (r8-2)', async () => {
		const screen = shown(named('a token nobody read'));
		const text = screen.container.textContent ?? '';
		expect(text).toContain('decimalsUnknown');
		expect(text).toContain('decimalsLine');
	});

	it('r5reg-15 / r4a-18: its recipient row wears what the review knows of it, by name', async () => {
		const send = named("Arc's USDC ERC-20 transfer");
		const to = '0x4444444444444444444444444444444444444444';
		const screen = shown(
			send,
			{ [to]: { text: 'tagFirstTime', tone: 'caution' } },
			{ [to]: 'R8 Bob' }
		);
		const text = screen.container.textContent ?? '';
		expect(text).toContain('tagFirstTime');
		expect(text).toContain('R8 Bob');
	});

	it('a member change has no calls: nothing drawn', async () => {
		const screen = shown(named('a member change'));
		expect(screen.container.querySelector('[data-testid="multisig-call-review"]')).toBeNull();
	});
});
