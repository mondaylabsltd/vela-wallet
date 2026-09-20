/**
 * The batch importer's overlay (spec 026 T252).
 *
 * The rule this machine exists for: when nobody can price the chosen
 * currency, the importer refuses to convert rather than falling back to 1:1.
 * A 5,000 CNY payroll line converted at a defaulted rate pays ~7x. The core
 * says `rate_status: 'failed'` and `can_apply: false`; the screen must show
 * that refusal, not a number.
 *
 * And the rule issue 204 taught: the gate is never read without its reasons.
 * Every way `can_apply` can be false has a field, every field has a sentence in
 * the corpus, and each one is pinned below — so the next dark button with
 * nothing beside it fails here first.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import type { BatchView } from '$lib/core/generated/BatchView';
import { resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { preferences } from '$lib/services/preferences.svelte';
import { buildDesktopFlowState, buildFlowState } from './fixtures';
import { liveBatchImport, type BatchLiveInputs } from './live-batch';
import type { BatchRowModel } from './model';
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
	priced: true,
	rate_status: 'loading',
	rate_input: '',
	rate_edited: false,
	preview: [],
	over_cap: false,
	errors: [],
	rejected: 0,
	recipient_count: 0,
	total_token: '0',
	total_fiat: null,
	over_balance: false,
	can_apply: false,
	recipients: [],
	applied: false
};

const ADDRESS = '0x' + 'ab'.repeat(20);

const ROW = {
	line: 1,
	name: 'Alice',
	address: ADDRESS,
	valid: true,
	dup: false,
	raw_amount: '5000',
	token_amount: '690.13',
	ok: true
};

function view(over: Partial<BatchView>): BatchView {
	return { ...EMPTY, ...over };
}

function build(over: Partial<BatchView>, inputs: Partial<BatchLiveInputs> = {}) {
	return liveBatchImport(model(), {
		batch: view(over),
		m,
		symbol: 'USDT',
		balance: '1000',
		identicon,
		...inputs
	});
}

/** The parsed rows of a built model — the refused lines are not these. */
function rowsOf(built: ReturnType<typeof build>): BatchRowModel[] {
	return (built.preview?.rows ?? []).filter((row): row is BatchRowModel => row.kind === 'row');
}

beforeEach(() => {
	preferences.resetForTests();
});

describe('the parsed table', () => {
	it('shows each row as the core read it: who, what they get, what the sheet said', () => {
		const built = build({
			rate_status: 'ok',
			rate_input: '7.25',
			preview: [ROW],
			recipient_count: 1,
			total_token: '690.13',
			total_fiat: '5000',
			can_apply: true
		});
		expect(built.preview?.rows).toEqual([
			{
				kind: 'row',
				ok: true,
				name: 'Alice',
				// A name never stands in for the address it pays — both are shown.
				address: '0xababab…ababab',
				addressFull: ADDRESS,
				identiconSvg: '<svg/>',
				amount: '690.13 USDT',
				source: '5,000 CNY',
				note: undefined
			}
		]);
		expect(built.preview?.label).toContain('1');
		expect(built.ctaDisabled).toBe(false);
	});

	it('an address with no name is shown by its address alone', () => {
		const built = build({ preview: [{ ...ROW, name: null }] });
		expect(rowsOf(built)[0].name).toBeUndefined();
		expect(rowsOf(built)[0].addressFull).toBe(ADDRESS);
	});

	it('says WHY a row is skipped, not only that it is', () => {
		const built = build({ preview: [ROW, { ...ROW, line: 2, dup: true, ok: false }] });
		expect(rowsOf(built)[1].note).toBe(m['send.batchDup']);
		expect(rowsOf(built)[0].note).toBeUndefined();
	});

	it('counts the rows it READ above the list, not the rows it kept', () => {
		const built = build({
			preview: [ROW, { ...ROW, line: 2, dup: true, ok: false }],
			recipient_count: 1
		});
		expect(built.preview?.label).toBe('Parsed · 2 rows');
	});

	it('draws no list, and no "0 rows", before anything is brought', () => {
		expect(build({}).preview).toBeUndefined();
		expect(build({}).total).toBeUndefined();
	});

	it('counts skipped rows, singular and plural', () => {
		expect(build({ rejected: 1 }).notices[0]).toContain('1');
		expect(build({ rejected: 3 }).notices[0]).toContain('3');
		expect(build({ rejected: 0 }).notices).toEqual([]);
	});

	it('the cap and the skipped rows are said together — neither hides the other', () => {
		const built = build({ rejected: 2, over_cap: true, recipient_count: 60 });
		expect(built.notices).toHaveLength(2);
		expect(built.notices[1]).toBe('Only the first 60 recipients will be sent.');
	});
});

