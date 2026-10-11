/**
 * The hero's line under the figure (device walk of 2026-10-11, item 4).
 *
 * On the phones the refresh control sat alone on a line of its own — a lone ↻
 * under 「正在检查…」 — and left an empty line behind once the read had
 * settled. The web drew the same two lines the other way up: the control on
 * its own line under the figure, and the status line under that, empty
 * whenever the wallet had nothing to say, which is nearly always.
 *
 * They are ONE line now: what the wallet has to say on its leading side, the
 * control at its trailing edge, one height in every state. This renders the
 * real home — the phone's at 390 × 844 and the wide layout's — in each state
 * the line can be in, and holds the geometry: the hero's height, and the y of
 * the figure, the line, the control and the first row under the hero.
 */
import '$lib/tokens/tokens.css';
import { tick } from 'svelte';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { commands, page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import { loadCore } from '$lib/core/client';
import { words } from '$lib/settings/__fixtures__/words';
import { buildDesktopState, buildMobileState } from './fixtures';
import type { WalletMessages } from './messages';
import type { BalanceModel } from './model';
import WalletDesktop from './WalletDesktop.svelte';
import WalletHome from './WalletHome.svelte';

const m = words() as unknown as WalletMessages;
const identicon = (seed: string) => `<svg viewBox="0 0 8 8" data-seed="${seed}"></svg>`;

const refresh = (updated: string | undefined, spinning = false) => ({
	updated,
	updating: 'Updating…',
	a11yIdle: 'Refresh balance',
	spinning
});
const base = { label: 'Total balance', a11yHide: 'Hide balance', a11yShow: 'Show balance' };
const figure = { currency: 'USD', state: 'normal' as const, integer: '$1,383', decimals: '28' };

/** Every state the line can be in, as the live builder hands it to the hero. */
const STATES: Record<string, BalanceModel> = {
	// The first read is out: a skeleton, "Checking…", and no "updated" yet.
	checking: { ...base, state: 'loading', checkingText: 'Checking…', refresh: refresh(undefined) },
	// A read settled and the wallet has nothing to add.
	updated: { ...base, ...figure, refresh: refresh('Updated 2m') },
	// The person pressed the control: it turns and says so.
	updating: { ...base, ...figure, refresh: refresh('Updated 2m', true) },
	// A network did not answer: the figure may be short, and the line says why.
	failed: {
		...base,
		...figure,
		status: { kind: 'warning', text: "Can't reach BNB Chain right now" },
		refresh: refresh('Updated 2m')
	},
	// Nothing known and nothing reachable: the skeleton and the reason.
	offline: {
		...base,
		state: 'loading',
		status: { kind: 'warning', text: "Can't reach the network right now" },
		refresh: refresh(undefined)
	},
	hidden: {
		...base,
		currency: 'USD',
		state: 'hidden',
		integer: '••••••',
		refresh: refresh('Updated 2m')
	},
	// A new wallet, read and empty.
	live: {
		...base,
		currency: 'USD',
		state: 'zero-live',
		integer: '$0',
		decimals: '00',
		liveText: 'Live · listening for payments',
		refresh: refresh('Updated 2m')
	}
};

/** The same states with the longest words the corpus has for them (de). */
const LONG: Record<string, BalanceModel> = {
	failed: {
		...STATES.failed,
		status: { kind: 'warning', text: 'BNB Chain ist gerade nicht erreichbar' },
		refresh: { ...refresh('Aktualisiert vor 2 Min.'), updating: 'Wird aktualisiert …' }
	},
	updated: {
		...STATES.updated,
		refresh: { ...refresh('Aktualisiert vor 2 Min.'), updating: 'Wird aktualisiert …' }
	}
};

interface Geometry {
	/** The hero's height. */
	hero: number;
	/** The y of the figure's line, from the hero's top. */
	figure: number;
	/** The y of the line under it, and its height. */
	line: number;
	lineHeight: number;
	/** The y of the refresh control's box, and of its glyph. */
	control: number;
	glyph: number;
	/** How far the control's glyph ends from the line's trailing edge. */
	glyphFromLineEnd: number;
	/** How far the line ends from the hero's trailing edge. */
	lineFromHeroEnd: number;
	/** The y of the first row under the hero (the action row). */
	next: number;
	/** Whatever stands in the hero between the figure and the line. */
	between: number;
	/** What the leading side says, and whether it is cut. */
	said: string;
	cut: boolean;
	/** The control's words as drawn ('' = the glyph alone). */
	words: string;
}

const round = (n: number) => Math.round(n * 100) / 100;

function geometry(root: Element): Geometry {
	const hero = root.querySelector('.balance .label')!.parentElement!;
	const heroBox = hero.getBoundingClientRect();
	const box = (selector: string) => root.querySelector(selector)!.getBoundingClientRect();
	const figureBox = (
		root.querySelector('.amount-slot') ?? root.querySelector('.hidden-row')!
	).getBoundingClientRect();
	const line = box('.said');
	const control = root.querySelector<HTMLElement>('[data-testid="balance-refresh"]')!;
	// Where the glyph is LAID OUT: while it turns, its drawn box is the
	// rotated one, a fraction of a pixel wider than the square it turns in.
	const turning = control.querySelector<HTMLElement>('.glyph')!;
	turning.style.animation = 'none';
	const glyph = turning.getBoundingClientRect();
	const sentence = root.querySelector<HTMLElement>('.said .sentence, .said .live-words');
	const rows = [...hero.children].map((child) => child.getBoundingClientRect());
	return {
		hero: round(heroBox.height),
		figure: round(figureBox.top - heroBox.top),
		line: round(line.top - heroBox.top),
		lineHeight: round(line.height),
		control: round(control.getBoundingClientRect().top - heroBox.top),
		glyph: round(glyph.top - heroBox.top),
		glyphFromLineEnd: round(line.right - glyph.right),
		lineFromHeroEnd: round(heroBox.right - line.right),
		next: round(box('.actions').top - heroBox.top),
		// The hero's children are its label, its figure and its line: three.
		between: rows.length - 3,
		said: sentence?.textContent?.trim() ?? '',
		cut: sentence !== null && sentence.scrollWidth > sentence.clientWidth + 1,
		words: [...control.querySelectorAll<HTMLElement>('.words > span')]
			.filter((span) => getComputedStyle(span).visibility === 'visible')
			.map((span) => span.textContent)
			.join('')
	};
}

const GEOMETRY_OUT = import.meta.env.VITE_HERO_GEOMETRY as string | undefined;

const LAYOUTS = {
	phone: { width: 390, height: 844 },
	small: { width: 320, height: 640 },
	wide: { width: 1440, height: 900 }
} as const;
type Layout = keyof typeof LAYOUTS;

async function measured(
	layout: Layout,
	states: Record<string, BalanceModel>,
	textScale = 1
): Promise<Record<string, Geometry>> {
	const { width, height } = LAYOUTS[layout];
	await page.viewport(width, height);
	const out: Record<string, Geometry> = {};
	for (const [name, balance] of Object.entries(states)) {
		const screen =
			layout === 'wide'
				? render(WalletDesktop, {
						props: { model: { ...buildDesktopState('d1', m, identicon), balance } }
					})
				: render(WalletHome, {
						props: { model: { ...buildMobileState('h1', m, identicon), balance, textScale } }
					});
		await tick();
		// The status sentence is measured and set a frame after it lands.
		await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
		out[name] = geometry(screen.container);
		screen.unmount();
	}
	return out;
}

/** The parts of a geometry that must be the same in every state of a layout. */
const still = ({ hero, figure, line, lineHeight, control, glyph, next }: Geometry) => ({
	hero,
	figure,
	line,
	lineHeight,
	control,
	glyph,
	next
});

beforeAll(() => loadCore());
afterAll(() => page.viewport(414, 896));

describe('the hero’s line: what the wallet says, and the control that reads again', () => {
	const CASES: [string, Layout, Record<string, BalanceModel>, number][] = [
		['phone', 'phone', STATES, 1],
		['small', 'small', STATES, 1],
		['wide', 'wide', STATES, 1],
		['phone, long words', 'phone', LONG, 1],
		['small, long words', 'small', LONG, 1],
		['phone, large text', 'phone', STATES, 1.35]
	];
	const table: Record<string, Record<string, Geometry>> = {};

	beforeAll(async () => {
		for (const [name, layout, states, scale] of CASES) {
			table[name] = await measured(layout, states, scale);
		}
		// Written out when asked (`VITE_HERO_GEOMETRY=<file>`), for the
		// before/after table a "nothing jumps" change reports.
		if (GEOMETRY_OUT) await commands.writeFile(GEOMETRY_OUT, JSON.stringify(table, null, 1));
	});

	it.each(CASES.map(([name]) => name))(
		'%s: one height, and every y the same, whatever the line says',
		(name) => {
			const states = Object.entries(table[name]);
			const [, first] = states[0];
			for (const [state, at] of states) {
				expect(still(at), `${name} · ${state}`).toEqual(still(first));
			}
		}
	);

	it.each(CASES.map(([name]) => name))(
		'%s: the control is ON the line, at its trailing edge, in every state',
		(name) => {
			for (const [state, at] of Object.entries(table[name])) {
				const where = `${name} · ${state}`;
				// Its box is the line's own height, where the line is.
				expect(at.control, where).toBeCloseTo(at.line, 1);
				// Its glyph is the last thing on the line.
				expect(at.glyphFromLineEnd, where).toBeCloseTo(0, 1);
			}
		}
	);

	it.each(CASES.map(([name]) => name))(
		'%s: no line of its own for the control, and no empty line left behind',
		(name) => {
			for (const [state, at] of Object.entries(table[name])) {
				const where = `${name} · ${state}`;
				// The hero is its label, its figure and ONE line under it…
				expect(at.between, where).toBe(0);
				// …and that line is the last thing in it.
				expect(at.line + at.lineHeight, where).toBeCloseTo(at.hero, 1);
			}
		}
	);

	it('on a phone the line ends where the hero does; on the wide layout, where the rows do', () => {
		for (const name of ['phone', 'small']) {
			for (const at of Object.values(table[name])) expect(at.lineFromHeroEnd).toBeCloseTo(0, 1);
		}
		const wide = Object.values(table.wide);
		// The hero keeps the column (the figure may need it); its line keeps
		// the rows' measure, so the control stands over their trailing actions.
		expect(wide[0].lineFromHeroEnd).toBeGreaterThan(100);
		for (const at of wide) expect(at.lineFromHeroEnd).toBe(wide[0].lineFromHeroEnd);
	});

	it('says one thing at a time on a phone, and both on the wide layout', () => {
		const phone = table.phone;
		// Nothing to say: when the figure was read.
		expect(phone.updated).toMatchObject({ said: '', words: 'Updated 2m' });
		expect(phone.updating).toMatchObject({ said: '', words: 'Updating…' });
		expect(phone.hidden).toMatchObject({ said: '', words: 'Updated 2m' });
		// Something to say: said whole, the control its glyph.
		expect(phone.checking).toMatchObject({ said: 'Checking…', words: '', cut: false });
		expect(phone.failed).toMatchObject({
			said: "Can't reach BNB Chain right now",
			words: '',
			cut: false
		});
		expect(phone.offline).toMatchObject({ words: '', cut: false });
		expect(phone.live).toMatchObject({
			said: 'Live · listening for payments',
			words: '',
			cut: false
		});
		// The wide layout has the room.
		expect(table.wide.failed).toMatchObject({
			said: "Can't reach BNB Chain right now",
			words: 'Updated 2m',
			cut: false
		});
		expect(table.wide.live).toMatchObject({ words: 'Updated 2m', cut: false });
	});

	it('the corpus’s long words: the sentence is whole at 390 and the line is still one line at 320', () => {
		expect(table['phone, long words'].failed).toMatchObject({
			said: 'BNB Chain ist gerade nicht erreichbar',
			cut: false
		});
		expect(table['phone, long words'].updated.words).toBe('Aktualisiert vor 2 Min.');
		// At 320 the same sentence may be set smaller or end in "…"; what it
		// may not do is change the line (held above).
		expect(table['small, long words'].failed.said).toBe('BNB Chain ist gerade nicht erreichbar');
	});
});
