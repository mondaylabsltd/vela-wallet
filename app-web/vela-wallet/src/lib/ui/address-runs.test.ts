/**
 * r14-11a: a short address never breaks across lines.
 *
 * The zh side panel drew a pending multisig transfer as "已批准 1/2 · 至
 * 0x5fb7e1… / 47B271": Unicode line breaking allows a break after "…", and a
 * CJK line takes it. The text is the core's, one string; `addressRuns` finds
 * the short address forms in it so they are drawn as one unbreakable run —
 * the words around them still wrap, and the address moves to the next line
 * whole.
 */
import { describe, expect, it } from 'vitest';
import { addressRuns } from './address-runs';

describe('addressRuns', () => {
	it('finds the short form inside a sentence, in any language', () => {
		expect(addressRuns('已批准 1/2 · 至 0x5fb7e1…47B271')).toEqual([
			{ text: '已批准 1/2 · 至 ', address: false },
			{ text: '0x5fb7e1…47B271', address: true }
		]);
		expect(addressRuns('Add 0x5fb7e1…47B271 as a member')).toEqual([
			{ text: 'Add ', address: false },
			{ text: '0x5fb7e1…47B271', address: true },
			{ text: ' as a member', address: false }
		]);
	});

	it('finds every one, the 6+4 form and the three-dot form too', () => {
		expect(addressRuns('0x14fB1f…D1eA5c → 0xAbCd…1234 (0x1234ab...cd5678)')).toEqual([
			{ text: '0x14fB1f…D1eA5c', address: true },
			{ text: ' → ', address: false },
			{ text: '0xAbCd…1234', address: true },
			{ text: ' (', address: false },
			{ text: '0x1234ab...cd5678', address: true },
			{ text: ')', address: false }
		]);
	});

	it('leaves a whole address, and text without one, as it is', () => {
		const full = 'To 0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';
		expect(addressRuns(full)).toEqual([{ text: full, address: false }]);
		expect(addressRuns('Arc Testnet')).toEqual([{ text: 'Arc Testnet', address: false }]);
		expect(addressRuns('')).toEqual([]);
	});
});
