/**
 * §12: the sign door every shell draws, replayed from the core's shared
 * vectors (`rust/crates/vela-core/tests/vectors/multisig/review-state.json`)
 * through the link door the web calls (`review.door`) — and the shell's own
 * fall-back: a door it could not ask for is shut.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import vectors from '../../../../../../rust/crates/vela-core/tests/vectors/multisig/review-state.json';
import { missingSaid, missingWords, reasonWords, reviewDoor, stateFor } from './review-door';

describe('review state door (§12, shared vectors)', () => {
	for (const c of vectors.door) {
		it(c.name, () => {
			const args = c.args as { state?: unknown; review?: unknown };
			expect(reviewDoor((args.state ?? null) as never, args.review ?? null)).toEqual(c.expect);
		});
	}

	// r14-13: where a gathered review lands — the network answered none of
	// its reads: failed, never "… anyway".
	for (const c of vectors.settle) {
		it(`settle: ${c.name}`, () => {
			expect(JSON.parse(wasm.multisigLinkCall('review.settle', JSON.stringify(c.args)))).toEqual(
				c.expect
			);
		});
	}

	// r16-3: whether a review's safety checks are final for the read —
	// "unchecked" (… anyway) is published only once they are.
	for (const c of vectors.recheck) {
		it(`recheck: ${c.name}`, () => {
			expect(
				JSON.parse(wasm.multisigLinkCall('review.checksFinal', JSON.stringify(c.args)))
			).toEqual(c.expect);
		});
	}

	// §18 (b): a sheet opened without a link, its transaction not listed.
	for (const c of vectors.missing) {
		it(`missing: ${c.name}`, () => {
			const a = c.args as { loading: boolean; unreadable: boolean; mailbox: boolean | null };
			expect(missingSaid(a.loading, a.unreadable, a.mailbox)).toEqual(c.expect);
		});
	}

	it('says a sheet whose service did not answer as that, never "not shared yet"', () => {
		const proposal = {
			opening: 'o',
			cantCheck: 'c {{network}}',
			notInMailbox: 'm',
			notHere: 'h',
			unread: { linkService: 's' }
		};
		const said = missingSaid(false, false, false);
		expect(said.retry).toBe(true);
		expect(missingWords(proposal, said.key, 'Arc Testnet')).toBe('s');
		expect(missingWords(proposal, 'home.multisig.proposal.cantCheck', 'Arc Testnet')).toBe(
			'c Arc Testnet'
		);
	});

	it('a state the core cannot read is a shut door, never an open one', () => {
		const door = reviewDoor({ kind: 'nonsense' } as never, { sign: { allowed: true } });
		expect(door.enabled).toBe(false);
	});

	it('finds a state by its key in any case, and says not asked as null', () => {
		const states = { '100:0xAB': { kind: 'ready' as const } };
		expect(stateFor(states, 100, '0xab')).toEqual({ kind: 'ready' });
		expect(stateFor(states, 100, '0xcd')).toBeNull();
	});

	it('says each reason in its own words', () => {
		const m = {
			loading: 'l',
			blocked: 'b',
			timedOut: 't',
			unreadable: 'u',
			gone: 'g',
			tryAgain: 'r',
			unchecked: 'c',
			chainUnread: 'n {{network}}'
		};
		expect(reasonWords(m, 'home.multisig.review.state.timedOut')).toBe('t');
		expect(reasonWords(m, 'home.multisig.review.state.gone')).toBe('g');
		expect(reasonWords(m, 'home.multisig.review.state.unreadable')).toBe('u');
		expect(reasonWords(m, 'something.else')).toBe('u');
		// r14-13: the network that answered nothing, by name.
		expect(reasonWords(m, 'home.multisig.review.state.chainUnread', 'Arc Testnet')).toBe(
			'n Arc Testnet'
		);
	});
});
