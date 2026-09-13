/**
 * Spec 018 gates: every contacts key resolves in all 15 locales (US4), the
 * state-id inventory matches data-model.md exactly, the zh mock copy is
 * verbatim (FR-012), and the 8+1 canon addresses are byte-exact (research D7).
 */
import { describe, expect, it } from 'vitest';
import { rawResolve, resolveContactsMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import {
	addMenu,
	buildDesktopState,
	buildMobileState,
	CONTACTS,
	CONTACT_BOARD_SEEDS,
	contactContextMenu,
	DESKTOP_STATES,
	GROUP_ONLY_MEMBER,
	groupContextMenu,
	groupMenuMobile,
	headerDropdown,
	INDEX_LETTERS,
	MOBILE_STATES,
	SECTION_LETTERS
} from './fixtures';
import { CONTACTS_KEYS } from './messages';
import { DESKTOP_CONTACTS_STATES, MOBILE_CONTACTS_STATES } from './model';

const IDENTICON_STUB = (seed: string) => `<svg data-seed="${seed}"></svg>`;

describe('contacts messages', () => {
	it.each(SUPPORTED_LOCALES)('every contacts key resolves in %s', (locale) => {
		for (const key of CONTACTS_KEYS) {
			const value = rawResolve(locale, key);
			expect(value, `${key} in ${locale}`).not.toBe(key);
			expect(value.trim()).not.toBe('');
		}
	});
});

describe('state-id inventory (data-model.md §Screen states)', () => {
	it('mobile states are exactly c1…c6 plus c1s/c1f/c2s', () => {
		expect(MOBILE_STATES).toEqual(['c1', 'c1s', 'c1f', 'c2', 'c2s', 'c3', 'c4', 'c5', 'c6']);
		expect(MOBILE_STATES).toEqual(MOBILE_CONTACTS_STATES);
	});

	it('desktop states are exactly dc1…dc6 plus dc2n', () => {
		expect(DESKTOP_STATES).toEqual(['dc1', 'dc2', 'dc3', 'dc4', 'dc5', 'dc6', 'dc2n']);
		expect(DESKTOP_STATES).toEqual(DESKTOP_CONTACTS_STATES);
	});

	it('every state builds', () => {
		const zh = resolveContactsMessages('zh');
		for (const state of MOBILE_STATES) {
			expect(buildMobileState(state, zh, IDENTICON_STUB).state).toBe(state);
		}
		for (const state of DESKTOP_STATES) {
			expect(buildDesktopState(state, zh, IDENTICON_STUB).state).toBe(state);
		}
	});
});

describe('canon addresses (research.md D7 — byte-exact across platforms)', () => {
	it('pins the 8 位 roster', () => {
		expect(CONTACTS.map((c) => [c.name, c.addressDisplay, c.addressFull])).toEqual([
			['Alice', '0x9F3c…21aE', '0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE'],
			['阿豪', '0x77Bd…4F02', '0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02'],
			[
				'Bartholomew Vanderbilt-Konstantinopoulos.eth',
				'0x31c9…E77a',
				'0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a'
			],
			['Bob · 泵泵', '0x44Aa…9C21', '0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21'],
			['Charlie', '0x5eF0…3a9C', '0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C'],
			['DAO 金库', '0xF00d…C0de', '0xF00dBaBe8712004343cD00926Ab004D6C042C0de'],
			['hold on', '0xCafe…F00d', '0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d'],
			['妈妈', '0x88Ce…12aB', '0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB']
		]);
	});

	it('pins the group-only member 表弟', () => {
		expect(GROUP_ONLY_MEMBER.name).toBe('表弟');
		expect(GROUP_ONLY_MEMBER.addressDisplay).toBe('0xA1c3…88dD');
		expect(GROUP_ONLY_MEMBER.addressFull).toBe('0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD');
	});

	it('display heads and tails match the full addresses', () => {
		for (const c of [...CONTACTS, GROUP_ONLY_MEMBER]) {
			const [head, tail] = c.addressDisplay.split('…');
			expect(c.addressFull.startsWith(head), c.name).toBe(true);
			expect(c.addressFull.endsWith(tail), c.name).toBe(true);
			expect(c.addressFull).toHaveLength(42);
		}
	});

	it('seeds the identicon with the full address, never a lowercased copy', () => {
		const zh = resolveContactsMessages('zh');
		const m = buildMobileState('c1', zh, IDENTICON_STUB);
		const rows = (m.list?.sections ?? []).flatMap((s) => s.contacts);
		for (const row of rows) {
			expect(row.identiconSvg).toContain(row.addressFull);
		}
	});

	it('boards the 9 canon seeds plus the invalid placeholder', () => {
		expect(CONTACT_BOARD_SEEDS).toHaveLength(10);
		expect(CONTACT_BOARD_SEEDS.at(-1)).toBe('');
	});
});

describe('fixture canon (zh mock verbatim)', () => {
	const zh = resolveContactsMessages('zh');

	it('c1 carries the C1 header, groups and sectioned list', () => {
		const m = buildMobileState('c1', zh, IDENTICON_STUB);
		expect(m.title).toBe('通讯录');
		expect(m.list?.search.placeholder).toBe('搜索名字、ENS 或地址');
		expect(m.list?.groupsTitle).toBe('分组');
		expect(m.list?.groupsAction).toBe('管理');
		expect(m.list?.groups.map((g) => `${g.name} ${g.countLabel}`)).toEqual([
			'家人 3 人',
			'工作 5 人',
			'交易所 2 人'
		]);
		expect(m.list?.contactsTitle).toBe('联系人');
		expect(m.list?.contactsCount).toBe('8 位');
		expect(m.list?.sections.map((s) => s.letter)).toEqual([...SECTION_LETTERS]);
		expect(m.list?.indexLetters).toEqual(INDEX_LETTERS);
		expect(m.list?.indexLetters).toHaveLength(27);
		expect(m.tabs.contacts).toBe('通讯录');
	});

	it('c1s pins the swipe-revealed 阿豪 row with 转账 / 删除', () => {
		const m = buildMobileState('c1s', zh, IDENTICON_STUB);
		expect(m.list?.revealed).toEqual({ letter: 'A', index: 1 });
		expect(m.list?.sections[0].contacts[1].name).toBe('阿豪');
		expect(m.list?.swipeActions).toEqual({ send: '转账', delete: '删除' });
	});

	it('c1f ships the pre-filtered "Ali" list (fixtures filter, components do not)', () => {
		const m = buildMobileState('c1f', zh, IDENTICON_STUB);
		expect(m.list?.search.query).toBe('Ali');
		expect(m.list?.sections.map((s) => s.letter)).toEqual(['A']);
		expect(m.list?.sections[0].contacts.map((c) => c.name)).toEqual(['Alice']);
		expect(m.list?.contactsCount).toBe('1 位');
	});

	it('c2 carries the C2 detail: chips, actions, two-line address, 最近往来', () => {
		const m = buildMobileState('c2', zh, IDENTICON_STUB);
		const d = m.detail;
		expect(d?.contact.name).toBe('Alice');
		expect(d?.contact.addressDisplay).toBe('0x9F3c…21aE');
		expect(d?.chips).toEqual(['家人']);
		expect(d?.addChipLabel).toBe('分组');
		expect(d?.actions).toEqual({ send: '转账', receive: '收款', qr: '二维码' });
		expect(d?.address.label).toBe('地址');
		expect(d?.address.lines).toEqual(['0x9F3cA71b04E82f5C55d9', 'B21aE00734F8Dd8021aE']);
		expect(d?.address.lines.join('')).toBe(CONTACTS[0].addressFull);
		expect(d?.activityTitle).toBe('最近往来');
		expect(d?.activityAction).toBe('全部');
		expect(d?.activityLink).toBe('查看全部往来');
		expect(d?.rows[0]).toMatchObject({
			title: '已收到',
			subtitle: '昨天 20:15 · Ethereum',
			amount: '+50',
			unit: 'USDC',
			positive: true
		});
		expect(d?.rows[1]).toMatchObject({
			title: '已发送',
			subtitle: '8 月 5 日 · Arbitrum',
			amount: '−0.2',
			unit: 'ETH',
			positive: false
		});
		expect(d?.deleteLabel).toBe('删除联系人');
	});

	it('c2s raises the destructive confirm naming the contact', () => {
		const m = buildMobileState('c2s', zh, IDENTICON_STUB);
		expect(m.confirm).toEqual({
			title: '删除联系人？',
			body: 'Alice 将从通讯录中移除。',
			confirm: '删除',
			cancel: '取消'
		});
	});

	it('c3 shows the C3 empty copy with both CTAs', () => {
		const m = buildMobileState('c3', zh, IDENTICON_STUB);
		expect(m.empty).toEqual({
			title: '还没有联系人',
			caption: '添加常用地址，转账时不再反复粘贴。也可以从文件导入现有通讯录。',
			primary: '添加联系人',
			secondary: '从文件导入'
		});
		expect(m.list?.sections).toEqual([]);
	});

	it('c4 shows 家人 · 3 位成员 with the pinned 群发转账 CTA', () => {
		const m = buildMobileState('c4', zh, IDENTICON_STUB);
		expect(m.group?.group.name).toBe('家人');
		expect(m.group?.group.membersLabel).toBe('3 位成员');
		expect(m.group?.group.members.map((c) => c.name)).toEqual(['妈妈', '表弟', 'Alice']);
		expect(m.group?.addMember).toBe('添加成员');
		expect(m.group?.cta).toBe('群发转账');
		expect(m.group?.ctaCaption).toBe('向本组 3 人转账，金额可分别设置。');
		expect(m.group?.captionTitled).toBe('群发转账：向本组 3 人转账，金额可分别设置。');
	});

	it('c5 / c6 open the C5 and C6 sheets', () => {
		expect(buildMobileState('c5', zh, IDENTICON_STUB).sheet?.items.map((i) => i.label)).toEqual([
			'新建联系人',
			'从文件导入',
			'导出通讯录'
		]);
		const c6 = buildMobileState('c6', zh, IDENTICON_STUB).sheet;
		expect(c6?.items.map((i) => i.label)).toEqual([
			'编辑分组',
			'导入到本组',
			'导出本组',
			'删除分组'
		]);
		expect(c6?.items.at(-1)?.destructive).toBe(true);
		expect(c6?.items[2].dividerAfter).toBe(true);
		expect(c6?.cancel).toBe('取消');
	});
});

describe('desktop canon (zh DC mocks verbatim)', () => {
	const zh = resolveContactsMessages('zh');

	it('dc1 rails 全部联系人 8 with the three groups and 新建分组', () => {
		const m = buildDesktopState('dc1', zh, IDENTICON_STUB);
		expect(m.title).toBe('通讯录');
		expect(m.addLabel).toBe('添加联系人');
		expect(m.search.shortcut).toBe('⌘F');
		expect(m.rail.allLabel).toBe('全部联系人');
		expect(m.rail.allCount).toBe('8');
		expect(m.rail.allSelected).toBe(true);
		expect(m.rail.groups.map((g) => `${g.name} ${g.count}`)).toEqual([
			'家人 3',
			'工作 5',
			'交易所 2'
		]);
		expect(m.rail.newGroup).toBe('新建分组');
		expect(m.sidebar.nav.find((n) => n.selected)?.id).toBe('contacts');
		expect(m.initialPanel).toBe('none');
	});

	it('dc2 opens the contact-detail third column on Alice', () => {
		const m = buildDesktopState('dc2', zh, IDENTICON_STUB);
		expect(m.initialPanel).toBe('contact-detail');
		expect(m.panelTitle).toBe('联系人');
		expect(m.selectedContact).toBe('Alice');
		expect(m.detail?.editLabel).toBe('编辑');
		expect(m.detail?.deleteLabel).toBe('删除联系人');
		expect(m.forceOverlay).toBe(false);
	});

	it('dc2n is dc2 in the narrow overlay mode', () => {
		const m = buildDesktopState('dc2n', zh, IDENTICON_STUB);
		expect(m.initialPanel).toBe('contact-detail');
		expect(m.forceOverlay).toBe(true);
	});

	it('dc3 rails 全部联系人 0 with 新建分组 only', () => {
		const m = buildDesktopState('dc3', zh, IDENTICON_STUB);
		expect(m.rail.allCount).toBe('0');
		expect(m.rail.groups).toEqual([]);
		expect(m.empty?.title).toBe('还没有联系人');
		expect(m.sections).toEqual([]);
	});

	it('dc4 swaps the list for the 家人 group view', () => {
		const m = buildDesktopState('dc4', zh, IDENTICON_STUB);
		expect(m.rail.selectedGroup).toBe('家人');
		expect(m.rail.allSelected).toBe(false);
		expect(m.group?.group.membersLabel).toBe('3 位成员');
		expect(m.group?.captionTitled).toBe('群发转账：向本组 3 人转账，金额可分别设置。');
		expect(m.sections).toEqual([]);
	});

	it('dc5 / dc6 pre-open the header dropdown and the group context menu', () => {
		expect(buildDesktopState('dc5', zh, IDENTICON_STUB).openMenu).toBe('header');
		expect(buildDesktopState('dc6', zh, IDENTICON_STUB).openMenu).toBe('group');
	});
});

describe('menu fixtures (data-model.md §Menus)', () => {
	const zh = resolveContactsMessages('zh');

	it('M1 header dropdown lists 导入通讯录 / 导出全部通讯录 with no cancel', () => {
		const menu = headerDropdown(zh);
		expect(menu.kind).toBe('dropdown');
		expect(menu.items.map((i) => [i.icon, i.label])).toEqual([
			['download', '导入通讯录'],
			['upload', '导出全部通讯录']
		]);
		expect(menu.cancel).toBeUndefined();
	});

	it('M2 group context menu carries the divider and the destructive row', () => {
		const menu = groupContextMenu(zh);
		expect(menu.kind).toBe('context');
		expect(menu.items.map((i) => i.label)).toEqual([
			'重命名分组',
			'导入到本组',
			'导出本组',
			'删除分组'
		]);
		expect(menu.items[2].dividerAfter).toBe(true);
		expect(menu.items[3].destructive).toBe(true);
		expect(menu.cancel).toBeUndefined();
	});

	it('the contact context menu renders the desktop SPEC items', () => {
		expect(contactContextMenu(zh).items.map((i) => i.label)).toEqual([
			'转账',
			'收款',
			'复制地址',
			'编辑',
			'移入分组',
			'删除'
		]);
	});

	it('sheets end with a separate 取消 button', () => {
		expect(addMenu(zh).cancel).toBe('取消');
		expect(groupMenuMobile(zh).cancel).toBe('取消');
	});
});

describe('en resolves too (US4: no key leaks, locale switch re-renders)', () => {
	const en = resolveContactsMessages('en');

	it('c4 and dc1 carry English copy with the counts interpolated', () => {
		const m = buildMobileState('c4', en, IDENTICON_STUB);
		expect(m.group?.cta).toBe('Send to group');
		expect(m.group?.ctaCaption).toContain('3');
		expect(m.group?.ctaCaption).not.toContain('{{');

		const d = buildDesktopState('dc1', en, IDENTICON_STUB);
		expect(d.rail.allLabel).toBe('All contacts');
		expect(d.title).not.toContain('.');
		// Names and addresses are data — identical in every locale.
		expect(d.detail?.contact.name).toBe('Alice');
		expect(d.detail?.contact.addressFull).toBe(CONTACTS[0].addressFull);
	});
});
