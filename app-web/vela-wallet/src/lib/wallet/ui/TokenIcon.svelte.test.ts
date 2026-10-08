/**
 * The token mark's logo fallback (2026-09-05): a logo from the chain-data
 * endpoint draws OVER the glyph, and a logo that does not load leaves the
 * glyph exactly as drawn — the property the founder asked for ("read from
 * the index endpoint; if it cannot, use today's default").
 *
 * A `.svelte.test.ts`: the browser project, because `<img onerror>` is the
 * browser's to fire.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
// The global base: every box is border-box, so a ring sits inside its size.
import '../../../app.css';
import { hasFailed, resetLogoCacheForTests } from '$lib/services/logo-cache';
import TokenIcon from './TokenIcon.svelte';

/** A 1×1 PNG — a logo that loads. */
const LOADS =
	'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';
/** Not an image at all — a logo that fails, the way a 404 does. */
const FAILS = 'data:image/png;base64,bm90LWFuLWltYWdl';

beforeEach(() => resetLogoCacheForTests());

describe('TokenIcon', () => {
	it('draws the glyph, and only the glyph, when it has no logo', () => {
		const screen = render(TokenIcon, { props: { ticker: 'usdt', badgeColor: '#000' } });
		expect(screen.container.textContent).toContain('USD');
		expect(screen.container.querySelector('img')).toBeNull();
	});

	it('shows the first candidate that loads, past the ones that do not', async () => {
		const screen = render(TokenIcon, {
			props: { ticker: 'eth', badgeColor: '#000', logoUrls: [FAILS, LOADS] }
		});
		await vi.waitFor(() => {
			expect(screen.container.querySelector('img')?.getAttribute('src')).toBe(LOADS);
		});
		// The failure is remembered for every other mark on the page.
		expect(hasFailed(FAILS)).toBe(true);
		// The glyph is still underneath — the circle was never blank.
		expect(screen.container.textContent).toContain('ETH');
	});

	it('falls back to the glyph when no candidate loads', async () => {
		const screen = render(TokenIcon, {
			props: { ticker: 'pusd', badgeColor: '#000', logoUrls: [FAILS] }
		});
		await vi.waitFor(() => {
			expect(screen.container.querySelector('img')).toBeNull();
		});
		expect(screen.container.textContent).toContain('PUS');
	});

	it("wears the badge chain's logo over the dot, and no badge when it would repeat the token", async () => {
		const badged = render(TokenIcon, {
			props: { ticker: 'eth', badgeColor: '#000', logoUrls: [LOADS], badgeLogoUrl: LOADS }
		});
		await vi.waitFor(() => {
			expect(badged.container.querySelectorAll('img')).toHaveLength(2);
		});
		expect(badged.container.querySelector('.badge')).not.toBeNull();

		const own = render(TokenIcon, {
			props: { ticker: 'eth', badgeColor: '#000', logoUrls: [LOADS], badgeHidden: true }
		});
		expect(own.container.querySelector('.badge')).toBeNull();
	});

	// The badge ruling every shell draws to (2026-10-08): a chain badge that
	// carries a logo is 16 across, its 1.5 ring inside that — the phones moved
	// up from 12 to match. A smaller badge was a logo nobody could read.
	it('draws a logo badge 16 across in a 1.5 ring, on every shell the same', () => {
		const screen = render(TokenIcon, {
			props: { ticker: 'usdc', badgeColor: '#000', logoUrls: [LOADS], badgeLogoUrl: LOADS }
		});
		const badge = screen.container.querySelector<HTMLElement>('.badge.with-logo')!;
		expect(badge.getBoundingClientRect().width).toBe(16);
		expect(badge.getBoundingClientRect().height).toBe(16);
		// The ring is the emphasis border; at 1x the browser snaps 1.5 to a
		// whole pixel, so the token is what is pinned, and that it is drawn.
		const ring = getComputedStyle(document.documentElement).getPropertyValue('--border-emphasis');
		expect(ring.trim()).toBe('1.5px');
		expect(parseFloat(getComputedStyle(badge).borderTopWidth)).toBeGreaterThan(0);
	});
});
