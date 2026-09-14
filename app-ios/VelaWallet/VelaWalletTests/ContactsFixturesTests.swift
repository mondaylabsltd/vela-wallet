//
//  ContactsFixturesTests.swift
//  VelaWalletTests
//
//  Spec 018 fixture canon (data-model.md, FR-012): the builders must
//  reproduce the mock content verbatim so visual diffing against
//  docs/design/contacts/ stays meaningful. Pins the state-id inventory, the zh
//  copy, and the 8+1 canon addresses byte-exact.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ContactsFixturesTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    // MARK: - State inventory

    @Test func allTwelveMobileStatesExist() {
        #expect(ContactsStateId.allCases.map(\.rawValue) ==
            ["c1", "c1s", "c1f", "c2", "c2s", "c3", "c4", "c5", "c6", "c7", "c8", "c9"])
        for state in ContactsStateId.allCases {
            #expect(ContactsFixtures.buildMobileState(state, loc: loc).state == state)
        }
    }

    @Test func statesResolveToTheRightScreen() {
        #expect(ContactsFixtures.buildMobileState(.c1, loc: loc).home != nil)
        #expect(ContactsFixtures.buildMobileState(.c1s, loc: loc).home != nil)
        #expect(ContactsFixtures.buildMobileState(.c1f, loc: loc).home != nil)
        #expect(ContactsFixtures.buildMobileState(.c3, loc: loc).home != nil)
        #expect(ContactsFixtures.buildMobileState(.c5, loc: loc).home != nil)
        #expect(ContactsFixtures.buildMobileState(.c2, loc: loc).detail != nil)
        #expect(ContactsFixtures.buildMobileState(.c2s, loc: loc).detail != nil)
        #expect(ContactsFixtures.buildMobileState(.c4, loc: loc).group != nil)
        #expect(ContactsFixtures.buildMobileState(.c6, loc: loc).group != nil)
        // C7 is the add form over the list; C8 the edit form over the contact;
        // C9 the contact with the star lit.
        #expect(ContactsFixtures.buildMobileState(.c7, loc: loc).home != nil)
        #expect(ContactsFixtures.buildMobileState(.c8, loc: loc).detail != nil)
        #expect(ContactsFixtures.buildMobileState(.c9, loc: loc).detail != nil)
    }

    // MARK: - C7 / C8 / C9 (spec 054 US5a)

    /// The add form is empty and its Save is SHUT. A form that offered to save
    /// nothing would be a button that does nothing — the shape this program
    /// has found three times already.
    @Test func theAddFormIsEmptyAndGated() throws {
        let home = try #require(ContactsFixtures.buildMobileState(.c7, loc: loc).home)
        let form = try #require(home.form)
        #expect(form.title == loc.t("contacts.addTitle"))
        #expect(form.name.isEmpty)
        #expect(form.address.isEmpty)
        #expect(!form.saveEnabled)
        #expect(!form.addressLocked, "a new contact's address is the one thing being typed")
    }

    /// The edit form is filled, saveable, and its address is LOCKED: in the
    /// address book an address is the identity of the person, so typing a
    /// different one would name somebody else rather than rename this one.
    @Test func theEditFormLocksTheAddress() throws {
        let detail = try #require(ContactsFixtures.buildMobileState(.c8, loc: loc).detail)
        let form = try #require(detail.form)
        #expect(form.title == loc.t("contacts.editTitle"))
        #expect(!form.name.isEmpty)
        #expect(form.address.hasPrefix("0x"))
        #expect(form.saveEnabled)
        #expect(form.addressLocked)
    }

    /// C9: the star is lit, and the core's reading of the address is drawn
    /// beside the name — two neutral tags, never a warning.
    @Test func theFavouriteDetailLightsTheStar() throws {
        let detail = try #require(ContactsFixtures.buildMobileState(.c9, loc: loc).detail)
        let favourite = try #require(detail.favourite)
        #expect(favourite.on)
        #expect(favourite.label == loc.t("contacts.sectionFavorites"))
        #expect(detail.inspection?.tag == loc.t("componentsUi.signing.walletTag"))
        #expect(detail.inspection?.firstTime == loc.t("componentsUi.signing.firstTimeTagNeutral"))
        // C2 is the same contact without either — the star and the tags are
        // the whole difference between the two states.
        let plain = try #require(ContactsFixtures.buildMobileState(.c2, loc: loc).detail)
        #expect(plain.favourite == nil)
        #expect(plain.inspection == nil)
    }

    // MARK: - Canon addresses (byte-exact — identicon seeds)

    @Test func rosterIsTheCanonicalEight() {
        #expect(ContactsFixtures.roster.count == 8)
        let expected: [(String, String, String, String)] = [
            ("Alice", "0x9F3c…21aE", "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE", "A"),
            ("阿豪", "0x77Bd…4F02", "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02", "A"),
            ("Bartholomew Vanderbilt-Konstantinopoulos.eth", "0x31c9…E77a",
             "0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a", "B"),
            ("Bob · 泵泵", "0x44Aa…9C21", "0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21", "B"),
            ("Charlie", "0x5eF0…3a9C", "0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C", "C"),
            ("DAO 金库", "0xF00d…C0de", "0xF00dBaBe8712004343cD00926Ab004D6C042C0de", "D"),
            ("hold on", "0xCafe…F00d", "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d", "H"),
            ("妈妈", "0x88Ce…12aB", "0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB", "M"),
        ]
        for (index, want) in expected.enumerated() {
            let canon = ContactsFixtures.roster[index]
            #expect(canon.name == want.0)
            #expect(canon.addressDisplay == want.1)
            #expect(canon.addressFull == want.2)
            #expect(canon.section == want.3)
            // Seeds are full 42-char addresses, never lowercased at rest.
            #expect(canon.addressFull.count == 42)
        }
    }

    @Test func groupOnlyMemberIsPinned() {
        #expect(ContactsFixtures.cousin.name == "表弟")
        #expect(ContactsFixtures.cousin.addressDisplay == "0xA1c3…88dD")
        #expect(ContactsFixtures.cousin.addressFull == "0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD")
    }

    @Test func identiconBoardCoversTheCanonPlusPlaceholder() {
        let seeds = ContactsFixtures.identiconBoardSeeds
        #expect(seeds.count == 10)
        #expect(seeds[0] == ContactsFixtures.roster[0].addressFull)
        #expect(seeds[8] == ContactsFixtures.cousin.addressFull)
        #expect(seeds[9] == "")
    }

    @Test func aliceAddressBlockIsTheMocksTwoLines() {
        #expect(ContactsFixtures.aliceAddressLines == ["0x9F3cA71b04E82f5C55d9", "B21aE00734F8Dd8021aE"])
        #expect(ContactsFixtures.aliceAddressLines.joined() == ContactsFixtures.roster[0].addressFull)
    }

    // MARK: - C1 (default list)

    @Test func c1MatchesTheMock() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c1, loc: loc).home)
        #expect(model.title == "通讯录")
        #expect(model.search.placeholder == "搜索名字、ENS 或地址")
        #expect(model.search.query == nil)
        #expect(model.groupsHeader?.title == "分组")
        #expect(model.groupsHeader?.action == "管理")
        #expect(model.groups.map(\.name) == ["家人", "工作", "交易所"])
        #expect(model.groups.map(\.countLabel) == ["3 人", "5 人", "2 人"])
        #expect(model.contactsHeader?.title == "联系人")
        #expect(model.contactsHeader?.action == "8 位")
        #expect(model.sections.map(\.letter) == ["A", "B", "C", "D", "H", "M"])
        #expect(model.sections.flatMap(\.contacts).count == 8)
        #expect(model.sections[0].contacts.map(\.name) == ["Alice", "阿豪"])
        #expect(model.sections[5].contacts[0].addressDisplay == "0x88Ce…12aB")
        #expect(model.tabs.contacts == "通讯录")
        #expect(model.reveal == nil)
        #expect(model.sheet == nil)
        #expect(model.empty == nil)
        #expect(model.textScale == 1)
    }

    @Test func indexRailIsTheFullAlphabetPlusHash() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c1, loc: loc).home)
        #expect(model.indexLetters.count == 27)
        #expect(model.indexLetters.first == "A")
        #expect(model.indexLetters[25] == "Z")
        #expect(model.indexLetters.last == "#")
        // Letters with no section still render (spec edge case).
        #expect(model.indexLetters.contains("E"))
        #expect(!model.sections.map(\.letter).contains("E"))
    }

    @Test func c1sRevealsTheSwipeActions() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c1s, loc: loc).home)
        let reveal = try #require(model.reveal)
        #expect(reveal.sendLabel == "转账")
        #expect(reveal.deleteLabel == "删除")
        // 阿豪 — the second row of section A (data-model.md C1s).
        #expect(model.sections[0].contacts[1].name == "阿豪")
        #expect(reveal.contactId == model.sections[0].contacts[1].id)
    }

    @Test func c1fIsPreFilteredBySearch() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c1f, loc: loc).home)
        #expect(model.search.query == "Ali")
        #expect(model.sections.map(\.letter) == ["A"])
        #expect(model.sections[0].contacts.map(\.name) == ["Alice"])
        #expect(model.contactsHeader?.action == "1 位")
    }

    @Test func swipeDeleteAlwaysHasAConfirm() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c1s, loc: loc).home)
        let contact = model.sections[0].contacts[1]
        let confirm = try #require(model.deleteConfirms[contact.id])
        #expect(confirm.title == "删除联系人？")
        #expect(confirm.body == "阿豪 将从通讯录中移除。")
        #expect(confirm.items[0].destructive)
    }

    // MARK: - C2 (contact detail)

    @Test func c2MatchesTheMock() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c2, loc: loc).detail)
        #expect(model.contact.name == "Alice")
        #expect(model.contact.addressDisplay == "0x9F3c…21aE")
        #expect(model.contact.addressFull == "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE")
        #expect(model.chips == ["家人"])
        #expect(model.addChip == "分组")
        #expect(model.actions.send == "转账")
        #expect(model.actions.receive == "收款")
        #expect(model.actions.qr == "二维码")
        #expect(model.addressLabel == "地址")
        #expect(model.addressLines == ContactsFixtures.aliceAddressLines)
        #expect(model.activityTitle == "最近往来")
        #expect(model.activityAction == "全部")
        #expect(model.deleteLabel == "删除联系人")
        #expect(model.sheet == nil)
    }

    @Test func c2ActivityMirrorsTheWalletStory() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c2, loc: loc).detail)
        #expect(model.activity.count == 2)
        #expect(model.activity[0].title == "已收到")
        #expect(model.activity[0].subtitle == "昨天 20:15 · Ethereum")
        #expect(model.activity[0].amount == "+50")
        #expect(model.activity[0].unit == "USDC")
        #expect(model.activity[0].positive)
        #expect(model.activity[1].title == "已发送")
        #expect(model.activity[1].subtitle == "8 月 5 日 · Arbitrum")
        #expect(model.activity[1].amount == "−0.2")
        #expect(model.activity[1].unit == "ETH")
        #expect(!model.activity[1].positive)
    }

    @Test func c2sRaisesTheDestructiveConfirm() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c2s, loc: loc).detail)
        let sheet = try #require(model.sheet)
        #expect(sheet.title == "删除联系人？")
        #expect(sheet.body == "Alice 将从通讯录中移除。")
        #expect(sheet.items.count == 1)
        #expect(sheet.items[0].label == "删除")
        #expect(sheet.items[0].destructive)
        #expect(sheet.cancel == "取消")
    }

    // MARK: - C3 (empty)

    @Test func c3MatchesTheMock() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c3, loc: loc).home)
        let empty = try #require(model.empty)
        #expect(empty.title == "还没有联系人")
        #expect(empty.caption == "添加常用地址，转账时不再反复粘贴。也可以从文件导入现有通讯录。")
        #expect(empty.primary == "添加联系人")
        #expect(empty.secondary == "从文件导入")
        #expect(model.groups.isEmpty)
        #expect(model.sections.isEmpty)
        // The header and search field stay (spec acceptance §4).
        #expect(model.title == "通讯录")
        #expect(model.search.placeholder == "搜索名字、ENS 或地址")
    }

    // MARK: - C4 / C6 (group detail)

    @Test func c4MatchesTheMock() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c4, loc: loc).group)
        #expect(model.name == "家人")
        #expect(model.membersLabel == "3 位成员")
        #expect(model.members.map(\.name) == ["妈妈", "表弟", "Alice"])
        #expect(model.members.map(\.addressDisplay) == ["0x88Ce…12aB", "0xA1c3…88dD", "0x9F3c…21aE"])
        #expect(model.addMemberLabel == "添加成员")
        #expect(model.ctaLabel == "群发转账")
        #expect(model.ctaCaption == "向本组 3 人转账，金额可分别设置。")
        #expect(model.ctaEnabled)
        #expect(model.sheet == nil)
    }

    @Test func emptyGroupDisablesTheCta() {
        let model = ContactsFixtures.emptyGroup(loc: loc)
        #expect(model.members.isEmpty)
        #expect(model.membersLabel == "0 位成员")
        #expect(!model.ctaEnabled)
    }

    // MARK: - Menus

    @Test func c5AddMenuMatchesTheMock() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c5, loc: loc).home)
        let sheet = try #require(model.sheet)
        #expect(sheet.items.map(\.label) == ["新建联系人", "从文件导入", "导出通讯录"])
        #expect(sheet.items.map(\.icon) == [.userRoundPlus, .download, .upload])
        #expect(!sheet.items.contains { $0.destructive })
        #expect(!sheet.items.contains { $0.dividerAbove })
        #expect(sheet.cancel == "取消")
        #expect(sheet.title == nil)
        // The dimmed list underneath is still the full C1 content.
        #expect(model.sections.flatMap(\.contacts).count == 8)
    }

    @Test func c6GroupMenuMatchesTheMock() throws {
        let model = try #require(ContactsFixtures.buildMobileState(.c6, loc: loc).group)
        let sheet = try #require(model.sheet)
        #expect(sheet.items.map(\.label) == ["编辑分组", "导入到本组", "导出本组", "删除分组"])
        #expect(sheet.items.map(\.icon) == [.pencil, .download, .upload, .trash2])
        #expect(sheet.items[3].destructive)
        #expect(sheet.items[3].dividerAbove)
        #expect(sheet.cancel == "取消")
    }

    // MARK: - Edge-case treatments (recorded in results notes)

    @Test func searchEmptyReusesTheEmptyTreatment() {
        let model = ContactsFixtures.searchEmpty(loc: loc, query: "zzz")
        #expect(model.title == "没有匹配「zzz」的结果")
    }

    @Test func noActivityReusesTheWalletEmptyState() {
        let model = ContactsFixtures.activityEmpty(loc: loc)
        #expect(!model.title.isEmpty)
        #expect(!model.title.contains("."))
    }

    // MARK: - Localization (US4)

    @Test func englishLocaleResolvesEveryString() throws {
        let en = Loc(overrideTag: "en", preferredLanguages: [])
        let home = try #require(ContactsFixtures.buildMobileState(.c1, loc: en).home)
        let detail = try #require(ContactsFixtures.buildMobileState(.c2, loc: en).detail)
        let group = try #require(ContactsFixtures.buildMobileState(.c4, loc: en).group)
        let strings = [
            home.title, home.addLabel, home.search.placeholder,
            home.groupsHeader?.title, home.groupsHeader?.action,
            home.contactsHeader?.title, home.contactsHeader?.action,
            home.tabs.wallet, home.tabs.contacts, home.tabs.explore, home.tabs.settings,
            detail.addressLabel, detail.activityTitle, detail.activityAction,
            detail.deleteLabel, detail.actions.qr, detail.copyLabel,
            group.addMemberLabel, group.ctaLabel, group.ctaCaption, group.membersLabel,
        ].compactMap { $0 }
        for s in strings {
            // No key leaks: resolved strings never look like corpus keys.
            #expect(!s.contains("contacts."))
            #expect(!s.contains("componentsUi."))
            #expect(!s.isEmpty)
        }
    }
}
