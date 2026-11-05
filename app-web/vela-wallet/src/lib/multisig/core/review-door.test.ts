/**
 * §12: the sign door every shell draws, replayed from the core's shared
 * vectors (`rust/crates/vela-core/tests/vectors/multisig/review-state.json`)
 * through the link door the web calls (`review.door`) — and the shell's own
 * fall-back: a door it could not ask for is shut.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import vectors from '../../../../../../rust/crates/vela-core/tests/vectors/multisig/review-state.json';
import { reasonWords, reviewDoor, stateFor } from './review-door';

describe('review state door (§12, shared vectors)', () => {
	for (const c of vectors.door) {
		it(c.name, () => {
			const args = c.args as { state?: unknown; review?: unknown };
			expect(reviewDoor((args.state ?? null) as never, args.review ?? null)).toEqual(c.expect);
		});
	}

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
			unchecked: 'c'
		};
		expect(reasonWords(m, 'home.multisig.review.state.timedOut')).toBe('t');
		expect(reasonWords(m, 'home.multisig.review.state.gone')).toBe('g');
		expect(reasonWords(m, 'home.multisig.review.state.unreadable')).toBe('u');
		expect(reasonWords(m, 'something.else')).toBe('u');
	});
});
