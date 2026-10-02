/**
 * Issue #328 (reported on Android; the web's stack has the same shape): a
 * closed sheet must take its own level off the flow stack. Left on top, the
 * same row tapped again pushed the same step, which `push` ignores — nothing
 * opened — and the first ‹ only took the invisible sheet away.
 */
import { describe, expect, it } from 'vitest';
import { FlowNav } from './nav.svelte';

describe('FlowNav.sheetClosed', () => {
	it('a closed token sheet takes its level with it, and the next row opens', () => {
		const nav = new FlowNav();
		nav.enter('assets');
		nav.push('token-detail');
		expect(nav.mobile).toEqual(['t1', 't2']);

		nav.sheetClosed('t2');
		expect(nav.mobile).toEqual(['t1']);
		nav.push('token-detail');
		expect(nav.mobile).toEqual(['t1', 't2']);
	});

	it('without it the next push is swallowed — the reported failure', () => {
		const nav = new FlowNav();
		nav.enter('assets');
		nav.push('token-detail');
		nav.push('token-detail');
		expect(nav.mobile).toEqual(['t1', 't2']);
	});

	it('ten closes in a row each leave every pushed sheet able to open again', () => {
		const cases: [Parameters<FlowNav['enter']>[0], string, string][] = [
			['assets', 'token-detail', 't2'],
			['activity', 'tx-detail', 'a2'],
			['receive', 'receive-qr', 'r2'],
			['assets', 'add-token', 't3']
		];
		for (const [entry, step, sheet] of cases) {
			const nav = new FlowNav();
			nav.enter(entry);
			const root = [...nav.mobile];
			for (let i = 0; i < 10; i++) {
				nav.push(step);
				expect(nav.mobileTop, `${step} opens (#${i})`).toBe(sheet);
				nav.sheetClosed(nav.mobileTop!);
				expect(nav.mobile, `${step} closes onto its list (#${i})`).toEqual(root);
			}
		}
	});

	it('back after a close leaves the flow at once', () => {
		const nav = new FlowNav();
		nav.enter('token-detail');
		nav.sheetClosed('t2');
		nav.back();
		expect(nav.mobile).toEqual([]);
	});

	it('a close for a level that is not on top pops nothing, twice included', () => {
		const nav = new FlowNav();
		nav.enter('send');
		nav.push('send-form');
		// The fee-coin sheet is the send machine's, never pushed.
		nav.sheetClosed('sd2f');
		expect(nav.mobile).toEqual(['sd1', 'sd2']);

		nav.enter('activity');
		nav.push('tx-detail');
		nav.sheetClosed('a2');
		nav.sheetClosed('a2');
		expect(nav.mobile).toEqual(['a1']);
	});

	it('the desktop stack drops the same step’s level, and only that', () => {
		const nav = new FlowNav();
		nav.enter('activity');
		nav.push('tx-detail');
		expect(nav.desktop).toEqual(['da1', 'da2']);
		nav.sheetClosed('a2');
		expect(nav.desktop).toEqual(['da1']);

		// The token sheet pushes no desktop level (the asset column is it).
		nav.enter('assets');
		nav.push('token-detail');
		expect(nav.desktop).toEqual(['dt1']);
		nav.sheetClosed('t2');
		expect(nav.desktop).toEqual(['dt1']);
	});
});
