//
//  ContactsLiveTests.swift
//  VelaWalletTests
//
//  The live builder's two presentation judgements, and its refusals.
//
//  Everything else this file could test belongs to `vela-core` and is tested
//  there: which contacts exist, what order they come in, whether one is
//  favourited, which are tombstoned. What is genuinely the shell's is the A–Z
//  directory, the search narrowing, and the three states that must never be
//  confused with each other — waiting, empty, and no-search-results.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ContactsLiveTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func contact(
        _ address: String,
        name: String? = nil,
        resolved: String? = nil
    ) -> ContactWire {
        ContactWire(
            address: address, name: name, resolvedName: resolved, resolvedSource: nil,
            kind: .unknown, favorite: false, note: nil, txCount: 0,
            lastUsedMs: 0, firstSeenMs: 0, source: .manual
        )
    }

    private func view(
        _ contacts: [ContactWire],
        groups: [ContactGroupWire] = [],
        loaded: Bool = true
    ) -> ContactsViewWire {
        ContactsViewWire(
            loaded: loaded, contacts: contacts, groups: groups,
            lastImport: nil, recipient: nil
        )
    }

    // MARK: - The three states that must not be confused

    /// Waiting is not empty. "你还没有联系人" is a claim about a book that has
    /// not been read yet — telling somebody their address book is gone.
    @Test func waitingShowsChromeWithNoClaimAboutTheBook() {
        let model = ContactsLive.waiting(loc: loc)
        #expect(model.empty == nil)
        #expect(model.searchEmpty == nil)
        #expect(model.sections.isEmpty)
        #expect(model.contactsHeader == nil)
        // The tab bar is still there, so the screen is not a place to be stuck.
        #expect(!model.tabs.wallet.isEmpty)
    }

    @Test func anEmptyBookGetsTheDrawnEmptyState() {
        let model = ContactsLive.home(view([]), loc: loc)
        #expect(model.empty != nil)
        #expect(model.searchEmpty == nil)
        #expect(model.sections.isEmpty)
    }

    @Test func aSearchWithNoMatchesIsNotAnEmptyBook() {
        let model = ContactsLive.home(view([contact("0xabc", name: "Alice")]),
                                      loc: loc, query: "zzzz")
        #expect(model.searchEmpty != nil)
        #expect(model.empty == nil, "an empty RESULT must not read as an empty book")
    }

    // MARK: - Judgement 1: A–Z sectioning over the core's order

    /// The core's order is authoritative INSIDE a letter — the directory is a
    /// way to find somebody, and within one letter the person you use most
    /// should still be first.
    @Test func sectioningKeepsTheCoresOrderWithinALetter() {
        let model = ContactsLive.home(
            view([
                contact("0x1", name: "Amber"),
                contact("0x2", name: "Bob"),
                contact("0x3", name: "Alice"),
            ]),
            loc: loc
        )
        #expect(model.sections.map(\.letter) == ["A", "B"])
        #expect(model.sections[0].contacts.map(\.name) == ["Amber", "Alice"])
    }

    /// CJK names file under their pinyin initial, exactly as the drawing does.
    ///
    /// `ContactsFixtures` is the canon and it is unambiguous: 阿豪 sits in
    /// section A and 妈妈 in section M. The straight `("A"..."Z")` rule sends
    /// every Chinese name to one bucket at the bottom of the rail — an A–Z
    /// directory with the directory taken out.
    @Test func cjkNamesFileUnderTheirPinyinInitialLikeTheDrawing() {
        let model = ContactsLive.home(
            view([
                contact("0x1", name: "妈妈"),
                contact("0x2", name: "阿豪"),
                contact("0x3", name: "Bob"),
            ]),
            loc: loc
        )
        #expect(model.sections.map(\.letter) == ["A", "B", "M"])
        #expect(model.sections.first?.contacts.map(\.name) == ["阿豪"])
        #expect(model.sections.last?.contacts.map(\.name) == ["妈妈"])
    }

    /// The fixture roster's own sectioning is the acceptance test for the rule:
    /// if the live builder disagrees with the drawing about where a canon name
    /// files, one of them is wrong.
    @Test func theLiveRuleAgreesWithTheDrawnRosterSections() {
        for canon in ContactsFixtures.roster {
            let letter = ContactsLive.home(view([contact("0x1", name: canon.name)]), loc: loc)
                .sections.first?.letter
            #expect(letter == canon.section, "\(canon.name) files under \(letter ?? "?"), drawn as \(canon.section)")
        }
    }

    /// What iOS's transliteration covers **beyond** Chinese.
    ///
    /// Recorded as a regression guard ahead of the core taking sectioning over
    /// (028's `ContactsView.sections`, landed 2026-09-05 on an unmerged
    /// branch). Its table is U+4E00–U+9FFF plus Latin-1/Extended-A, while
    /// `.toLatin` is ICU and romanises every script it knows — so these names
    /// file under a letter today and would fall to `#` under a CJK-only table.
    ///
    /// ja and ko are shipped locales, so this is a real population, not a
    /// curiosity. If the switch lands with the narrower table, THIS is the test
    /// that should fail.
    @Test func transliterationAlsoFilesKanaHangulAndCyrillic() {
        let cases: [(String, String)] = [
            ("さくら", "S"),   // kana   — sakura
            ("김민준", "G"),    // hangul — gim
            ("Ελένη", "E"),    // greek  — eléni
            ("Дмитрий", "D"),  // cyrillic
        ]
        for (name, letter) in cases {
            let section = ContactsLive.home(view([contact("0x1", name: name)]), loc: loc)
                .sections.first?.letter
            #expect(section == letter, "\(name) filed under \(section ?? "?"), expected \(letter)")
        }
    }

    /// Digits, emoji and an unnamed address still land under `#`, which sorts
    /// last — where the drawn rail puts it.
    @Test func thingsWithNoLatinInitialCollectUnderHashWhichSortsLast() {
        let model = ContactsLive.home(
            view([
                contact("0x1", name: "7-Eleven"),
                contact("0x2", name: "Bob"),
                contact("0x3", name: "🎩"),
            ]),
            loc: loc
        )
        #expect(model.sections.map(\.letter) == ["B", "#"])
        #expect(model.sections.last?.contacts.count == 2)
    }

    /// The rail is always the full alphabet, whatever the book contains.
    @Test func theRailIsAlwaysFull() {
        let model = ContactsLive.home(view([contact("0x1", name: "Bob")]), loc: loc)
        #expect(model.indexLetters.count == 27)
        #expect(model.indexLetters.last == "#")
    }

    // MARK: - Judgement 2: search narrows, never re-orders

    @Test func searchMatchesNameResolvedNameAndAddress() {
        let book = view([
            contact("0xdeadbeef", name: "Alice"),
            contact("0xcafe", resolved: "bob.eth"),
            contact("0xfeed", name: "Carol"),
        ])
        #expect(ContactsLive.home(book, loc: loc, query: "ali").sections
            .flatMap(\.contacts).map(\.name) == ["Alice"])
        #expect(ContactsLive.home(book, loc: loc, query: "BOB.ETH").sections
            .flatMap(\.contacts).count == 1)
        #expect(ContactsLive.home(book, loc: loc, query: "0xfeed").sections
            .flatMap(\.contacts).map(\.name) == ["Carol"])
    }

    @Test func aBlankQueryIsNotASearch() {
        let model = ContactsLive.home(view([contact("0x1", name: "Alice")]),
                                      loc: loc, query: "   ")
        #expect(model.searchEmpty == nil)
        #expect(model.sections.flatMap(\.contacts).count == 1)
    }

    // MARK: - Display

    /// Their own name wins; then a resolved one; then the address introduces
    /// itself. A blank row is never an option.
    @Test func displayNameFallsBackWithoutEverBeingBlank() {
        #expect(ContactsLive.displayName(contact("0x1", name: "Alice", resolved: "a.eth")) == "Alice")
        #expect(ContactsLive.displayName(contact("0x1", resolved: "a.eth")) == "a.eth")
        let bare = ContactsLive.displayName(contact("0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE"))
        #expect(bare == "0x9F3c…21aE")
    }

    /// The identicon seed is the FULL address, never lowercased at the call
    /// site — vela-core normalises it, and pre-lowercasing draws a different
    /// picture for the same person (spec 003 rule).
    @Test func theIdenticonSeedIsTheFullAddress() {
        let address = "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE"
        let row = ContactsLive.home(view([contact(address, name: "Alice")]), loc: loc)
            .sections.first?.contacts.first
        #expect(row?.addressFull == address)
    }

    /// Group membership on a row comes from the core's resolved groups, so a
    /// row and its group page cannot disagree about who is in what.
    @Test func groupMembershipComesFromTheCore() {
        let alice = contact("0xabc", name: "Alice")
        let model = ContactsLive.home(
            view([alice], groups: [ContactGroupWire(id: "grp_1", name: "家人",
                                                    color: nil, members: [alice])]),
            loc: loc
        )
        #expect(model.sections.first?.contacts.first?.groups == ["家人"])
        #expect(model.groups.count == 1)
    }

    // MARK: - Detail

    /// There is no transaction store until spec 052, so a contact's page shows
    /// the drawn "no activity" treatment rather than a fixture's two rows under
    /// a real person's name.
    @Test func detailShowsNoActivityRatherThanFixtureActivity() {
        let alice = contact("0xabc", name: "Alice")
        let model = ContactsLive.detail(alice, view: view([alice]), loc: loc)
        #expect(model.activity.isEmpty)
        #expect(model.activityEmpty != nil)
        #expect(model.contact.name == "Alice")
    }

    /// An empty group cannot start a batch send — the drawn CTA is disabled.
    @Test func anEmptyGroupDisablesItsBatchSend() {
        let group = ContactGroupWire(id: "grp_1", name: "家人", color: nil, members: [])
        let model = ContactsLive.group(group, view: view([]), loc: loc)
        #expect(!model.ctaEnabled)
    }
}
