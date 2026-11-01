/**
 * r11-6: a member is named ONE way in every title — the core's `names.who`
 * ("Interleave (0x88cCA0…266894)", or the short form alone), with the names
 * as they stand when the line is drawn. Activity drew "Add 0x88cCA0…266894"
 * once and kept it after the index answered; the sheet said "Add Interleave
 * (0x88cCA0…266894)".
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { memberActionTitle, memberWho } from './display';
import type { MultisigMessages } from '../messages';
import type { Action } from './kernel';

const INTERLEAVE = '0x88cca0eedbf2c4426110bbfc998f048689266894';
const m = {
	change: {
		add: 'Add {{member}}',
		remove: 'Remove {{member}}',
		replace: 'Replace {{old}} with {{new}}'
	}
} as unknown as MultisigMessages;

describe('r11-6: one title rule for a member', () => {
	it("is the core's: the label and the 6+6 short form, or the short form alone", () => {
		expect(memberWho(INTERLEAVE, 'Interleave')).toBe('Interleave (0x88cCA0…266894)');
		expect(memberWho(INTERLEAVE, null)).toBe('0x88cCA0…266894');
		expect(memberWho(INTERLEAVE, '0x88cCA0…266894')).toBe('0x88cCA0…266894');
		expect(memberWho(INTERLEAVE, '  ')).toBe('0x88cCA0…266894');
	});

	it('names a member change with the names as they stand now', () => {
		const add = { kind: 'addOwner', owner: INTERLEAVE, threshold: 2 } as unknown as Action;
		let label: string | null = null;
		const who = (a: string) => memberWho(a, label);
		expect(memberActionTitle(m, add, who)).toBe('Add 0x88cCA0…266894');
		label = 'Interleave'; // the passkey index answered
		expect(memberActionTitle(m, add, who)).toBe('Add Interleave (0x88cCA0…266894)');
		const send = { kind: 'transfer', token: null, to: INTERLEAVE, amount: '1' } as Action;
		expect(memberActionTitle(m, send, who)).toBeNull();
	});
});
