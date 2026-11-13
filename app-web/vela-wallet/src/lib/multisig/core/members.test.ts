/**
 * What a multisig can be made of, asked of the real core (ms-8, ms-25): a
 * member typed by address is refused where it is added when it is the zero
 * address or Safe's sentinel, and the permanent shared name takes nothing
 * invisible and nothing that turns the text around.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { validName } from './kernel';
import { memberProblem } from './members';

const A = '0x1111111111111111111111111111111111111111';

describe('a member typed by address', () => {
	it('is refused, with its reason, when it can never be a member', () => {
		expect(memberProblem('0x0000000000000000000000000000000000000000', [A])).toBe('notMember');
		expect(memberProblem('0x0000000000000000000000000000000000000001', [A])).toBe('notMember');
		expect(memberProblem('0x12', [A])).toBe('invalidAddress');
		expect(memberProblem(A.toUpperCase().replace('0X', '0x'), [A])).toBe('duplicate');
		expect(memberProblem(' 0x2222222222222222222222222222222222222222 ', [A])).toBeNull();
	});
});

describe("a multisig's name", () => {
	it('takes no bidirectional control and no zero-width character', () => {
		expect(validName('Team fund')).toBe(true);
		expect(validName('Team‮dnuf')).toBe(false);
		expect(validName('Team​fund')).toBe(false);
		expect(validName('⁦Team⁩')).toBe(false);
		expect(validName('﻿Team')).toBe(false);
		// An emoji's own joiner is part of the emoji.
		expect(validName('Family \u{1F468}‍\u{1F469}‍\u{1F467}')).toBe(true);
	});
});
