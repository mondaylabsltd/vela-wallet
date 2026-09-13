//! Canonical contacts fixtures — the desktop port of
//! `specs/018-contacts-ui/data-model.md`. Names, addresses, amounts and dates
//! are data, verbatim across locales (spec FR-012); labels resolve through
//! `ContactsStrings`. Components never sort/filter/format — sections arrive
//! pre-grouped here.

use gpui::SharedString;

use crate::icons::Icon;
use crate::wallet::fill;
use crate::wallet::fixtures::{ActivityKind, ActivityRowModel, chain_arbitrum, chain_ethereum};

use super::ContactsStrings;

/// The gallery state inventory (desktop). `dc2n` (narrow overlay) is N/A on
/// native desktop — the window minimum is 1280 wide (research.md D6).
///
/// The chip strip is what a reviewer actually clicks; this constant is the
/// contract it is checked against (`gallery_exposes_every_desktop_contacts_state`).
#[allow(dead_code, reason = "gallery inventory contract, asserted by tests")]
pub const DESKTOP_STATES: [&str; 6] = ["dc1", "dc2", "dc3", "dc4", "dc5", "dc6"];

/// One canon contact. Identicon seed = `address_full` through the platform's
/// normalize route (spec 003 rule: never lowercase at a call site).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ContactFixture {
    pub name: &'static str,
    pub address_display: &'static str,
    pub address_full: &'static str,
    pub section: &'static str,
    /// Group membership (only 家人 has an opened member list in the mocks).
    pub group: Option<&'static str>,
    /// Only Alice carries recent activity (data-model.md §Contact detail).
    pub has_activity: bool,
}

const fn contact(
    name: &'static str,
    address_display: &'static str,
    address_full: &'static str,
    section: &'static str,
    group: Option<&'static str>,
    has_activity: bool,
) -> ContactFixture {
    ContactFixture {
        name,
        address_display,
        address_full,
        section,
        group,
        has_activity,
    }
}

/// The canonical 8-contact roster (8 位), pre-sorted into its letter sections.
/// Only Alice's full address appears in a mock (C2); the other seven are the
/// canon's pinned inventions whose first/last four hex chars match the mocks'
/// truncated displays (research.md D7).
pub const CONTACTS: [ContactFixture; 8] = [
    contact(
        "Alice",
        "0x9F3c…21aE",
        "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE",
        "A",
        Some("家人"),
        true,
    ),
    contact(
        "阿豪",
        "0x77Bd…4F02",
        "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02",
        "A",
        None,
        false,
    ),
    contact(
        "Bartholomew Vanderbilt-Konstantinopoulos.eth",
        "0x31c9…E77a",
        "0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a",
        "B",
        None,
        false,
    ),
    contact(
        "Bob · 泵泵",
        "0x44Aa…9C21",
        "0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21",
        "B",
        None,
        false,
    ),
    contact(
        "Charlie",
        "0x5eF0…3a9C",
        "0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C",
        "C",
        None,
        false,
    ),
    contact(
        "DAO 金库",
        "0xF00d…C0de",
        "0xF00dBaBe8712004343cD00926Ab004D6C042C0de",
        "D",
        None,
        false,
    ),
    contact(
        "hold on",
        "0xCafe…F00d",
        "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d",
        "H",
        None,
        false,
    ),
    contact(
        "妈妈",
        "0x88Ce…12aB",
        "0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB",
        "M",
        Some("家人"),
        false,
    ),
];

/// Group-only member — the recorded C1-vs-DC1 mock inconsistency
/// (spec Assumptions): 表弟 exists only inside the 家人 group fixture.
pub const COUSIN: ContactFixture = contact(
    "表弟",
    "0xA1c3…88dD",
    "0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD",
    "B",
    Some("家人"),
    false,
);

/// Identicon board seeds: the 8+1 canon addresses plus the placeholder.
pub const IDENTICON_CANON_SEEDS: [&str; 10] = [
    CONTACTS[0].address_full,
    CONTACTS[1].address_full,
    CONTACTS[2].address_full,
    CONTACTS[3].address_full,
    CONTACTS[4].address_full,
    CONTACTS[5].address_full,
    CONTACTS[6].address_full,
    CONTACTS[7].address_full,
    COUSIN.address_full,
    "",
];

/// Total roster size (rail 全部联系人 trailing count, DC1).
pub const TOTAL_CONTACTS: u32 = 8;

