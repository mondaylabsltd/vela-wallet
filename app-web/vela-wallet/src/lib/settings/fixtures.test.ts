/**
 * Spec 023 gates: every settings key resolves in all 15 locales, the state-id
 * inventory covers every mock in `design/settings/`, each state builds, and the
 * numbers a reviewer would compare against the PNGs are pinned.
 */
import { describe, expect, it } from 'vitest';
import { rawResolve, resolveSettingsMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import {
	ACCOUNT_NAME,
	ADDRESS_FULL,
	buildDesktopState,
	buildMobileState,
	COMMUNITY_LINKS,
	communityRows,
	DESKTOP_STATES,
	LOCALE_ENDONYMS,
	MOBILE_STATES
} from './fixtures';
import { SETTINGS_KEYS } from './messages';
import { DESKTOP_SETTINGS_STATES, MOBILE_SETTINGS_STATES } from './model';

const IDENTICON_STUB = (seed: string) => `<svg data-seed="${seed}"></svg>`;

describe('settings messages', () => {
	it.each(SUPPORTED_LOCALES)('every settings key resolves in %s', (locale) => {
		for (const key of SETTINGS_KEYS) {
			const value = rawResolve(locale, key);
			expect(value, `${key} in ${locale}`).not.toBe(key);
			expect(value.trim()).not.toBe('');
		}
	});
});

// Spec 095 (App Review 5.1.1(i)): About links the privacy policy, the terms
// and the support page, on every shell, in every language.
describe('Settings → About', () => {
	it.each(SUPPORTED_LOCALES)('links the policy, the terms and support in %s', (locale) => {
		const about = buildMobileState('st14', resolveSettingsMessages(locale), IDENTICON_STUB).about;
		expect(about.links.slice(-3).map((link) => link.href)).toEqual([
			'https://getvela.app/privacy',
			'https://getvela.app/terms',
			'https://getvela.app/support'
		]);
		for (const link of about.links) {
			expect(link.label, locale).not.toMatch(/^about\./);
			expect(link.href).toBe(`https://${link.value}`);
		}
	});
});

// 078 (founder, 2026-09-27): the official accounts in Settings. The strings
// are pinned here so this shell cannot drift from getvela.app's footer or from
// the three native shells, which pin the same three.
describe('Settings → Community', () => {
	it('links exactly the three official accounts', () => {
		expect(COMMUNITY_LINKS.map((link) => [link.title, link.handle, link.url])).toEqual([
			['X (Twitter)', '@realvelawallet', 'https://x.com/realvelawallet'],
			['Telegram', '@velawallet', 'https://t.me/velawallet'],
			// Not the expired discord.gg/S6A8RyCk6.
			['Discord', 'discord.gg/23gWrtaYSa', 'https://discord.gg/23gWrtaYSa']
		]);
	});

	it('draws each as a row that leaves the app: its brand mark, its handle, the external mark', () => {
		expect(communityRows()).toEqual([
			{
				id: 'community-x',
				icon: 'brand-x',
				title: 'X (Twitter)',
				subtitle: '@realvelawallet',
				trailing: 'external',
				href: 'https://x.com/realvelawallet'
			},
			{
				id: 'community-telegram',
				icon: 'brand-telegram',
				title: 'Telegram',
				subtitle: '@velawallet',
				trailing: 'external',
				href: 'https://t.me/velawallet'
			},
			{
				id: 'community-discord',
				icon: 'brand-discord',
				title: 'Discord',
				subtitle: 'discord.gg/23gWrtaYSa',
				trailing: 'external',
				href: 'https://discord.gg/23gWrtaYSa'
			}
		]);
	});

	it('ends the phone list Advanced → Community → About, Send feedback', () => {
		const m = resolveSettingsMessages('en');
		const sections = buildMobileState('st1', m, IDENTICON_STUB).sections;
		const labels = sections.map((section) => section.label);
		expect(labels.slice(-3)).toEqual([m.sections.advanced, m.sections.community, undefined]);
		expect(sections.at(-2)?.rows.map((row) => row.id)).toEqual([
			'community-x',
			'community-telegram',
			'community-discord'
		]);
		expect(sections.at(-1)?.rows.map((row) => row.id)).toEqual(['about', 'feedback']);
		// The report opens a sheet here — a chevron, not the external mark the
		// Community rows above it wear.
		expect(sections.at(-1)?.rows[1].trailing).toBe('chevron');
		// And nowhere else: the first block is Contacts alone.
		expect(sections[0].rows.map((row) => row.id)).toEqual(['contacts']);
	});

	it.each(['en', 'zh', 'de', 'es-MX'] as const)('titles the group in %s', (locale) => {
		const m = resolveSettingsMessages(locale);
		expect(m.sections.community.trim()).not.toBe('');
		expect(m.sections.community).not.toBe('settings.sections.community');
	});
});

describe('state-id inventory (one id per mock in design/settings/)', () => {
	it('mobile states cover ST1–ST18 and SR1–SR5', () => {
		expect(MOBILE_STATES).toEqual(MOBILE_SETTINGS_STATES);
		// 16 ST mocks — ST1b, ST3b, ST9b, ST10b, ST10c and ST13b are their own
		// states, so the ST count is 22 — plus spec 102's ST17/ST17b (where you
		// review and sign) and ST18/ST18b (signing pages): 26 — plus ST10d, the
		// second refusal (no P-256 verifier; ST10c is missing contracts): 27.
		// Then SR1, SR2, SR2b, SR3, SR4, SR5.
		expect(MOBILE_STATES.filter((s) => s.startsWith('st'))).toHaveLength(27);
		expect(MOBILE_STATES.filter((s) => s.startsWith('sr'))).toHaveLength(6);
	});

	it('desktop states cover DST1–DST9 plus DST4b and DSR1', () => {
		expect(DESKTOP_STATES).toEqual(DESKTOP_SETTINGS_STATES);
		expect(DESKTOP_STATES).toEqual([
			'dst1',
			'dst2',
			'dst3',
			'dst4',
			'dst4b',
			'dst5',
			'dst6',
			'dst7',
			'dst8',
			// Spec 102: Settings → Signing pages.
			'dst9',
			'dsr1'
		]);
	});

	it('every state builds, and says which one it is', () => {
		const zh = resolveSettingsMessages('zh');
		for (const state of MOBILE_STATES) {
			expect(buildMobileState(state, zh, IDENTICON_STUB).state).toBe(state);
		}
		for (const state of DESKTOP_STATES) {
			expect(buildDesktopState(state, zh, IDENTICON_STUB).state).toBe(state);
		}
	});

	it('seeds the identicon with the full address, never a lowercased copy', () => {
		const zh = resolveSettingsMessages('zh');
		const st1 = buildMobileState('st1', zh, IDENTICON_STUB);
		expect(st1.account.identiconSvg).toBe(`<svg data-seed="${ADDRESS_FULL}"></svg>`);
		expect(st1.account.name).toBe(ACCOUNT_NAME);
	});
});

describe('page / overlay shape (what each mock actually shows)', () => {
	const zh = resolveSettingsMessages('zh');

	it('ST1 is the list with nothing over it; ST1b opens 高级', () => {
		expect(buildMobileState('st1', zh, IDENTICON_STUB).page).toBe('home');
		expect(buildMobileState('st1', zh, IDENTICON_STUB).overlay).toBe('none');
		const advanced = buildMobileState('st1b', zh, IDENTICON_STUB).sections.find(
			(s) => s.collapsible === true
		);
		expect(advanced?.collapsed).toBe(false);
		expect(
			buildMobileState('st1', zh, IDENTICON_STUB).sections.find((s) => s.collapsible)?.collapsed
		).toBe(true);
	});

	it('each picker mock opens its own sheet', () => {
		const pairs = [
			['st2', 'accounts'],
			['st3', 'sign-out'],
			['st4', 'language'],
			['st5', 'currency'],
			['st6', 'number-format'],
			['st7', 'date-format'],
			['st8', 'time-format'],
			['st13b', 'clear-caches'],
			['st15', 'feedback'],
			['st16', 'erase-device']
		] as const;
		for (const [state, overlay] of pairs) {
			expect(buildMobileState(state, zh, IDENTICON_STUB).overlay, state).toBe(overlay);
		}
	});

	it('each sub-page mock opens its own page', () => {
		const pairs = [
			['st9', 'networks'],
			['st9b', 'network-detail'],
			['st10', 'add-network'],
			['st11', 'rpc-providers'],
			['st12', 'endpoints'],
			['st13', 'storage'],
			['st14', 'about']
		] as const;
		for (const [state, page] of pairs) {
			expect(buildMobileState(state, zh, IDENTICON_STUB).page, state).toBe(page);
		}
	});

	it('the rescue states sit on the 钱包 tab, not on 设置', () => {
		for (const state of ['sr1', 'sr2', 'sr2b', 'sr3', 'sr4', 'sr5'] as const) {
			expect(buildMobileState(state, zh, IDENTICON_STUB).tab, state).toBe('wallet');
		}
	});
});

describe('canon numbers (pinned against the PNGs)', () => {
	const zh = resolveSettingsMessages('zh');
	const st1 = buildMobileState('st1', zh, IDENTICON_STUB);

	it('ST9 lists eight networks, the last one custom and removable', () => {
		expect(st1.networks.rows).toHaveLength(8);
		const last = st1.networks.rows.at(-1);
		expect(last?.name).toBe('X Layer');
		expect(last?.removable).toBe(true);
		expect(last?.badge).toBeUndefined();
	});

	it('ST9b flags the chain-ID mismatch; the plain detail does not', () => {
		expect(buildMobileState('st9b', zh, IDENTICON_STUB).networkDetail.callout?.tone).toBe('danger');
		expect(st1.networkDetail.callout).toBeUndefined();
	});

	it('ST10b passes every check; ST10c and ST10d are the two refusals, each with its own way on', () => {
		const ok = buildMobileState('st10b', zh, IDENTICON_STUB).addNetwork;
		const contracts = buildMobileState('st10c', zh, IDENTICON_STUB).addNetwork;
		const verifier = buildMobileState('st10d', zh, IDENTICON_STUB).addNetwork;
		expect(ok.checks?.map((c) => c.ok)).toEqual([true, true, true, true]);
		expect(ok.primary).toBeDefined();
		// ST10c — contracts are missing, the P-256 verifier is there: something
		// can be deployed, so Chain Setup is offered, opened on that chain.
		expect(contracts.checks?.map((c) => c.ok)).toEqual([true, false, true, false]);
		expect(contracts.primary).toBeUndefined();
		expect(contracts.callout?.text).toBe(
			'这个网络上还缺少 Vela 需要的部分合约。链配置工具会列出缺哪些、谁能部署。'
		);
		expect(contracts.secondary).toEqual({
			label: zh.addNetwork.openChainSetupTool,
			href: 'https://getvela.app/chain-setup?chain=48900'
		});
		expect(contracts.recheck).toBeDefined();
		// ST10d — no P-256 verifier: nothing to deploy, so no button; the line
		// says the network cannot run Vela wallets and not to send money there.
		expect(verifier.checks?.map((c) => c.ok)).toEqual([true, true, false, true]);
		expect(verifier.primary).toBeUndefined();
		expect(verifier.secondary).toBeUndefined();
		expect(verifier.callout?.text).toContain('转进去会被卡住');
		expect(verifier.recheck).toBeDefined();
	});

	it('ST13 accounts for 2.4 MB over three groups and 216 records', () => {
		expect(st1.storage.amount).toBe('2.4');
		expect(st1.storage.unit).toBe('MB');
		expect(st1.storage.summary).toContain('216');
		expect(st1.storage.groups.map((g) => g.items.length)).toEqual([4, 3, 1]);
		// Only the cache group offers a clear-them-all action.
		expect(st1.storage.groups.map((g) => g.action !== undefined)).toEqual([false, true, false]);
		// User data and connections clear destructively; caches do not.
		expect(st1.storage.groups[0].items.every((i) => i.destructive === true)).toBe(true);
		expect(st1.storage.groups[1].items.every((i) => i.destructive === undefined)).toBe(true);
	});

	it('the segments sum to the whole bar', () => {
		const total = st1.storage.segments.reduce((n, s) => n + s.fraction, 0);
		expect(total).toBeCloseTo(1, 5);
	});

	it('SR2 is offline with a way to fix it; SR2b is restored and done', () => {
		const failing = buildMobileState('sr2', zh, IDENTICON_STUB).rpcFix;
		const restored = buildMobileState('sr2b', zh, IDENTICON_STUB).rpcFix;
		expect(failing.badge.tone).toBe('error');
		expect(failing.callout.tone).toBe('warning');
		expect(failing.providers).toHaveLength(4);
		expect(restored.badge.tone).toBe('ok');
		expect(restored.callout.tone).toBe('success');
		// Nothing left to go and get once it works.
		expect(restored.providers).toBeUndefined();
		expect(restored.report).toBeUndefined();
	});

	it('SR3 tells rate-limiting (quiet, no button) from a dead RPC (loud, retry)', () => {
		const detail = buildMobileState('sr3', zh, IDENTICON_STUB).balanceDetail;
		expect(detail.pending[0].tone).toBe('neutral');
		expect(detail.pending[0].action).toBeUndefined();
		expect(detail.pending[1].tone).toBe('error');
		expect(detail.pending[1].action).toBeDefined();
	});

	it('SR1 names both unreachable networks in the banner', () => {
		const banner = buildMobileState('sr1', zh, IDENTICON_STUB).rpcBanner;
		expect(banner?.chips.map((c) => c.name)).toEqual(['Polygon', 'Gnosis']);
		expect(banner?.text).toContain('2');
	});

	it('ST3b adds the pending-upload warning that ST3 has no reason to show', () => {
		expect(buildMobileState('st3', zh, IDENTICON_STUB).signOutSheet.callout).toBeUndefined();
		expect(buildMobileState('st3b', zh, IDENTICON_STUB).signOutSheet.callout?.tone).toBe('warning');
	});

	it('every destructive confirm is red and every reversible one is accent', () => {
		expect(st1.signOutSheet.tone).toBe('danger');
		expect(st1.eraseSheet.tone).toBe('danger');
		expect(st1.clearCachesSheet.tone).toBe('accent');
	});
});

describe('desktop canon', () => {
	const zh = resolveSettingsMessages('zh');

	it('the second-level nav is the phone list, in the same order', () => {
		const nav = buildDesktopState('dst1', zh, IDENTICON_STUB).nav;
		expect(nav.map((n) => n.id)).toEqual([
			'account',
			'appearance',
			'localization',
			'networks',
			'rpc-providers',
			'endpoints',
			// Spec 068 — the stored default transaction speed, in the position
			// the phone's 高级 section gives it (after 服务端点, before 存储).
			'fee-speed',
			// Spec 102 — the signing pages this device trusts, in the phone's
			// 高级 position (just before 存储).
			'signing-pages',
			'storage',
			// 078 — the official accounts, directly above the last group.
			'community',
			// The last group, the same on all four shells (2026-09-27): About,
			// then the report (spec 081 FR-016 gave the report its desktop
			// entrance — before it, only `/gallery` reached it).
			'about',
			'feedback'
		]);
	});

	it('the desktop Community panel carries the phone group’s three rows', () => {
		const desktop = buildDesktopState('dst1', zh, IDENTICON_STUB);
		expect(desktop.community.rows).toEqual(communityRows());
	});

	it('each DST mock selects its own panel', () => {
		const pairs = [
			['dst1', 'account'],
			['dst2', 'appearance'],
			['dst3', 'localization'],
			['dst4', 'networks'],
			['dst5', 'rpc-providers'],
			['dst6', 'endpoints'],
			['dst7', 'storage'],
			['dst8', 'about']
		] as const;
		for (const [state, page] of pairs) {
			expect(buildDesktopState(state, zh, IDENTICON_STUB).page, state).toBe(page);
		}
	});

	it('DST3 is the only state with an open dropdown, and it hangs off 数字格式', () => {
		expect(buildDesktopState('dst3', zh, IDENTICON_STUB).dropdown?.rowId).toBe('number-format');
		for (const state of DESKTOP_STATES.filter((s) => s !== 'dst3')) {
			expect(buildDesktopState(state, zh, IDENTICON_STUB).dropdown, state).toBeUndefined();
		}
	});

	it('DST4b and DSR1 are the two dialogs', () => {
		expect(buildDesktopState('dst4b', zh, IDENTICON_STUB).overlay).toBe('add-network');
		expect(buildDesktopState('dsr1', zh, IDENTICON_STUB).overlay).toBe('rpc-fix');
		expect(buildDesktopState('dsr1', zh, IDENTICON_STUB).rpcBanner).toBeDefined();
	});

	it('the desktop expands Ethereum in place rather than pushing a page', () => {
		const rows = buildDesktopState('dst4', zh, IDENTICON_STUB).networks.rows;
		expect(rows.filter((r) => r.expanded === true).map((r) => r.id)).toEqual(['ethereum']);
	});

	it('only the desktop shows the average-latency and self-hosting extras', () => {
		const phone = buildMobileState('st11', zh, IDENTICON_STUB);
		const desk = buildDesktopState('dst5', zh, IDENTICON_STUB);
		expect(phone.rpcProviders.providers[0].support).not.toContain('112');
		expect(desk.rpcProviders.providers[0].support).toContain('112');
		expect(phone.endpoints.guide).toBeUndefined();
		expect(buildDesktopState('dst6', zh, IDENTICON_STUB).endpoints.guide).toBeDefined();
	});
});

describe('language endonyms', () => {
	it('names every shipped locale in its own language', () => {
		expect(LOCALE_ENDONYMS.map((l) => l.id)).toEqual([...SUPPORTED_LOCALES]);
	});

	it('reads the same whichever locale the app is in — they are data, not copy', () => {
		const zh = resolveSettingsMessages('zh');
		const en = resolveSettingsMessages('en');
		const zhRows = buildMobileState('st4', zh, IDENTICON_STUB).languageSheet.rows.slice(1);
		const enRows = buildMobileState('st4', en, IDENTICON_STUB).languageSheet.rows.slice(1);
		expect(zhRows.map((r) => r.label)).toEqual(enRows.map((r) => r.label));
	});
});
