/**
 * Spec 102 (D4): the hand-off card's boards. What must hold, in every locale:
 * no preview (the page is the authority), the key named, the integrity line
 * in words, and Open shut when the check refused the page.
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
				const model = buildHandoffState(state, base, m);
				expect(model.blocks, `${locale} ${state}`).toEqual([]);
				expect(JSON.stringify(model.handoff), `${locale} ${state}`).not.toContain('{{');
				expect(model.handoff?.key, locale).toContain('大表哥');
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
		expect(ready.title).toBe('Review and sign on your trusted page');
		const refused = buildHandoffState('ho2', base, m).handoff!;
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
});