/// Letter sections, pre-grouped in canon order (A, B, C, D, H, M).
pub fn sections() -> Vec<(&'static str, Vec<ContactFixture>)> {
    let mut out: Vec<(&'static str, Vec<ContactFixture>)> = Vec::new();
    for c in CONTACTS {
        match out.last_mut() {
            Some((letter, rows)) if *letter == c.section => rows.push(c),
            _ => out.push((c.section, vec![c])),
        }
    }
    out
}

/// One group row of the desktop rail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GroupFixture {
    pub name: &'static str,
    pub count: u32,
}

pub const GROUPS: [GroupFixture; 3] = [
    GroupFixture {
        name: "家人",
        count: 3,
    },
    GroupFixture {
        name: "工作",
        count: 5,
    },
    GroupFixture {
        name: "交易所",
        count: 2,
    },
];

/// Members of a group, in the mock's order. Only 家人 is ever opened in the
/// mocks — the others are count-only (data-model.md §Groups).
pub fn group_members(group: usize) -> Vec<ContactFixture> {
    match group {
        0 => vec![CONTACTS[7], COUSIN, CONTACTS[0]], // 妈妈, 表弟, Alice
        _ => Vec::new(),
    }
}

// -- contact detail (Alice — dc2) -------------------------------------------

/// Third-column detail content: display-ready, straight from the canon.
pub struct ContactDetailModel {
    pub name: SharedString,
    /// The identicon seed — the ADDRESS, always. Owned since 031, because a
    /// live detail's seed is a real address rather than a `'static` literal.
    pub seed: SharedString,
    /// Group-membership chips (家人); empty for ungrouped contacts.
    pub chips: Vec<SharedString>,
    pub address_full: SharedString,
    pub activity: Vec<ActivityRowModel>,
}

pub fn contact_detail(s: &ContactsStrings, c: &ContactFixture) -> ContactDetailModel {
    ContactDetailModel {
        name: c.name.into(),
        seed: c.address_full.into(),
        chips: c.group.iter().map(|g| SharedString::from(*g)).collect(),
        address_full: c.address_full.into(),
        activity: if c.has_activity {
            alice_activity(s)
        } else {
            Vec::new()
        },
    }
}

/// Alice's 最近往来 (data-model.md table). The received entry mirrors the 015
/// wallet fixture's Alice row — one story across features; `8 月 5 日` is
/// literal fixture data, verbatim per FR-012.
pub fn alice_activity(s: &ContactsStrings) -> Vec<ActivityRowModel> {
    vec![
        ActivityRowModel {
            kind: ActivityKind::Received,
            title: s.label_received.clone(),
            subtitle: format!("{} 20:15 · Ethereum", s.yesterday).into(),
            amount: "+50".into(),
            unit: "USDC".into(),
            positive: true,
            badge: chain_ethereum(),
        },
        ActivityRowModel {
            kind: ActivityKind::Sent,
            title: s.label_sent.clone(),
            subtitle: "8 月 5 日 · Arbitrum".into(),
            amount: "−0.2".into(),
            unit: "ETH".into(),
            positive: false,
            badge: chain_arbitrum(),
        },
    ]
}

// -- menus (MenuFixture) ------------------------------------------------------

#[derive(Clone)]
pub struct MenuItemModel {
    pub icon: Icon,
    pub label: SharedString,
    pub destructive: bool,
}

#[derive(Clone)]
pub struct MenuModel {
    pub items: Vec<MenuItemModel>,
    /// Index after which the hairline divider renders (M2 anatomy).
    pub divider_after: Option<usize>,
}

fn item(icon: Icon, label: SharedString) -> MenuItemModel {
    MenuItemModel {
        icon,
        label,
        destructive: false,
    }
}

fn destructive(icon: Icon, label: SharedString) -> MenuItemModel {
    MenuItemModel {
        icon,
        label,
        destructive: true,
    }
}

/// Header ⋯ dropdown (dc5, M1): 导入通讯录 / 导出全部通讯录.
pub fn header_dropdown(s: &ContactsStrings) -> MenuModel {
    MenuModel {
        items: vec![
            item(Icon::Download, s.import_all.clone()),
            item(Icon::Upload, s.export_all.clone()),
        ],
        divider_after: None,
    }
}

