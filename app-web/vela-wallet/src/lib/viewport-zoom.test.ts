/**
 * r14-1: on iOS a page must never zoom in when a field takes focus.
 *
 * The Safari Web Extension's wallet tab was found laid out 429 pt wide in a
 * 402 pt viewport, its right edge (chevrons, the Scan card, the Settings tab)
 * cut off. Nothing on the page was wider than the screen: the numbers are a
 * ZOOM — 429 / 402 and 934 / 874 are both 16 / 15. iOS zooms a page in when a
 * field whose text is under 16 px takes focus (ours is `--text-lg`, 15 px),
 * so that its text reads at 16, and it never zooms back out. Every screen
 * after the first field typed into stayed 6.7 % too big.
 *
 * `app.html` caps the scale at 1 before the first paint — on iOS and iPadOS
 * alone: Safari there keeps pinch-zoom whatever the page says (since iOS 10),
 * so the cap takes away the focus zoom and nothing else; on Android the same
 * cap would take pinch-zoom away from the person who needs it.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const INITIAL = 'width=device-width, initial-scale=1';
const UA = {
	iphone:
		'Mozilla/5.0 (iPhone; CPU iPhone OS 26_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.2 Mobile/15E148 Safari/604.1',
	ipad: 'Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
	// iPadOS asks for the desktop site by default: it says "Macintosh", and
	// only its touch points tell it from a Mac.
	ipadDesktop:
		'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15',
	android:
		'Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36',
	mac: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15'
};

/** The block in `app.html`, run against a page with this browser's navigator. */
function viewportAfter(navigator: {
	userAgent: string;
	platform: string;
	maxTouchPoints: number;
}): string {
	const html = readFileSync(new URL('../app.html', import.meta.url), 'utf8');
	const start = html.indexOf('/* ios-zoom:begin */');
	const end = html.indexOf('/* ios-zoom:end */');
	expect(start).toBeGreaterThan(-1);
	expect(end).toBeGreaterThan(start);
	let content = INITIAL;
	const meta = {
		getAttribute: (name: string) => (name === 'content' ? content : null),
		setAttribute: (name: string, value: string) => {
			if (name === 'content') content = value;
		}
	};
	const document = {
		querySelector: (selector: string) => (selector === 'meta[name="viewport"]' ? meta : null)
	};
	new Function('navigator', 'document', html.slice(start, end))(navigator, document);
	return content;
}

describe('the viewport on iOS (r14-1)', () => {
	it('caps the scale at 1 on an iPhone and an iPad, so a focused field never zooms the page', () => {
		const capped = `${INITIAL}, maximum-scale=1`;
		expect(viewportAfter({ userAgent: UA.iphone, platform: 'iPhone', maxTouchPoints: 5 })).toBe(
			capped
		);
		expect(viewportAfter({ userAgent: UA.ipad, platform: 'iPad', maxTouchPoints: 5 })).toBe(capped);
		expect(
			viewportAfter({ userAgent: UA.ipadDesktop, platform: 'MacIntel', maxTouchPoints: 5 })
		).toBe(capped);
	});

	it('leaves every other browser its pinch-zoom', () => {
		expect(
			viewportAfter({ userAgent: UA.android, platform: 'Linux armv81', maxTouchPoints: 5 })
		).toBe(INITIAL);
		expect(viewportAfter({ userAgent: UA.mac, platform: 'MacIntel', maxTouchPoints: 0 })).toBe(
			INITIAL
		);
	});

	it('the page as served still says the plain viewport (the cap is added on iOS only)', () => {
		const html = readFileSync(new URL('../app.html', import.meta.url), 'utf8');
		expect(html).toContain(`<meta name="viewport" content="${INITIAL}" />`);
	});
});