describe('the lines the parser refused', () => {
	const REFUSED = [
		{ line: 2, raw: 'Mallory , 0x12zz , 10', reason: 'no_address' as const },
		{ line: 4, raw: 'Carol , 0xabc… , ', reason: 'no_amount' as const }
	];

	it('are listed as written, with the reason — they used to be only a number', () => {
		const built = build({ preview: [ROW], errors: REFUSED, rejected: 2 });
		const refused = (built.preview?.rows ?? []).filter((row) => row.kind === 'refused');
		expect(refused).toEqual([
			{ kind: 'refused', text: 'Mallory , 0x12zz , 10', note: 'Invalid address' },
			{ kind: 'refused', text: 'Carol , 0xabc… , ', note: 'Not a valid amount' }
		]);
	});

	it('sit between the neighbours they have in the sheet', () => {
		const built = build({
			preview: [ROW, { ...ROW, line: 3, name: 'Bob' }],
			errors: REFUSED
		});
		expect(built.preview?.rows.map((row) => row.kind)).toEqual([
			'row',
			'refused',
			'row',
			'refused'
		]);
		// And the count above the list is the length of the list.
		expect(built.preview?.label).toBe('Parsed · 4 rows');
	});

	it('a sheet of nothing but refused lines still shows them', () => {
		const built = build({ errors: REFUSED, rejected: 2 });
		expect(built.preview?.rows).toHaveLength(2);
		expect(built.total).toBeUndefined();
		expect(built.ctaDisabled).toBe(true);
	});

	it('a core built before the list existed reads as none', () => {
		const old = { ...view({ preview: [ROW] }) } as Record<string, unknown>;
		delete old.errors;
		const built = liveBatchImport(model(), {
			batch: old as never,
			m,
			symbol: 'USDT',
			balance: '1',
			identicon
		});
		expect(built.preview?.rows).toHaveLength(1);
	});
});

describe('what it adds up to (issue 204)', () => {
	const FILLED = {
		rate_status: 'ok' as const,
		rate_input: '7.25',
		preview: [ROW],
		recipient_count: 1,
		total_token: '1793.103448',
		total_fiat: '13000'
	};

	it('shows the total beside the balance it is read against', () => {
		const built = build({ ...FILLED, can_apply: true }, { balance: '2500.5' });
		expect(built.total).toEqual({
			label: 'Total · 1 recipient',
			value: '1793.103448 USDT',
			detail: '13,000 CNY',
			balance: 'Balance 2500.5 USDT',
			over: false,
			overText: undefined
		});
	});

	it('a total over the balance is refused IN WORDS, beside the figure', () => {
		const built = build({ ...FILLED, over_balance: true, can_apply: false }, { balance: '53' });
		expect(built.ctaDisabled).toBe(true);
		expect(built.total?.over).toBe(true);
		expect(built.total?.overText).toBe('Total exceeds your USDT balance.');
	});

	it('never trims a figure that is about to be sent', () => {
		const built = build({
			...FILLED,
			preview: [{ ...ROW, token_amount: '0.00022989' }],
			total_token: '0.00022989'
		});
		expect(rowsOf(built)[0].amount).toBe('0.00022989 USDT');
		expect(built.total?.value).toBe('0.00022989 USDT');
	});

	it('token mode has no sheet-currency sum', () => {
		const built = build({ ...FILLED, unit: 'token', total_fiat: null });
		expect(built.total?.detail).toBeUndefined();
		expect(rowsOf(built)[0].source).toBeUndefined();
	});
});