/// Group-row context menu (dc6, M2): 重命名分组 / 导入到本组 / 导出本组 /
/// divider / 删除分组 (destructive).
pub fn group_context(s: &ContactsStrings) -> MenuModel {
    MenuModel {
        items: vec![
            item(Icon::Pencil, s.group_rename.clone()),
            item(Icon::Download, s.import_group.clone()),
            item(Icon::Upload, s.export_group.clone()),
            destructive(Icon::Trash2, s.group_delete.clone()),
        ],
        divider_after: Some(2),
    }
}

/// Contact-row context menu (desktop SPEC; component board only in this
/// feature): 转账 / 收款 / 复制地址 / 编辑 / 移入分组 / divider / 删除.
pub fn contact_context(s: &ContactsStrings) -> MenuModel {
    MenuModel {
        items: vec![
            item(Icon::ArrowUpRight, s.action_send.clone()),
            item(Icon::ArrowDownLeft, s.action_receive.clone()),
            item(Icon::Copy, s.copy_address.clone()),
            item(Icon::Pencil, s.edit.clone()),
            item(Icon::UsersRound, s.move_group.clone()),
            destructive(Icon::Trash2, s.delete.clone()),
        ],
        divider_after: Some(4),
    }
}

/// Which groups this contact is in — the answer visible on every row.
///
/// A menu rather than a dialog with a Save button, for the reason the explore
/// one is a menu: the question is "which of these", and a tick per row is the
/// shortest way to both ask it and show the current answer. Every tap sends
/// the WHOLE membership back (`SetContactGroups`), which is the event the core
/// offers and the shape it normalises.
pub fn contact_group_pick(groups: &[(SharedString, bool)]) -> MenuModel {
    pick_menu(groups, Icon::UsersRound)
}

/// Which contacts this group holds — the same menu the other way round.
///
/// One shape for both directions, because they are one question asked from two
/// screens, and two shapes would be two places to get the tick wrong.
pub fn group_member_pick(contacts: &[(SharedString, bool)]) -> MenuModel {
    pick_menu(contacts, Icon::UserRoundPlus)
}

fn pick_menu(rows: &[(SharedString, bool)], unpicked: Icon) -> MenuModel {
    MenuModel {
        divider_after: None,
        items: rows
            .iter()
            .map(|(name, member)| MenuItemModel {
                // The tick IS the state: `Check` for a row that is in the set,
                // the neutral glyph for one that is not.
                icon: if *member { Icon::Check } else { unpicked },
                label: name.clone(),
                destructive: false,
            })
            .collect(),
    }
}

// -- assembled labels ---------------------------------------------------------

/// `3 位成员` (dc4 content header).
pub fn members_count_label(s: &ContactsStrings, count: u32) -> SharedString {
    fill(&s.members_count, "count", &count.to_string()).into()
}

/// `群发转账：向本组 3 人转账，金额可分别设置。` (dc4 caption).
pub fn batch_send_caption(s: &ContactsStrings, count: u32) -> SharedString {
    fill(&s.batch_send_hint_titled, "count", &count.to_string()).into()
}

