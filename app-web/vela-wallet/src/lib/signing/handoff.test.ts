/**
 * Spec 102 (D4): the hand-off card's boards. What must hold, in every locale:
 * no preview (the page is the authority), the key named as the core labels it
 * (D-17), the integrity line in words with the core's time (D-13), the fee
 * row only as the core's `handoff_fee` gives it (D-18), and Open shut when
 * the check refused the page.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { resolveHandoffMessages, resolveSigningMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { buildSigningState } from './fixtures';
import { buildHandoffState, HANDOFF_STATES } from './handoff';

const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;

describe('the hand-off boards', () => {
	it('fill every template and repeat no preview, in every locale', () => {
		for (const locale of SUPPORTED_LOCALES) {
			const base = buildSigningState('cs1', resolveSigningMessages(locale), identicon);
			const m = resolveHandoffMessages(locale);
			for (const state of HANDOFF_STATES) {
				const model = buildHandoffState(state, base, m, locale);
				expect(model.blocks, `${locale} ${state}`).toEqual([]);
				expect(JSON.stringify(model.handoff), `${locale} ${state}`).not.toContain('{{');
				// D-17: never the wallet's name — the "Signing account" row says it.
				expect(model.handoff?.key, locale).not.toContain('大表哥');
				// D-18: the fee and the speed the sheet settled, one quiet row.
				expect(model.handoff?.fee?.label, locale).toBe(m.feeLabel);
				expect(model.handoff?.fee?.value, locale).toContain('ETH');
				expect(model.handoff?.fee?.value, locale).toContain(m.tiers['send.gasTier.standard']);
				// D-13: checked at the core's time (a refused page's line names none).
				if (state !== 'ho2') expect(model.handoff?.integrity.text, locale).toContain('14:32');
				// The header keeps the minimal context: who asked, on which network.
				expect(model.dapp).toEqual(base.dapp);
			}
		}
	});

	it('Open only for a page the check let through; the refusal says it was not opened', () => {
		const base = buildSigningState('cs1', resolveSigningMessages('en'), identicon);
		const m = resolveHandoffMessages('en');
		const ready = buildHandoffState('ho1', base, m).handoff!;
		expect(ready.open.enabled).toBe(true);
		expect(ready.integrity).toEqual({
			text: "Version 0ba8ee8c · matches Vela's published build list · checked 14:32",
			tone: 'ok'
		});
		expect(ready.title).toBe('Review and sign on your trusted signing page');
		// A key the person named is named; one that carries the wallet's name
		// is named by its place (D-17).
		expect(ready.key).toBe('Confirm with YubiKey 5C');
		expect(ready.fee).toEqual({ label: 'Network fee', value: '0.00042 ETH · Standard' });
		const refused = buildHandoffState('ho2', base, m).handoff!;
		expect(refused.key).toBe('Confirm with Phone or tablet');
		expect(refused.open.enabled).toBe(false);
		expect(refused.integrity.tone).toBe('error');
		expect(refused.integrity.text).toContain('Not opened');
		const waiting = buildHandoffState('ho3', base, m).handoff!;
		expect(waiting.waiting).toEqual({
			title: m.waiting,
			hint: m.waitingHint,
			reopen: m.reopen
		});
	});

	it('draws no fee row while the fee is not settled for the speed in force', async () => {
		const { handoffFeeRow } = await import('$lib/core/kernels');
		const { HANDOFF_FEE, HANDOFF_SPEED } = await import('./handoff');
		expect(handoffFeeRow({ ...HANDOFF_FEE, busy: true }, HANDOFF_SPEED)).toBeNull();
		expect(handoffFeeRow(HANDOFF_FEE, { ...HANDOFF_SPEED, tier: 'fast' })).toBeNull();
		expect(handoffFeeRow({ ...HANDOFF_FEE, confirm_fee_ready: false }, HANDOFF_SPEED)).toBeNull();
		// One speed on the network: the fee alone, no speed to restate.
		expect(handoffFeeRow(HANDOFF_FEE, { ...HANDOFF_SPEED, single: true })).toMatchObject({
			tier: null,
			tier_key: null
		});
	});
});
