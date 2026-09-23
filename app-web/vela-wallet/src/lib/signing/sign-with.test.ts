/**
 * "Sign with" on the signing sheet (spec 071, contract §6): the picker lists
 * what the core offers AND this shell has words for, a request starts at
 * Settings' default — not always `auto` — and a pick lies over it for that
 * request alone.
 *
 * The core offers `trusted_signer` to every shell; this one has no Trusted Signer
 * at all (owner, 2026-09-23), so it is never drawn and never in force. That is
 * the same rule an unknown name from a newer build already met.
 */
import { describe, expect, it } from 'vitest';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { signWithModel } from './live';

const m = resolveSigningMessages('en');
const CORE_OFFERS = ['auto', 'platform', 'hybrid', 'security_key', 'trusted_signer'];
const DRAWN = ['auto', 'platform', 'hybrid', 'security_key'];

const row = (defaultMethod: string, picked: string | null = null, offered = CORE_OFFERS) =>
	signWithModel({ offered, defaultMethod, picked, open: true, m });

describe('the sheet\u2019s "Sign with"', () => {
	it('lists the four it has words for, in the core\u2019s order', () => {
		const { row: model } = row('auto');
		expect(model.options.map((option) => option.id)).toEqual(DRAWN);
		expect(model.options.map((option) => option.title)).toEqual([
			m.signWithAuto,
			m.signWithPlatform,
			m.signWithHybrid,
			m.signWithSecurityKey
		]);
	});

	it('the Trusted Signer is never drawn here, and never in force', () => {
		const { row: model, method } = row('trusted_signer');
		expect(model.options.map((option) => option.id)).not.toContain('trusted_signer');
		// A stored default this shell cannot honour falls back rather than
		// selecting a row that is not there.
		expect(method).toBe('auto');
		expect(model.value).toBe(m.signWithAuto);
	});

	it('a request starts at the stored default, not at `auto`', () => {
		const started = row('hybrid');
		expect(started.method).toBe('hybrid');
		expect(started.row.value).toBe(m.signWithHybrid);
		expect(started.row.options.filter((option) => option.selected).map((o) => o.id)).toEqual([
			'hybrid'
		]);
	});

	it('a pick for this request lies over the default', () => {
		const picked = row('security_key', 'hybrid');
		expect(picked.method).toBe('hybrid');
		expect(picked.row.value).toBe(m.signWithHybrid);
	});

	it('a name this build has no words for is not drawn, and is not in force', () => {
		const { row: model, method } = row('auto', null, [...CORE_OFFERS, 'from_the_future']);
		expect(model.options.map((option) => option.id)).toEqual(DRAWN);
		expect(method).toBe('auto');
	});
});