/// `没有匹配「zzz」的结果` (search-empty board variant, query is data).
pub fn no_results_label(s: &ContactsStrings, query: &str) -> SharedString {
    fill(&s.no_results, "query", query).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loc::Loc;

    /// The gallery inventory is pinned exactly (data-model.md §Screen states).
    #[test]
    fn desktop_state_inventory_is_pinned() {
        assert_eq!(DESKTOP_STATES, ["dc1", "dc2", "dc3", "dc4", "dc5", "dc6"]);
    }

    /// The 8+1 canon addresses, byte-exact (research.md D7 — identicon seeds
    /// must round-trip unchanged on every platform).
    #[test]
    fn canon_addresses_are_byte_exact() {
        let expected: [(&str, &str, &str); 9] = [
            (
                "Alice",
                "0x9F3c…21aE",
                "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE",
            ),
            (
                "阿豪",
                "0x77Bd…4F02",
                "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02",
            ),
            (
                "Bartholomew Vanderbilt-Konstantinopoulos.eth",
                "0x31c9…E77a",
                "0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a",
            ),
            (
                "Bob · 泵泵",
                "0x44Aa…9C21",
                "0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21",
            ),
            (
                "Charlie",
                "0x5eF0…3a9C",
                "0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C",
            ),
            (
                "DAO 金库",
                "0xF00d…C0de",
                "0xF00dBaBe8712004343cD00926Ab004D6C042C0de",
            ),
            (
                "hold on",
                "0xCafe…F00d",
                "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d",
            ),
            (
                "妈妈",
                "0x88Ce…12aB",
                "0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB",
            ),
            (
                "表弟",
                "0xA1c3…88dD",
                "0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD",
            ),
        ];
        let roster: Vec<ContactFixture> = CONTACTS
            .into_iter()
            .chain(std::iter::once(COUSIN))
            .collect();
        assert_eq!(roster.len(), expected.len());
        for (c, (name, display, full)) in roster.iter().zip(expected) {
            assert_eq!(c.name, name);
            assert_eq!(c.address_display, display, "{name}: display drifted");
            assert_eq!(c.address_full, full, "{name}: full address drifted");
        }
    }

    /// Sections arrive pre-grouped in canon order: A, B, C, D, H, M.
    #[test]
    fn sections_are_pregrouped_in_canon_order() {
        let sections = sections();
        let letters: Vec<&str> = sections.iter().map(|(l, _)| *l).collect();
        assert_eq!(letters, ["A", "B", "C", "D", "H", "M"]);
        let counts: Vec<usize> = sections.iter().map(|(_, rows)| rows.len()).collect();
        assert_eq!(counts, [2, 2, 1, 1, 1, 1]);
        assert_eq!(
            CONTACTS.len() as u32,
            TOTAL_CONTACTS,
            "roster is the canonical 8 位"
        );
    }

    /// 家人 members in the mock's order (DC4): 妈妈, 表弟, Alice.
    #[test]
    fn family_group_members_match_dc4() {
        let members = group_members(0);
        let names: Vec<&str> = members.iter().map(|m| m.name).collect();
        assert_eq!(names, ["妈妈", "表弟", "Alice"]);
        assert_eq!(GROUPS[0].count as usize, members.len());
        // 工作 / 交易所 are count-only in the mocks.
        assert!(group_members(1).is_empty());
        assert!(group_members(2).is_empty());
        assert_eq!(GROUPS[1].count, 5);
        assert_eq!(GROUPS[2].count, 2);
    }

    /// FR-012: the zh locale reproduces the mock copy verbatim (the desktop
    /// twin of the web fixtures test).
    #[test]
    fn zh_fixtures_match_the_mocks() {
        // SAFETY: test-local env pin, same pattern the loc tests rely on.
        unsafe { std::env::set_var("VELA_LANG", "zh") };
        let loc = Loc::from_env();
        let s = ContactsStrings::resolve(&loc);

        assert_eq!(s.title.as_ref(), "通讯录");
        assert_eq!(s.search_placeholder.as_ref(), "搜索名字、ENS 或地址");
        assert_eq!(s.all_contacts.as_ref(), "全部联系人");
        assert_eq!(s.section_groups.as_ref(), "分组");
        assert_eq!(s.group_new.as_ref(), "新建分组");
        assert_eq!(s.add_contact.as_ref(), "添加联系人");
        assert_eq!(s.section_contacts.as_ref(), "联系人");
        assert_eq!(s.batch_send.as_ref(), "群发转账");
        assert_eq!(s.add_member.as_ref(), "添加成员");
        assert_eq!(s.empty.as_ref(), "还没有联系人");
        assert_eq!(
            s.empty_hint.as_ref(),
            "添加常用地址，转账时不再反复粘贴。也可以从文件导入现有通讯录。"
        );
        assert_eq!(s.import_file.as_ref(), "从文件导入");
        assert_eq!(s.recent_activity.as_ref(), "最近往来");
        assert_eq!(s.view_all_activity.as_ref(), "查看全部往来");
        assert_eq!(s.delete_contact.as_ref(), "删除联系人");
        assert_eq!(s.edit.as_ref(), "编辑");

        assert_eq!(members_count_label(&s, 3).as_ref(), "3 位成员");
        assert_eq!(
            batch_send_caption(&s, 3).as_ref(),
            "群发转账：向本组 3 人转账，金额可分别设置。"
        );

        // dc5/dc6 menu rows (M1/M2).
        let dropdown = header_dropdown(&s);
        let labels: Vec<&str> = dropdown.items.iter().map(|i| i.label.as_ref()).collect();
        assert_eq!(labels, ["导入通讯录", "导出全部通讯录"]);
        assert_eq!(dropdown.divider_after, None);

        let context = group_context(&s);
        let labels: Vec<&str> = context.items.iter().map(|i| i.label.as_ref()).collect();
        assert_eq!(labels, ["重命名分组", "导入到本组", "导出本组", "删除分组"]);
        assert_eq!(context.divider_after, Some(2));
        assert!(context.items[3].destructive, "删除分组 is destructive");

        // Alice's 最近往来 (dc2).
        let rows = alice_activity(&s);
        assert_eq!(rows[0].title.as_ref(), "已收到");
        assert_eq!(rows[0].subtitle.as_ref(), "昨天 20:15 · Ethereum");
        assert_eq!(rows[0].amount.as_ref(), "+50");
        assert_eq!(rows[0].unit.as_ref(), "USDC");
        assert_eq!(rows[1].title.as_ref(), "已发送");
        assert_eq!(rows[1].subtitle.as_ref(), "8 月 5 日 · Arbitrum");
        assert_eq!(rows[1].amount.as_ref(), "−0.2");
        assert_eq!(rows[1].unit.as_ref(), "ETH");

        // Alice detail carries the 家人 chip; ungrouped contacts carry none.
        let alice = contact_detail(&s, &CONTACTS[0]);
        let chips: Vec<&str> = alice.chips.iter().map(|c| c.as_ref()).collect();
        assert_eq!(chips, ["家人"]);
        assert_eq!(alice.activity.len(), 2);
        let charlie = contact_detail(&s, &CONTACTS[4]);
        assert!(charlie.chips.is_empty());
        assert!(charlie.activity.is_empty());
    }
}

