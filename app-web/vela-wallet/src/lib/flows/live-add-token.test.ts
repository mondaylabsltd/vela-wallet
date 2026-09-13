/**
 * The add-token sheet, read off `manage_tokens` (spec 028 T444 — US4).
 *
 * The drawn sheet has one field, one result card and one CTA, and T5 draws
 * every failure as a variant of those. Each variant below is a projection of
 * the core's view — validity, the probe, the identity the chain answered with,
 * the dedupe verdict, a failed write — and the shell only words it. The one
 * decision that looks like the shell's (which card is shown when several
 * chains answer) is the first in registry order, which is the core's order.
 */
import { describe, expect, it } from 'vitest';
import type { MtokView } from '$lib/core/generated/MtokView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { buildFlowState } from './fixtures';
import { liveAddToken } from './live';

const m = resolveWalletFlowMessages('en');
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const CONTRACT = '0x' + 'c0ffee'.repeat(6) + 'c0ff';

const EMPTY: MtokView = {
	input_address: '',
	address_valid: false,
	detecting: false,
	found: [],
	saving: false,
	custom_tokens: [],
	not_found: false,
	save_error: false
};

const FOUND = {
	chain_id: 1,
	network_name: 'Ethereum',
	name: 'Coffee Token',
	symbol: 'COFFEE',
	decimals: 18,
	added: false
};

function sheet() {
	const model = buildFlowState('t3', m, identicon).sheet;
	if (model?.kind !== 'add-token') throw new Error('not the add-token sheet');
	return model.model;
}

function live(view: Partial<MtokView>) {
	return liveAddToken(sheet(), { view: { ...EMPTY, ...view }, m });
}

describe('the field', () => {
	it('echoes what was typed and says nothing about an empty one', () => {
		const model = live({});
		expect(model.fieldValue).toBe('');
		expect(model.fieldError).toBeUndefined();
		expect(model.result).toEqual({ kind: 'none' });
		expect(model.ctaDisabled).toBe(true);
	});

	it("flags an address the core refuses — T5's error state", () => {
		const model = live({ input_address: '0x1234', address_valid: false });
		expect(model.fieldError).toBe(m['addToken.invalidAddress']);
		expect(model.ctaDisabled).toBe(true);
	});
});

describe('the result card', () => {
	it('says the probe is running while it is', () => {
		const model = live({ input_address: CONTRACT, address_valid: true, detecting: true });
		expect(model.result).toEqual({ kind: 'searching', text: m['addToken.searchingNetworks'] });
		expect(model.ctaDisabled).toBe(true);
	});

	it("carries the chain's own identity, and arms the CTA", () => {
		const model = live({ input_address: CONTRACT, address_valid: true, found: [FOUND] });
		expect(model.result).toMatchObject({
			kind: 'token',
			name: 'Coffee Token',
			detail: `COFFEE · ${m['tokenDetail.labelDecimals']} 18 · Ethereum`
		});
		expect((model.result as { chip?: unknown }).chip).toBeUndefined();
		expect(model.cta).toBe(m['addToken.addToWalletBtn']);
		expect(model.ctaDisabled).toBe(false);
	});

	it("shows the core's dedupe verdict as the added chip, and disarms", () => {
		// Whether this token is already stored is the core's fresh read at save
		// time, not a list the sheet keeps.
		const model = live({
			input_address: CONTRACT,
			address_valid: true,
			found: [{ ...FOUND, added: true }]
		});
		expect(model.result).toMatchObject({ kind: 'token', chip: { text: m['addToken.tokenAdded'] } });
		expect(model.ctaDisabled).toBe(true);
	});

	it("says 'not found' with both sentences when no chain answered", () => {
		const model = live({ input_address: CONTRACT, address_valid: true, not_found: true });
		expect(model.result).toEqual({
			kind: 'not-found',
			text: `${m['addToken.notFoundTitle']} — ${m['addToken.notFoundMessage']}`
		});
		expect(model.ctaDisabled).toBe(true);
	});

	it('takes the first chain that answered when several did', () => {
		const model = live({
			input_address: CONTRACT,
			address_valid: true,
			found: [FOUND, { ...FOUND, chain_id: 137, network_name: 'Polygon' }]
		});
		expect(model.result).toMatchObject({
			kind: 'token',
			detail: expect.stringContaining('Ethereum')
		});
	});
});

describe('the write', () => {
	it('disarms the CTA while the save is in flight', () => {
		const model = live({
			input_address: CONTRACT,
			address_valid: true,
			found: [FOUND],
			saving: true
		});
		expect(model.ctaDisabled).toBe(true);
	});

	it('says a failed write failed, in the field, and leaves the CTA live', () => {
		// The core's `save_error` is a one-shot flag; a person has to be able to
		// try again from the same screen.
		const model = live({
			input_address: CONTRACT,
			address_valid: true,
			found: [FOUND],
			save_error: true
		});
		expect(model.fieldError).toBe(m['addToken.errorSaveToken']);
		expect(model.ctaDisabled).toBe(false);
	});
});
