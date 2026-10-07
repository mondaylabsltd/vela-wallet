//
//  MultiPickSearchTests.swift
//  VelaWalletTests
//
//  Issue #445: on iOS a group's Add member listed all 59 contacts with no way
//  to search them, while Android's sheet (#437) could be searched. Past six
//  rows the sheet now has the box, matching name and address.
//

import Foundation
import Testing
@testable import VelaWallet

struct MultiPickSearchTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func book(_ count: Int) -> MultiPickModel {
        MultiPickModel(
            title: "Add member",
            rows: (1...max(count, 1)).prefix(count).map { i in
                let address = "0x" + String(repeating: "0", count: 38) + String(format: "%02x", i)
                return MultiPickRowModel(
                    id: address, title: "Person \(i)", subtitle: "0x0000…00\(i)",
                    identiconSeed: address, picked: i == 1
                )
            },
            emptyText: "", save: "Save", cancel: "Cancel",
            search: ContactsLive.pickSearch(loc: loc, rows: count)
        )
    }

    @Test func aLongListGetsTheBoxAShortOneDoesNot() {
        #expect(ContactsLive.pickSearch(loc: loc, rows: 6) == nil)
        #expect(ContactsLive.pickSearch(loc: loc, rows: 7) != nil)
        #expect(book(59).search?.placeholder == loc.t("contacts.searchPlaceholder"))
    }

    @Test func theQueryNarrowsByNameAndByAddress() {
        let model = book(59)
        #expect(model.rows(matching: "").count == 59)
        #expect(model.rows(matching: "  person 42 ").map(\.title) == ["Person 42"])
        // The full address, which the row's subtitle shortens.
        #expect(model.rows(matching: "2a").map(\.title) == ["Person 42"])
        #expect(model.rows(matching: "nobody").isEmpty)
        #expect(model.search?.noMatch("nobody") == loc.t("contacts.noResults", vars: ["query": "nobody"]))
    }

    @Test func searchingHidesRowsWithoutUntickingThem() {
        let model = book(59)
        // Person 1 is ticked; a query that hides them leaves the tick in the model.
        #expect(model.rows(matching: "person 42").allSatisfy { !$0.picked })
        #expect(model.rows.first?.picked == true)
    }

    @Test func aShortListIsNeverFiltered() {
        let model = book(3)
        #expect(model.search == nil)
        #expect(model.rows(matching: "person 2").count == 3)
    }
}
