/**
 * Spec 021 gates: every wallet-flow key resolves in all 15 locales (SC-005),
 * every state in the matrix builds (SC-001), and the canon carries the mock
 * content verbatim.
 */
import { describe, expect, it } from 'vitest';
import { rawResolve, resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { WALLET_FLOW_KEYS } from './messages';
import {
	addressLines,
	buildDesktopFlowState,
	buildDesktopScan,
	buildFlowState,
	DESKTOP_FLOW_STATES,
	MOBILE_FLOW_STATES,
	NETWORKS,
	USDT_CONTRACT
} from './fixtures';

const IDENTICON_STUB = (seed: string) => `<svg data-seed="${seed}"></svg>`;

describe('wallet-flow messages', () => {
	it.each(SUPPORTED_LOCALES)('every flow key resolves in %s', (locale) => {
		for (const key of WALLET_FLOW_KEYS) {
			const value = rawResolve(locale, key);
			expect(value, `${key} in ${locale}`).not.toBe(key);
			expect(value.trim()).not.toBe('');
		}
	});

	it('names every key exactly once', () => {
		expect(new Set(WALLET_FLOW_KEYS).size).toBe(WALLET_FLOW_KEYS.length);
	});
});

describe('state matrix', () => {
	const zh = resolveWalletFlowMessages('zh');

	it('builds all 30 mobile states', () => {
		expect(MOBILE_FLOW_STATES).toHaveLength(30);
		for (const id of MOBILE_FLOW_STATES) {
			const model = buildFlowState(id, zh, IDENTICON_STUB);
			expect(model.state, id).toBe(id);
			expect(model.base, id).toBeDefined();
		}
	});

	it('builds all 19 desktop states', () => {
		expect(DESKTOP_FLOW_STATES).toHaveLength(19);
		for (const id of DESKTOP_FLOW_STATES) {
			const model = buildDesktopFlowState(id, zh, IDENTICON_STUB);
			expect(model.state, id).toBe(id);
			expect(model.title.trim(), id).not.toBe('');
			expect(model.body, id).toBeDefined();
		}
	});

	it('leaves no unresolved {{var}} in any built state', () => {
		// Exactly one template is deliberately left unfilled: the receive
		// list's no-match line takes the live search query, which a fixture
		// cannot know, so `ReceiveList` fills it at render. Anything else —
		// a stray `{{count}}`, a `{{symbol}}` — would ship to a user.
		const check = (json: string, id: string) => {
			const templates = json.match(/\{\{\s*\w+\s*\}\}/g) ?? [];
			for (const template of templates) {
				expect(template, `${id} carries an unfilled template`).toBe('{{query}}');
			}
			expect(templates.length, id).toBeLessThanOrEqual(1);
		};

		for (const id of MOBILE_FLOW_STATES) {
			check(JSON.stringify(buildFlowState(id, zh, IDENTICON_STUB)), id);
		}
		for (const id of DESKTOP_FLOW_STATES) {
			check(JSON.stringify(buildDesktopFlowState(id, zh, IDENTICON_STUB)), id);
		}
	});
});

describe('fixture canon (zh mock verbatim)', () => {
	const zh = resolveWalletFlowMessages('zh');

	it('r1 lists the eight supported networks with one shared address', () => {
		const m = buildFlowState('r1', zh, IDENTICON_STUB);
		if (m.base.kind !== 'receive-list') throw new Error('expected the receive list');
		expect(m.base.model.subtitle).toBe('同一地址，通用于全部 8 个网络');
		expect(m.base.model.rows).toHaveLength(8);
		expect(m.base.model.rows.map((r) => r.name)).toEqual([
			'Ethereum',
			'BNB Chain',
			'Polygon',
			'Arbitrum',
			'Optimism',
			'Base',
			'Avalanche',
			'Gnosis'
		]);
		// The point of the screen: every row is the SAME address.
		expect(new Set(m.base.model.rows.map((r) => r.addressDisplay)).size).toBe(1);
	});

	it('r2 titles the network and r3 the asset, and only r3 has a contract', () => {
		const r2 = buildFlowState('r2', zh, IDENTICON_STUB);
		const r3 = buildFlowState('r3', zh, IDENTICON_STUB);
		if (r2.sheet?.kind !== 'receive-qr' || r3.sheet?.kind !== 'receive-qr') {
			throw new Error('expected both QR sheets');
		}
		expect(r2.sheet.model.title).toBe('使用这个地址接收 Ethereum 上的资产');
		expect(r2.sheet.model.contract).toBeUndefined();
		expect(r3.sheet.model.title).toBe('使用这个地址接收 Ethereum 上的 USDT');
		expect(r3.sheet.model.contract?.label).toBe('代币合约');
	});

	it('r2x is the only state at 1.35x text scale', () => {
		for (const id of MOBILE_FLOW_STATES) {
			const expected = id === 'r2x' ? 1.35 : 1;
			expect(buildFlowState(id, zh, IDENTICON_STUB).textScale, id).toBe(expected);
		}
	});

	it('splits the account address into two even lines', () => {
		expect(addressLines('0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c')).toEqual([
			'0x14fB1fB21751E29F7Ec',
			'48dC450017552E3D1eA5c'
		]);
	});

	it('a1 groups the history by day, ending on a literal date', () => {
		const m = buildFlowState('a1', zh, IDENTICON_STUB);
		if (m.base.kind !== 'history') throw new Error('expected the history');
		expect(m.base.model.groups.map((g) => g.label)).toEqual(['今天', '昨天', '8/12']);
		expect(m.base.model.groups[0].rows[0]).toMatchObject({
			title: '已发送',
			amount: '−2',
			unit: 'POL',
			positive: false
		});
	});

	it('a2 carries a contract row and a3 does not', () => {
		const a2 = buildFlowState('a2', zh, IDENTICON_STUB);
		const a3 = buildFlowState('a3', zh, IDENTICON_STUB);
		if (a2.sheet?.kind !== 'tx-detail' || a3.sheet?.kind !== 'tx-detail') {
			throw new Error('expected both detail sheets');
		}
		expect(a2.sheet.model.amount).toBe('+120 USDT');
		expect(a2.sheet.model.positive).toBe(true);
		expect(a2.sheet.model.facts.some((f) => f.label === '代币合约')).toBe(true);

		expect(a3.sheet.model.amount).toBe('−2 POL');
		expect(a3.sheet.model.positive).toBe(false);
		// A native coin has no contract — the row must be absent, not empty.
		expect(a3.sheet.model.facts.some((f) => f.label === '代币合约')).toBe(false);
	});

	it('t1 lists the six mock assets and t4 replaces them with guidance', () => {
		const t1 = buildFlowState('t1', zh, IDENTICON_STUB);
		const t4 = buildFlowState('t4', zh, IDENTICON_STUB);
		if (t1.base.kind !== 'assets' || t4.base.kind !== 'assets') throw new Error('expected assets');
		expect(t1.base.model.rows).toHaveLength(6);
		expect(t1.base.model.rows[0]).toMatchObject({ ticker: 'BNB', balance: '0.8533' });
		expect(t1.base.model.empty).toBeUndefined();

		expect(t4.base.model.rows).toHaveLength(0);
		expect(t4.base.model.empty?.title).toBe('存入您的第一笔资产');
		expect(t4.base.model.empty?.hintTitle).toBe('已经收到代币但没有显示？');
	});

	it('t3 offers the real USDT contract and t5 rejects a truncated one', () => {
		const t3 = buildFlowState('t3', zh, IDENTICON_STUB);
		const t5 = buildFlowState('t5', zh, IDENTICON_STUB);
		if (t3.sheet?.kind !== 'add-token' || t5.sheet?.kind !== 'add-token') {
			throw new Error('expected both add-token sheets');
		}
		expect(t3.sheet.model.fieldValue).toBe(USDT_CONTRACT);
		expect(t3.sheet.model.fieldError).toBeUndefined();
		expect(t3.sheet.model.ctaDisabled).toBe(false);
		expect(t3.sheet.model.result.kind).toBe('token');

		expect(t5.sheet.model.fieldError).toBe('无效的合约地址');
		// A rejected address must not leave a stale result card under it.
		expect(t5.sheet.model.result.kind).toBe('none');
		expect(t5.sheet.model.ctaDisabled).toBe(true);
	});

	it('t5b marks Avalanche incompatible and disables the CTA', () => {
		const m = buildFlowState('t5b', zh, IDENTICON_STUB);
		if (m.sheet?.kind !== 'add-token') throw new Error('expected the add-token sheet');
		expect(m.sheet.model.tab).toBe('native');
		if (m.sheet.model.result.kind !== 'network') throw new Error('expected a network result');
		expect(m.sheet.model.result.chip).toMatchObject({ text: '不兼容', tone: 'error' });
		expect(m.sheet.model.result.facts.map((f) => f.value)).toContain('43114');
		expect(m.sheet.model.ctaDisabled).toBe(true);
	});

	it('sd1b greys the off-network rows and selects only the on-network ones', () => {
		const m = buildFlowState('sd1b', zh, IDENTICON_STUB);
		if (m.base.kind !== 'send-pick') throw new Error('expected the send picker');
		const selection = m.base.model.selection;
		expect(selection).toBeDefined();
		// Never both: a row that cannot be picked cannot be picked.
		selection?.selected.forEach((on, i) => {
			expect(on && selection.dimmed[i], `row ${i} is both selected and dimmed`).toBe(false);
		});
		expect(selection?.selected.filter(Boolean)).toHaveLength(3);
		expect(m.base.model.notice?.text).toContain('Ethereum');
		expect(m.base.model.cta.accent).toBe(true);
	});

	it('sd2b totals three recipients to the amount sd2 sends alone', () => {
		const m = buildFlowState('sd2b', zh, IDENTICON_STUB);
		if (m.base.kind !== 'send-form') throw new Error('expected the send form');
		expect(m.base.model.mode).toBe('split');
		expect(m.base.model.recipients).toHaveLength(3);
		const total = m.base.model.recipients?.reduce((n, r) => n + Number(r.amount), 0);
		expect(total).toBe(120);
		expect(m.base.model.summary?.value).toBe('120 USDT');
		expect(m.base.model.summary?.detail).toBe('≈ $120.00');
	});

	it('sd2d sweeps three tokens to one address and says so', () => {
		const m = buildFlowState('sd2d', zh, IDENTICON_STUB);
		if (m.base.kind !== 'send-form') throw new Error('expected the send form');
		expect(m.base.model.mode).toBe('sweep');
		expect(m.base.model.sweepRows).toHaveLength(3);
		expect(m.base.model.recipient?.note).toBe('多币发送时收款人为同一地址');
	});

	it('sd2c counts only the rows it can actually import', () => {
		const m = buildFlowState('sd2c', zh, IDENTICON_STUB);
		if (m.sheet?.kind !== 'batch-import') throw new Error('expected the import sheet');
		const rows = (m.sheet.model.preview?.rows ?? []).flatMap((row) =>
			row.kind === 'row' ? [row] : []
		);
		expect(rows.filter((r) => r.ok)).toHaveLength(2);
		// The CTA promises what it delivers — four lines read, two importable —
		// and the row that is not says why.
		expect(m.sheet.model.preview?.label).toContain('4');
		expect(m.sheet.model.cta).toContain('2');
		expect(m.sheet.model.notices[0]).toContain('2');
		// A line that never became a row is drawn too, as it was written.
		expect(m.sheet.model.preview?.rows.some((row) => row.kind === 'refused')).toBe(true);
		expect(rows.find((r) => !r.ok)?.note).toBe(zh['send.batchDup']);
		// The drawn state is the refusal, with its reason on screen.
		expect(m.sheet.model.ctaDisabled).toBe(true);
		expect(m.sheet.model.total?.overText).toBeTruthy();
	});

	it('sd2f offers one fee token as chosen and the rest not', () => {
		const m = buildFlowState('sd2f', zh, IDENTICON_STUB);
		if (m.sheet?.kind !== 'fee-token') throw new Error('expected the fee sheet');
		expect(m.sheet.model.rows.filter((r) => r.selected)).toHaveLength(1);
		expect(m.sheet.model.rows[0].symbol).toBe('ETH');
	});

	it('sd3 shows four facts and sd3c adds the per-asset breakdown', () => {
		const sd3 = buildFlowState('sd3', zh, IDENTICON_STUB);
		const sd3c = buildFlowState('sd3c', zh, IDENTICON_STUB);
		if (sd3.base.kind !== 'send-confirm' || sd3c.base.kind !== 'send-confirm') {
			throw new Error('expected both confirmations');
		}
		expect(sd3.base.model.amount).toBe('120 USDT');
		expect(sd3.base.model.facts).toHaveLength(4);
		expect(sd3.base.model.breakdown).toBeUndefined();

		// The mock drew 项资产; the corpus key the legacy receipt already
		// shares says 种资产. Not worth churning a shared string over.
		expect(sd3c.base.model.amount).toBe('3 种资产');
		expect(sd3c.base.model.breakdown).toHaveLength(3);
		expect(sd3c.base.model.subline).toContain('$200.90');
	});

	it('the receipt keeps one accent CTA and only for the final state', () => {
		const stages = (['sd4a', 'sd4b', 'sd4c'] as const).map((id) => {
			const m = buildFlowState(id, zh, IDENTICON_STUB);
			if (m.base.kind !== 'send-receipt') throw new Error('expected a receipt');
			return m.base.model;
		});
		expect(stages.map((s) => s.stage)).toEqual(['submitting', 'submitted', 'confirmed']);
		expect(stages.map((s) => s.ctaAccent)).toEqual([false, false, true]);
		// A transaction still in flight offers no hash and no explorer link.
		expect(stages[0].hash).toBeUndefined();
		expect(stages[2].hash?.value).toBe('0x8f3a…c21d');
		expect(stages[0].captions).toContain('关闭此页交易会在后台继续');
	});

	it('every affordance the desktop send form draws has a panel behind it', () => {
		// DSD2L draws a recipient picker and a fee chevron; DSD2bL adds an
		// import pill. The mock set draws no panel for any of the three, so
		// these states exist to keep those affordances from being dead.
		for (const id of ['dsd2e', 'dsd2f', 'dsd2c'] as DesktopFlowStateIdSubset[]) {
			const model = buildDesktopFlowState(id, zh, IDENTICON_STUB);
			expect(model.body.kind, id).toBe(
				id === 'dsd2e' ? 'contact-pick' : id === 'dsd2f' ? 'fee-token' : 'batch-import'
			);
		}
	});

	it('the desktop scanner drops the torch a webcam does not have', () => {
		const phone = buildFlowState('s1', zh, IDENTICON_STUB);
		if (phone.base.kind !== 'scan') throw new Error('expected the scanner');
		expect(phone.base.model.tools.map((t) => t.id)).toEqual(['gallery', 'torch', 'flip']);
		expect(buildDesktopScan(zh).tools.map((t) => t.id)).toEqual(['gallery', 'flip']);
	});

	it('every desktop state past the first level offers a way back', () => {
		const nested: DesktopFlowStateIdSubset[] = [
			'dr2',
			'dr3',
			'da2',
			'da3',
			'dt3',
			'dt3b',
			'dsd2',
			'dsd2b',
			'dsd3',
			'dsd4'
		];
		for (const id of nested) {
			expect(buildDesktopFlowState(id, zh, IDENTICON_STUB).backLabel, id).toBeDefined();
		}
		for (const id of ['dr1', 'da1', 'dt1', 'dt4', 'dsd1'] as DesktopFlowStateIdSubset[]) {
			expect(buildDesktopFlowState(id, zh, IDENTICON_STUB).backLabel, id).toBeUndefined();
		}
	});

	it('pins the network chain ids the add-network card prints', () => {
		expect(NETWORKS.find((n) => n.name === 'Avalanche')?.chainId).toBe('43114');
		expect(NETWORKS.find((n) => n.name === 'Gnosis')?.chainId).toBe('100');
	});
});

type DesktopFlowStateIdSubset = (typeof DESKTOP_FLOW_STATES)[number];