describe('the rate (issue 206)', () => {
	it('reads as one sentence with its unit: 1 USDT ≈ 7.25 CNY', () => {
		const rate = build({ rate_status: 'ok', rate_input: '7.25' }).rate;
		expect(rate).toMatchObject({ lead: '1 USDT', sign: '≈', value: '7.25', code: 'CNY' });
		expect(rate?.hint).toContain('CNY');
		expect(rate?.hint).toContain('USDT');
		expect(rate?.hintTone).toBe('plain');
	});

	it('a rate the person typed is exact, and says so', () => {
		const rate = build({ rate_status: 'ok', rate_input: '7.3', rate_edited: true }).rate;
		expect(rate?.sign).toBe('=');
		expect(rate?.edited).toBe(true);
	});

	it('wears the person’s decimal mark — "7.558" is seven thousand to a dot-grouper', () => {
		preferences.setNumberFormat('dot_comma');
		expect(build({ rate_status: 'ok', rate_input: '7.558' }).rate?.value).toBe('7,558');
	});

	it('is not drawn at all in token mode, where nothing is converted', () => {
		expect(build({ unit: 'token', rate_status: 'ok', rate_input: '7.25' }).rate).toBeUndefined();
	});
});

describe('an unpriceable currency', () => {
	it('shows the rate as unknown and leaves the CTA disabled — never 1:1', () => {
		const built = build({
			rate_status: 'failed',
			preview: [{ ...ROW, token_amount: '', ok: false }],
			rejected: 1,
			can_apply: false
		});
		// The failed state has its own sentence, in the colour for "look at this".
		expect(built.rate?.value).toBe('');
		expect(built.rate?.hint).toBe(m['send.batchRateFailed']);
		expect(built.rate?.hintTone).toBe('warning');
		// The unconvertible row shows a dash, never a token figure — and keeps
		// what the sheet WROTE beside it.
		expect(rowsOf(built)[0].amount).toBe('—');
		expect(rowsOf(built)[0].source).toBe('5,000 CNY');
		expect(built.ctaDisabled).toBe(true);
	});

	it('waits visibly while the rate is still being fetched', () => {
		const built = build({ rate_status: 'loading' });
		expect(built.rate?.hint).toBe(m['send.batchRateLoading']);
		expect(built.ctaDisabled).toBe(true);
	});

	it('a token with no market price asks for a rate, in its own words', () => {
		const built = build({ rate_status: 'ok', priced: false });
		expect(built.rate?.hint).toBe(m['send.batchNoPrice']);
		expect(built.rate?.hintTone).toBe('warning');
	});
});

describe('bringing the list (issue 205)', () => {
	it('each tool says what state it is in', () => {
		expect(build({}).tools.file).toEqual({ label: 'Import file', busy: false });
		expect(build({ busy: true }).tools.file).toEqual({ label: 'Reading…', busy: true });
		expect(build({}).tools.template).toEqual({ label: 'Get template', saved: false });
		expect(build({ template_saved: true }).tools.template).toEqual({
			label: 'Template saved',
			saved: true
		});
	});

	it('a file that could not be read says so — until a list parses', () => {
		expect(build({ file_error: true }).tools.error).toContain('Could not read file');
		// The core keeps the flag until the next pick; a parsed list outranks it.
		expect(build({ file_error: true, preview: [ROW] }).tools.error).toBeUndefined();
	});

	it('names the picked file, because a workbook leaves the paste box empty', () => {
		expect(build({ file_name: 'payroll.xlsx' }).tools.fileName).toBe('payroll.xlsx');
		expect(build({}).tools.fileName).toBeUndefined();
	});
});