/// The A–Z roster as the mocks draw it, in model form.
///
/// An ADAPTER, not a new fixture: every value comes from `CONTACTS` above. It
/// exists so the screen takes the same shape from either side of the seam.
#[must_use]
pub fn sections_model() -> Vec<(SharedString, Vec<crate::contacts::model::ContactRowModel>)> {
    sections()
        .into_iter()
        .map(|(letter, rows)| {
            (
                SharedString::from(letter),
                rows.into_iter()
                    .map(|c| crate::contacts::model::ContactRowModel {
                        name: SharedString::from(c.name),
                        address_display: SharedString::from(c.address_display),
                        address_full: SharedString::from(c.address_full),
                        section: SharedString::from(c.section),
                    })
                    .collect(),
            )
        })
        .collect()
}

/// One fixture contact in model form — for the gallery boards, which pick
/// individual mocks rather than a whole roster.
#[must_use]
pub fn row_model(c: ContactFixture) -> crate::contacts::model::ContactRowModel {
    crate::contacts::model::ContactRowModel {
        name: SharedString::from(c.name),
        address_display: SharedString::from(c.address_display),
        address_full: SharedString::from(c.address_full),
        section: SharedString::from(c.section),
    }
}

/// A group's members in model form. The group screen stays fixture-driven in
/// spec 030 — the core carries groups, but wiring the member list is the
/// interaction work US2 scopes to the roster first.
#[must_use]
pub fn group_members_model(group: usize) -> Vec<crate::contacts::model::ContactRowModel> {
    group_members(group)
        .into_iter()
        .map(|c| crate::contacts::model::ContactRowModel {
            name: SharedString::from(c.name),
            address_display: SharedString::from(c.address_display),
            address_full: SharedString::from(c.address_full),
            section: SharedString::from(c.section),
        })
        .collect()
}

#[cfg(test)]
mod row_adapter_tests {
    use super::*;

    /// The adapter must reproduce what the screen drew before the seam existed.
    #[test]
    fn the_roster_adapter_reproduces_the_mock_exactly() {
        let plain = sections();
        let model = sections_model();
        assert_eq!(model.len(), plain.len());
        for ((letter, rows), (m_letter, m_rows)) in plain.iter().zip(&model) {
            assert_eq!(m_letter.as_ref(), *letter);
            assert_eq!(m_rows.len(), rows.len());
            for (c, m) in rows.iter().zip(m_rows) {
                assert_eq!(m.name.as_ref(), c.name);
                assert_eq!(m.address_display.as_ref(), c.address_display);
                assert_eq!(m.address_full.as_ref(), c.address_full);
            }
        }
    }
}
