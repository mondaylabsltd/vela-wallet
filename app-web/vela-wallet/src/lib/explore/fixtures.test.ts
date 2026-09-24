/**
 * Spec 022 gates for the explore layer: every key resolves in all 15 locales,
 * the state inventory matches data-model.md, and each state's shape is the one
 * the mock draws (which is the thing a refactor silently breaks).
 */
import { describe, expect, it } from 'vitest';
import { rawResolve, resolveExploreMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import {
	buildDesktopState,
	buildMobileState,
	DESKTOP_STATES,
	MOBILE_STATES,
	SITES
} from './fixtures';

const IDENTICON_STUB = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const messages = resolveExploreMessages('zh');

/** Every corpus key this layer names, derived from the resolver's own output. */
const EXPLORE_KEYS = Object.keys(resolveExploreMessages('en')).filter(
	(k) => k !== 'nav' && k !== 'closeLabel'
);

describe('explore messages', () => {
	it.each(SUPPORTED_LOCALES)('every explore key resolves in %s', (locale) => {
		for (const key of EXPLORE_KEYS) {
			const value = rawResolve(locale, `explore.${key}`);
			expect(value, `explore.${key} in ${locale}`).not.toBe(`explore.${key}`);
			expect(value.trim()).not.toBe('');
		}
	});
});

describe('state inventory (data-model.md §2)', () => {
	it('is the eight phone states and the four desktop ones', () => {
		expect(MOBILE_STATES).toEqual(['e1', 'e2', 'e3', 'e4', 'e5', 'e6', 'e7', 'e8']);
		expect(DESKTOP_STATES).toEqual(['de1', 'de2', 'de3', 'de4']);
	});

	it.each(MOBILE_STATES)('%s builds with its own id and no unfilled template', (state) => {
		const model = buildMobileState(state, messages, IDENTICON_STUB);
		expect(model.state).toBe(state);
		expect(JSON.stringify(model)).not.toContain('{{');
	});

	it.each(DESKTOP_STATES)('%s builds with its own id', (state) => {
		const model = buildDesktopState(state, messages, IDENTICON_STUB);
		expect(model.state).toBe(state);
		expect(JSON.stringify(model)).not.toContain('{{');
	});
});

describe('what each state is FOR', () => {
	it('E1 is the empty start page — no favourites, no groups', () => {
		const e1 = buildMobileState('e1', messages, IDENTICON_STUB);
		expect(e1.empty).toBeDefined();
		expect(e1.favorites).toBeUndefined();
		expect(e1.groups).toHaveLength(0);
		expect(e1.tabCountLabel).toBeUndefined();
	});

	it('E2 carries the eight favourites plus the add tile, and three groups', () => {
		const e2 = buildMobileState('e2', messages, IDENTICON_STUB);
		expect(e2.empty).toBeUndefined();
		expect(e2.favorites?.tiles).toHaveLength(8);
		expect(e2.favorites?.tiles.at(-1)?.kind).toBe('add');
		expect(e2.groups.map((g) => g.id)).toEqual(['recent', 'trading', 'prediction']);
	});

	it('E3/E6/E7/E8 open on a sheet; E1/E2/E4/E5 do not', () => {
		expect(buildMobileState('e3', messages, IDENTICON_STUB).sheet?.kind).toBe('group-manage');
		expect(buildMobileState('e6', messages, IDENTICON_STUB).sheet?.kind).toBe('site-menu');
		expect(buildMobileState('e7', messages, IDENTICON_STUB).sheet?.kind).toBe('connection');
		// E8 is the history row's menu: the site-menu sheet with its own three items.
		const e8 = buildMobileState('e8', messages, IDENTICON_STUB).sheet;
		expect(e8?.kind).toBe('site-menu');
		expect(e8?.kind === 'site-menu' && e8.items.map((i) => i.id)).toEqual([
			'new-tab',
			'favorite',
			'delete'
		]);
		for (const state of ['e1', 'e2', 'e4', 'e5'] as const) {
			expect(buildMobileState(state, messages, IDENTICON_STUB).sheet).toBeUndefined();
		}
	});

	it('the browsing states show a page; E5 shows the switcher', () => {
		for (const state of ['e4', 'e6', 'e7'] as const) {
			expect(buildMobileState(state, messages, IDENTICON_STUB).view).toBe('browsing');
		}
		expect(buildMobileState('e5', messages, IDENTICON_STUB).view).toBe('tabs');
		expect(buildMobileState('e2', messages, IDENTICON_STUB).view).toBe('start');
	});

	it('E5 selects the tab it was opened from, not the start page (mock E5)', () => {
		const tabs = buildMobileState('e5', messages, IDENTICON_STUB).tabs;
		expect(tabs.find((t) => t.selected)?.id).toBe('uniswap');
	});

	it('every state can raise the three sheets without inventing copy', () => {
		for (const state of MOBILE_STATES) {
			const { menus } = buildMobileState(state, messages, IDENTICON_STUB);
			expect(menus.siteMenu.items).toHaveLength(7);
			expect(menus.groupManage.rows).toHaveLength(4);
			expect(menus.connection.connection.explainer.length).toBeGreaterThan(0);
		}
	});

	it('the group manager can hide a system group but never delete one', () => {
		const rows = buildMobileState('e3', messages, IDENTICON_STUB).menus.groupManage.rows;
		const system = rows.filter((r) => r.system).map((r) => r.id);
		expect(system).toEqual(['favorites', 'recent']);
	});

	it('DE1 is empty, DE2 carries the tile context menu, DE3 opens the connection', () => {
		expect(buildDesktopState('de1', messages, IDENTICON_STUB).start.empty).toBeDefined();
		expect(buildDesktopState('de2', messages, IDENTICON_STUB).contextMenu?.items).toHaveLength(4);
		expect(buildDesktopState('de3', messages, IDENTICON_STUB).initialPanel).toBe('connection');
		expect(buildDesktopState('de4', messages, IDENTICON_STUB).initialPanel).toBe('signing');
	});
});

describe('fixture content is the mock, verbatim (FR-012)', () => {
	it('site names, hosts and letters', () => {
		expect(SITES.uniswap).toMatchObject({ name: 'Uniswap', host: 'app.uniswap.org', letter: 'U' });
		expect(SITES.hyperliquid.host).toBe('app.hyperliquid.xyz');
		expect(SITES.limitless.name).toBe('Limitless');
	});

	it('the custom groups keep the names a person typed, untranslated', () => {
		const groups = buildMobileState('e2', messages, IDENTICON_STUB).groups;
		expect(groups.map((g) => g.title).slice(1)).toEqual(['交易', '预测市场']);
	});

	it('the stand-in page is the site’s content, not our chrome', () => {
		const page = buildMobileState('e4', messages, IDENTICON_STUB).browser.page;
		expect(page.title).toBe('兑换');
		expect(page.fields.map((f) => f.symbol)).toEqual(['ETH', 'USDC']);
		expect(page.ctaTint).toBe(SITES.uniswap.tint);
	});
});
