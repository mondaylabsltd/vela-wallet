/**
 * Spec 015 gates: every wallet key resolves in all 15 locales (US4), and the
 * fixture canon carries the mock content verbatim (FR-012).
 */
import { describe, expect, it } from 'vitest';
import { rawResolve, resolveWalletMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { buildDesktopState, buildMobileState, MOBILE_STATES } from './fixtures';
import { WALLET_KEYS, fill } from './messages';

const IDENTICON_STUB = () => '<svg></svg>';

describe('wallet messages', () => {
	it.each(SUPPORTED_LOCALES)('every wallet key resolves in %s', (locale) => {
		for (const key of WALLET_KEYS) {
			const value = rawResolve(locale, key);
			expect(value, `${key} in ${locale}`).not.toBe(key);
			expect(value.trim()).not.toBe('');
		}
	});

	it('fill interpolates {{var}} templates', () => {
		expect(fill('至 {{name}}', { name: 'hold on' })).toBe('至 hold on');
		expect(fill('all {{count}} networks', { count: 8 })).toBe('all 8 networks');
		expect(fill('no {{other}} match', { name: 'x' })).toBe('no {{other}} match');
	});
});

describe('fixture canon (zh mock verbatim)', () => {
	const zh = resolveWalletMessages('zh');

	it('h1 carries the mock header, balance and rows', () => {
		const m = buildMobileState('h1', zh, IDENTICON_STUB);
		expect(m.header.name).toBe('大表哥');
		expect(m.header.addressDisplay).toBe('0x14fB1f…D1eA5c');
		expect(m.balance.integer).toBe('$1,383');
		expect(m.balance.decimals).toBe('28');
		expect(m.pill.label).toBe('全部网络');
		const rows = m.activityGroups.flatMap((g) => g.rows);
		expect(rows[0]).toMatchObject({
			title: '已发送',
			subtitle: '至 hold on',
			amount: '−2',
			unit: 'POL'
		});
		expect(rows[1]).toMatchObject({ amount: '+120', unit: 'USDT', positive: true });
		expect(m.assetRows[0]).toMatchObject({ ticker: 'BNB', chain: 'BNB Chain', balance: '0.8533' });
	});

	it('h4 marks CAKE as unpriced with the mock copy', () => {
		const m = buildMobileState('h4', zh, IDENTICON_STUB);
		expect(m.balance.status).toMatchObject({ kind: 'warning', text: '部分代币无法获取价格。' });
		const cake = m.assetRows.at(-1);
		expect(cake?.ticker).toBe('CAKE');
		expect(cake?.fiat).toMatchObject({ kind: 'no-price', text: '无价格' });
	});

	it('h5 masks amounts but keeps units', () => {
		const m = buildMobileState('h5', zh, IDENTICON_STUB);
		expect(m.balance.state).toBe('hidden');
		const rows = m.activityGroups.flatMap((g) => g.rows);
		expect(rows.every((r) => r.masked && r.amount === '••••')).toBe(true);
		expect(rows[0].unit).toBe('POL');
		expect(m.assetRows.every((r) => r.masked)).toBe(true);
	});

	it('h3 has never been read: the refresh is the glyph alone, as live draws it (issue 462)', () => {
		const loading = buildMobileState('h3', zh, IDENTICON_STUB).balance;
		expect(loading.state).toBe('loading');
		expect(loading.refresh).toMatchObject({ updated: undefined, spinning: false });
		expect(loading.refresh?.a11yIdle).toBe(zh.balance.refreshBalance);
		// Every board that has a figure says when it was read.
		expect(buildMobileState('h1', zh, IDENTICON_STUB).balance.refresh?.updated).toBe(
			fill(zh.balance.lastUpdated, { ago: fill(zh.balance.ago.minutes, { n: 2 }) })
		);
	});

	it('h7 uses the extreme fixtures and single-chain pill', () => {
		const m = buildMobileState('h7', zh, IDENTICON_STUB);
		expect(m.header.name).toBe('这是一个非常长');
		expect(m.pill).toMatchObject({ kind: 'single', label: 'BNB Chain' });
		expect(m.balance.integer).toBe('$1,234,567');
		const rows = m.activityGroups.flatMap((g) => g.rows);
		expect(rows[0].amount).toBe('−1234.5678');
		expect(rows[1].amount).toBe('−0.0000001');
		expect(m.assetRows[1].balance).toBe('1,234,567.8901');
		expect(m.textScale).toBe(1);
		expect(buildMobileState('h7x', zh, IDENTICON_STUB).textScale).toBe(1.35);
	});

	it('h8 lists 所有网络 ✓ 8 then six chains', () => {
		const m = buildMobileState('h8', zh, IDENTICON_STUB);
		expect(m.sheet?.title).toBe('选择链');
		expect(m.sheet?.rows[0]).toMatchObject({ name: '所有网络', count: 8, selected: true });
		expect(m.sheet?.rows).toHaveLength(7);
	});

	it('every mobile state builds', () => {
		for (const state of MOBILE_STATES) {
			expect(buildMobileState(state, zh, IDENTICON_STUB).state).toBe(state);
		}
	});

	it('desktop panels carry the D2/D3 content', () => {
		const d2 = buildDesktopState('d2', zh, IDENTICON_STUB);
		expect(d2.initialPanel).toBe('receive');
		expect(d2.panels.receive.token.detail).toBe('BNB Chain · 链 ID 56');
		expect(d2.panels.receive.networksLine).toBe('同一地址，通用于全部 8 个网络');
		expect(d2.panels.receive.qrCaption).toBe('演示占位图案 · 不可扫描');

		const d3 = buildDesktopState('d3', zh, IDENTICON_STUB);
		expect(d3.initialPanel).toBe('asset-detail');
		expect(d3.panels.assetDetail.facts.map((f) => `${f.label}=${f.value}`)).toEqual([
			'名称=BNB',
			'价格=1 BNB = $581.85',
			'合约=原生代币',
			'精度=18'
		]);
		expect(d3.panels.assetDetail.rows[1].subtitle).toBe('来自 0x21aE…9F3c · 8月1日');

		const d1 = buildDesktopState('d1', zh, IDENTICON_STUB);
		expect(d1.initialPanel).toBe('none');
		expect(d1.activityGroups.flatMap((g) => g.rows)[0].subtitle).toBe('至 hold on · 今天 14:02');
		expect(d1.sidebar.nav.find((n) => n.selected)?.id).toBe('wallet');
	});
});

describe('the wide layout carries the narrow layout’s empty copy (spec 082 RB13, G1)', () => {
	it('activity and assets, the same words the home uses', () => {
		const en = resolveWalletMessages('en');
		const wide = buildDesktopState('d1', en, () => '<svg/>');
		const narrow = buildMobileState('h1', en, () => '<svg/>');
		expect(wide.activitySection.empty).toEqual(narrow.activitySection.empty);
		expect(wide.assetsSection.empty).toEqual(narrow.assetsSection.empty);
		expect(wide.activitySection.empty?.title).toBe(en.activity.emptyTitle);
	});
});
