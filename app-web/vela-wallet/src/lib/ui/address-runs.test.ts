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
import { addressHalves, addressRuns } from './address-runs';

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

/**
 * r16-21: a full address in a batch row broke wherever the line ran out
 * ("…ab3169 / 2D", "…C7C8 / 102e2"). As on the desktop (r15-10), a full
 * address breaks only between its two halves — 22 + 20 for an address —
 * never one digit on its own.
 */
describe('addressHalves', () => {
	it('splits a full address 22 + 20, the 0x with the first half', () => {
		const full = '0x9641d764fc13c8B624c04430C7356C1C7C8102e2';
		expect(addressHalves(full)).toEqual(['0x9641d764fc13c8B624c0', '4430C7356C1C7C8102e2']);
		expect(addressHalves(full)!.map((h) => h.length)).toEqual([22, 20]);
	});

	it('splits a hash near evenly, and leaves short forms, words and calldata whole', () => {
		const hash = `0x${'ab'.repeat(32)}`;
		const [head, tail] = addressHalves(hash)!;
		expect(head + tail).toBe(hash);
		expect(head.length).toBe(34);
		expect(addressHalves('0x5fb7e1…47B271')).toBeNull();
		expect(addressHalves('Arc Testnet')).toBeNull();
		expect(addressHalves(`0x${'00'.repeat(40)}`)).toBeNull();
		expect(addressHalves('0xnothexnothexnothexnothexnothexnothexnoth')).toBeNull();
	});
});
