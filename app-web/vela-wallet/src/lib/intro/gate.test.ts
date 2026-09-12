import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { FORCE_PARAM, SKIP_PARAM, STORAGE_KEY, markIntroSeen, shouldShowIntro } from './gate';

/** A `Window` stand-in with just the two surfaces the gate touches. */
function fakeWindow(search = '', stored: string | null = null) {
	let value = stored;
	return {
		location: { search },
		localStorage: {
			getItem: () => value,
			setItem: (_: string, next: string) => {
				value = next;
			}
		},
		read: () => value
	} as unknown as Window & { read: () => string | null };
}

/** Storage that throws, as it does in private modes and sandboxed frames. */
function hostileWindow() {
	return {
		location: { search: '' },
		localStorage: {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('denied');
			}
		}
	} as unknown as Window;
}

describe('intro gate', () => {
	it('shows on a browser that has never seen it', () => {
		expect(shouldShowIntro(fakeWindow())).toBe(true);
	});

	it('never shows again once marked — this is a first-run screen, not a weekly one', () => {
		const win = fakeWindow();
		markIntroSeen(win, 1_700_000_000_000);
		expect(shouldShowIntro(win)).toBe(false);
		// A year later it is still seen: unlike the launch animation, there is no
		// replay window.
		expect(shouldShowIntro(fakeWindow('', win.read()))).toBe(false);
	});

	it('records the timestamp, so a later policy change has something to read', () => {
		const win = fakeWindow();
		markIntroSeen(win, 42);
		expect(win.read()).toBe('42');
		expect(STORAGE_KEY).toBe('vela.intro.seen');
	});

	it(`?${SKIP_PARAM} suppresses it for a deterministic e2e run`, () => {
		expect(shouldShowIntro(fakeWindow(`?${SKIP_PARAM}`))).toBe(false);
	});

	it(`?${FORCE_PARAM} shows it even once seen, and wins over the skip`, () => {
		expect(shouldShowIntro(fakeWindow(`?${FORCE_PARAM}`, '1'))).toBe(true);
		expect(shouldShowIntro(fakeWindow(`?${FORCE_PARAM}&${SKIP_PARAM}`, '1'))).toBe(true);
	});

	it('shows it when storage cannot be read at all', () => {
		// Showing twice is cosmetic; a front door that throws is not.
		expect(shouldShowIntro(hostileWindow())).toBe(true);
		expect(() => markIntroSeen(hostileWindow())).not.toThrow();
	});
});

/**
 * The pre-paint copy of the rule (spec 038). `app.html` decides before any
 * module loads whether Welcome is hidden under the intro, and cannot import
 * this module to do it — so the constants are written there by hand, and this
 * is what keeps the two from drifting: a renamed key here would otherwise hide
 * Welcome for an intro the component then declines to show.
 */
describe('the pre-paint copy in app.html', () => {
	const html = readFileSync(join(import.meta.dirname, '..', '..', 'app.html'), 'utf8');
	const block = html.slice(html.indexOf('Spec 038'), html.indexOf('</script>'));

	it('exists, and reads the same storage key', () => {
		expect(block.length).toBeGreaterThan(0);
		expect(block).toContain(`localStorage.getItem('${STORAGE_KEY}')`);
	});

	it('honours both query params, force before skip', () => {
		expect(block).toContain(`introParams.has('${FORCE_PARAM}')`);
		expect(block).toContain(`introParams.has('${SKIP_PARAM}')`);
		expect(block.indexOf(`has('${FORCE_PARAM}')`)).toBeLessThan(
			block.indexOf(`has('${SKIP_PARAM}')`)
		);
	});

	it('shows the intro when storage throws, like the module', () => {
		// The catch branch must still set the attribute — a blocked storage
		// shows the intro twice at worst; a blank first run is not acceptable.
		const catchBranch = block.slice(block.indexOf('catch (e)'));
		expect(catchBranch).toContain("dataset.intro = 'pending'");
	});

	it('only ever sets the attribute the CSS hides on', () => {
		const css = readFileSync(join(import.meta.dirname, '..', '..', 'app.css'), 'utf8');
		expect(css).toContain("html[data-intro='pending'] [data-intro-page]");
	});
});
