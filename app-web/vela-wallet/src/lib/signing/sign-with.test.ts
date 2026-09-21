/**
 * "Sign with" on the signing sheet, and the Clear Signer's own sheet (spec
 * 071, contract §5–6): the picker lists what the core offers, a request
 * starts at Settings' default — not always `auto` — and a pick lies over it
 * for that request alone; every ending of the Clear Signer's wait gets its
 * own sentence.
 */
import { describe, expect, it } from 'vitest';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { clearSignerModel, signWithModel } from './live';

const m = resolveSigningMessages('en');
const OFFERED = ['auto', 'platform', 'hybrid', 'security_key', 'clear_signer'];

const row = (defaultMethod: string, picked: string | null = null, offered = OFFERED) =>
	signWithModel({ offered, defaultMethod, picked, open: true, m });

describe('the sheet’s "Sign with"', () => {
	it('lists the five the core offers, in its order, by the words each is known by', () => {
		const { row: model } = row('auto');
		expect(model.options.map((option) => option.id)).toEqual(OFFERED);
		expect(model.options.map((option) => option.title)).toEqual([
			m.signWithAuto,
			m.signWithPlatform,
			m.signWithHybrid,
			m.signWithSecurityKey,
			m.signWithClearSigner
		]);
		expect(m.signWithClearSigner).toBe('Clear Signer');
		// Only the Clear Signer says what it is: the others are places a passkey is.
		expect(model.options.filter((option) => option.detail).map((option) => option.id)).toEqual([
			'clear_signer'
		]);
		expect(model.options.at(-1)?.detail).toBe(m.signWithClearSignerBody);
	});

	it('a request starts at the stored default, not at `auto`', () => {
		const started = row('clear_signer');
		expect(started.method).toBe('clear_signer');
		expect(started.row.value).toBe(m.signWithClearSigner);
		expect(started.row.options.filter((option) => option.selected).map((o) => o.id)).toEqual([
			'clear_signer'
		]);
	});

	it('a pick for this request lies over the default', () => {
		const picked = row('clear_signer', 'hybrid');
		expect(picked.method).toBe('hybrid');
		expect(picked.row.value).toBe(m.signWithHybrid);
	});

	it('a name this build has no words for is neither drawn nor in force', () => {
		const future = row('carrier_pigeon', 'carrier_pigeon', [...OFFERED, 'carrier_pigeon']);
		expect(future.method).toBe('auto');
		expect(future.row.options.map((option) => option.id)).toEqual(OFFERED);
	});
});

describe('the Clear Signer’s sheet', () => {
	it('while the page is open: waiting, the hint, open it again, cancel', () => {
		expect(clearSignerModel({ waiting: true, notice: null }, m)).toEqual({
			waiting: true,
			title: m.clearSignerWaiting,
			hint: m.clearSignerWaitingHint,
			reopen: m.clearSignerReopen,
			dismiss: m.clearSignerCancel
		});
	});

	it('each ending gets its own sentence, and a way to close it', () => {
		const said = (notice: 'closed' | 'refused' | 'mismatch' | 'timeout') =>
			clearSignerModel({ waiting: false, notice }, m);
		expect(said('closed')).toEqual({
			waiting: false,
			title: m.clearSignerClosed,
			dismiss: m.close
		});
		expect(said('refused')?.title).toBe(m.clearSignerRefused);
		expect(said('mismatch')?.title).toBe(m.clearSignerMismatch);
		expect(said('timeout')?.title).toBe(m.clearSignerTimeout);
		// Four different sentences: a refusal is not the person's cancel.
		expect(
			new Set(
				[said('closed'), said('refused'), said('mismatch'), said('timeout')].map((s) => s?.title)
			).size
		).toBe(4);
	});

	it('nothing open and nothing to say draws nothing', () => {
		expect(clearSignerModel({ waiting: false, notice: null }, m)).toBeNull();
	});
});