describe('what nobody had said', () => {
	it('token mode says the sheet’s figures ARE the amounts, where the rate would be', () => {
		const built = build({ unit: 'token' });
		expect(built.unitHint).toBe('Amounts in the sheet are read as USDT, exactly as written.');
		expect(build({ unit: 'fiat' }).unitHint).toBeUndefined();
	});

	it('says what importing does to the people already on the form, and offers the other', () => {
		const ready = { preview: [ROW], recipient_count: 1, can_apply: true };
		// Adding is the default; replacing is one press away and says so first.
		expect(build(ready, { formHasRows: true }).merge).toEqual({
			note: m['send.batchAddsToRows'],
			action: m['send.batchReplaceInstead']
		});
		expect(build(ready, { formHasRows: true, replaces: true }).merge).toEqual({
			note: m['send.batchReplacesRows'],
			action: m['send.batchAddInstead']
		});
		// Nobody on the form, nothing to say; nothing importable, nothing to choose.
		expect(build(ready, { formHasRows: false }).merge).toBeUndefined();
		expect(build({}, { formHasRows: true }).merge).toBeUndefined();
	});

	it('reads the total against what is LEFT when it adds to a form that has given some out', () => {
		const ready = { preview: [ROW], recipient_count: 1, total_token: '0.3', can_apply: true };
		const adding = build(ready, { formHasRows: true, remaining: '2.5', balance: '3' });
		expect(adding.total?.balance).toBe('2.5 USDT left');
		// Replacing draws from the whole balance again, and so does an empty form.
		expect(
			build(ready, { formHasRows: true, replaces: true, remaining: '2.5', balance: '3' }).total
				?.balance
		).toBe('Balance 3 USDT');
		expect(build(ready, { balance: '3' }).total?.balance).toBe('Balance 3 USDT');
	});

	it('asks the question the unit toggle answers', () => {
		expect(build({}).unitCaption).toBe('How the amounts are written');
	});
});

describe('the unit toggle', () => {
	it('follows the core, and the pasted text is the core’s copy of it', () => {
		const built = build({ unit: 'token', raw_text: '0xabc,5' });
		expect(built.unit).toBe('token');
		expect(built.pasteValue).toBe('0xabc,5');
	});
});

describe('the words and the shape (spec 038 E6)', () => {
	it('names the currency in force and the token being split, and offers what parsed', () => {
		const live = build(
			{ fiat_code: 'USD', rate_input: '1', preview: [ROW], recipient_count: 1, can_apply: true },
			{ symbol: 'XDAI' }
		);
		expect(live.units).toEqual({ fiat: 'In USD', token: 'In XDAI' });
		expect(live.rate?.lead).toBe('1 XDAI');
		expect(live.rate?.code).toBe('USD');
		expect(live.cta).toBe('Import 1 recipient');
	});

	it('overlays the desktop column body, where the importer is not a sheet', () => {
		const drawn = buildDesktopFlowState('dsd2c', m, identicon);
		if (drawn.body.kind !== 'batch-import') throw new Error('kind');
		const batch = view({ fiat_code: 'USD', raw_text: '0xabc, 5', unit: 'token' });
		const live = withLiveDesktopFlow(drawn, {
			batch: { batch, m, symbol: 'XDAI', balance: '3', identicon }
		} as unknown as Parameters<typeof withLiveDesktopFlow>[1]);
		if (live.body.kind !== 'batch-import') throw new Error('kind');
		expect(live.body.model.unit).toBe('token');
		expect(live.body.model.pasteValue).toBe('0xabc, 5');
		expect(live.body.model.units.fiat).toBe('In USD');
		// Nothing of the drawn picture survives into a live, empty importer.
		expect(live.body.model.preview).toBeUndefined();
		expect(live.body.model.total).toBeUndefined();
		expect(live.body.model.notices).toEqual([]);
		expect(live.body.model.cta).toBe('Import recipients');
	});
});
