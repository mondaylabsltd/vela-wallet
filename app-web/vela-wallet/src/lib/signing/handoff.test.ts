/**
 * Spec 102 (D4): the hand-off card's boards. What must hold, in every locale:
 * no preview (the page is the authority), the key as a row the core labels
 * (D-17 — its label from `label_key`, its value the key's name or place), the
 * integrity line in words with the core's time (D-13), Open shut when the
 * check refused the page or has not landed, and the fee said once — by the
 * sheet's own fee row above the card, never again on it (D-18).
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
				// The key row: the page's own label, and never the wallet's name —
				// the "Signing account" row above already says it (D-17).
				expect(model.handoff?.key.label, `${locale} ${state}`).toBe(
					m.keyLabels['componentsUi.signing.confirmWithLabel']
				);
				expect(model.handoff?.key.label.length, locale).toBeGreaterThan(0);
				expect(model.handoff?.key.value.length, locale).toBeGreaterThan(0);
				expect(model.handoff?.key.value, locale).not.toContain('大表哥');
				// D-18: the sheet's own fee row and signing account stay; the card
				// carries no fee of its own.
				expect(model.fee, `${locale} ${state}`).toEqual(base.fee);
				expect(model.signer, `${locale} ${state}`).toEqual(base.signer);
				expect(model.handoff, `${locale} ${state}`).not.toHaveProperty('fee');
				// D-13: checked at the core's time (a refused or running check names none).
				if (state === 'ho1' || state === 'ho3')
					expect(model.handoff?.integrity.text, locale).toContain('14:32');
				// The header keeps the minimal context: who asked, on which network.
				expect(model.dapp).toEqual(base.dapp);
			}
		}
	});

	it('the key row and the title, in the page’s words', () => {
		const base = buildSigningState('cs1', resolveSigningMessages('zh'), identicon);
		const m = resolveHandoffMessages('zh');
		const ready = buildHandoffState('ho1', base, m, 'zh').handoff!;
		expect(ready.title).toBe('在可信签名页上预览并签名');
		expect(ready.key).toEqual({ label: '确认方式', value: 'YubiKey 5C' });
		expect(buildHandoffState('ho2', base, m, 'zh').handoff!.key).toEqual({
			label: '确认方式',
			value: '手机或平板'
		});
		// The ceremony's label is carried too, for a card that makes a key.
		expect(m.keyLabels['componentsUi.signing.newKeyOnLabel']).toBe('新钥匙存在');
	});

	it('Open only for a page the check let through; the refusal says it was not opened', () => {
		const base = buildSigningState('cs1', resolveSigningMessages('en'), identicon);
		const m = resolveHandoffMessages('en');
		const ready = buildHandoffState('ho1', base, m).handoff!;
		expect(ready.open.enabled).toBe(true);
		expect(ready.integrity).toEqual({
			text: "Version 0ba8ee8c\u00a0· matches Vela's published build list\u00a0· checked 14:32",
			tone: 'ok'
		});
		expect(ready.title).toBe('Review and sign on a trusted signing page');
		// A key the person named is named; one that carries the wallet's name
		// is named by its place (D-17).
		expect(ready.key).toEqual({ label: 'Confirm with', value: 'YubiKey 5C' });
		const refused = buildHandoffState('ho2', base, m).handoff!;
		expect(refused.key).toEqual({ label: 'Confirm with', value: 'Phone or tablet' });
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

	it('while the check runs: no verdict, no Open — the card HO1 becomes once it lands', () => {
		const base = buildSigningState('cs1', resolveSigningMessages('en'), identicon);
		const m = resolveHandoffMessages('en');
		const checking = buildHandoffState('ho4', base, m).handoff!;
		const checked = buildHandoffState('ho1', base, m).handoff!;
		expect(checking.integrity).toEqual({ text: m.integrity.checking, tone: 'checking' });
		expect(checking.open).toEqual({ ...checked.open, enabled: false });
		expect({ ...checking, integrity: checked.integrity, open: checked.open }).toEqual(checked);
	});
});
