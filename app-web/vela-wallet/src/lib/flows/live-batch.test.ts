/**
 * The batch importer's overlay (spec 026 T252).
 *
 * The rule this machine exists for: when nobody can price the chosen
 * currency, the importer refuses to convert rather than falling back to 1:1.
 * A 5,000 CNY payroll line converted at a defaulted rate pays ~7x. The core
 * says `rate_status: 'failed'` and `can_apply: false`; the screen must show
 * that refusal, not a number.
 */
import { describe, expect, it } from 'vitest';
import type { BatchView } from '$lib/core/generated/BatchView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { buildDesktopFlowState, buildFlowState } from './fixtures';
import { liveBatchImport } from './live-batch';
import { withLiveDesktopFlow } from './live';

const m = resolveWalletFlowMessages('en');
const identicon = () => '<svg/>';

const model = () => {
	const state = buildFlowState('sd2c', m, identicon);
	if (state.sheet?.kind !== 'batch-import') throw new Error('kind');
	return state.sheet.model;
};

const EMPTY: BatchView = {
	opened: true,
	unit: 'fiat',
	fiat_code: 'CNY',
	raw_text: '',
	file_name: null,
	busy: false,
	file_error: false,
	template_saved: false,
	priced: false,
	rate_status: 'loading',
	rate_input: '',
	rate_edited: false,
	preview: [],
	over_cap: false,
	rejected: 0,
	recipient_count: 0,
	total_token: '0',
	total_fiat: null,
	over_balance: false,
	can_apply: false,
	recipients: [],
	applied: false
};

const ROW = {
	line: 1,
	name: 'Alice',
	address: '0x' + 'ab'.repeat(20),
	valid: true,
	dup: false,
	raw_amount: '5000',
	token_amount: '690.13',
	ok: true
};

function view(over: Partial<BatchView>): BatchView {
	return { ...EMPTY, ...over };
}

describe('the parsed table', () => {
	it('shows each row as the core read it, converted into the token', () => {
		const built = liveBatchImport(model(), {
			batch: view({
				rate_status: 'ok',
				rate_input: '7.25',
				preview: [ROW],
				recipient_count: 1,
				can_apply: true
			}),
			m,
			symbol: 'USDT'
		});
		expect(built.rows).toEqual([{ ok: true, address: 'Alice', conversion: '690.13 USDT' }]);
		expect(built.rateValue).toBe('7.25 CNY');
		expect(built.parsedLabel).toContain('1');
		expect(built.ctaDisabled).toBe(false);
	});

	it('an address with no name is shown by its address', () => {
		const built = liveBatchImport(model(), {
			batch: view({ preview: [{ ...ROW, name: null }] }),
			m,
			symbol: 'USDT'
		});
		expect(built.rows[0].address).toBe(ROW.address);
	});

	it('counts skipped rows, singular and plural', () => {
		expect(
			liveBatchImport(model(), { batch: view({ rejected: 1 }), m, symbol: 'USDT' }).rejectedText
		).toContain('1');
		expect(
			liveBatchImport(model(), { batch: view({ rejected: 3 }), m, symbol: 'USDT' }).rejectedText
		).toContain('3');
		expect(
			liveBatchImport(model(), { batch: view({ rejected: 0 }), m, symbol: 'USDT' }).rejectedText
		).toBeUndefined();
	});
});

describe('an unpriceable currency', () => {
	it('shows the rate as unknown and leaves the CTA disabled — never 1:1', () => {
		const built = liveBatchImport(model(), {
			batch: view({
				rate_status: 'failed',
				preview: [{ ...ROW, token_amount: '', ok: false }],
				rejected: 1,
				can_apply: false
			}),
			m,
			symbol: 'USDT'
		});
		// Spec 038 #E6: the failed state has its own sentence; the hint is the
		// paragraph under the (now editable) rate.
		expect(built.rateValue).toBe(m['send.batchRateFailed']);
		expect(built.rateValue).not.toContain('1');
		// The unconvertible row shows what was WRITTEN, never a token figure.
		expect(built.rows[0].conversion).toBe('5000');
		expect(built.ctaDisabled).toBe(true);
	});

	it('waits visibly while the rate is still being fetched', () => {
		const built = liveBatchImport(model(), {
			batch: view({ rate_status: 'loading' }),
			m,
			symbol: 'USDT'
		});
		expect(built.rateValue).toBe(m['send.batchRateLoading']);
		expect(built.ctaDisabled).toBe(true);
	});
});

describe('the unit toggle', () => {
	it('follows the core, and the pasted text is the core’s copy of it', () => {
		const built = liveBatchImport(model(), {
			batch: view({ unit: 'token', raw_text: '0xabc,5' }),
			m,
			symbol: 'USDT'
		});
		expect(built.unit).toBe('token');
		expect(built.pasteValue).toBe('0xabc,5');
	});
});

describe('the words and the shape (spec 038 #E6)', () => {
	it('names the currency in force and the token being split, and offers what parsed', () => {
		const live = liveBatchImport(model(), {
			batch: view({ fiat_code: 'USD', preview: [ROW], recipient_count: 1, can_apply: true }),
			m,
			symbol: 'XDAI'
		});
		expect(live.units).toEqual({ fiat: 'In USD', token: 'In XDAI' });
		expect(live.rateLabel).toContain('XDAI');
		expect(live.rateHint).toContain('USD');
		expect(live.rateHint).toContain('XDAI');
		expect(live.cta).toBe('Import 1 recipient');
	});

	it('overlays the desktop column body, where the importer is not a sheet', () => {
		const drawn = buildDesktopFlowState('dsd2c', m, identicon);
		if (drawn.body.kind !== 'batch-import') throw new Error('kind');
		const batch = view({ fiat_code: 'USD', raw_text: '0xabc, 5', unit: 'token' });
		const live = withLiveDesktopFlow(drawn, {
			batch: { batch, m, symbol: 'XDAI' }
		} as unknown as Parameters<typeof withLiveDesktopFlow>[1]);
		if (live.body.kind !== 'batch-import') throw new Error('kind');
		expect(live.body.model.unit).toBe('token');
		expect(live.body.model.pasteValue).toBe('0xabc, 5');
		expect(live.body.model.units.fiat).toBe('In USD');
		expect(live.body.model.rows).toEqual([]);
		expect(live.body.model.cta).toBe('Import recipients');
	});
});
