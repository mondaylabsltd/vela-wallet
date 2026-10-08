//
//  ContactPickerTests.swift
//  VelaWalletTests
//
//  Issue #467: on iOS the person icon beside the recipient opened nothing.
//  It pushed SD2e onto the shell's stack, but the live router draws the send
//  journey from the CORE's state, and nothing raised the core's
//  `show_contact_picker` — `open_contact_picker` had no caller. Now the icon
//  dispatches it, a pick is BY ADDRESS, and closing the sheet lowers the flag
//  so the icon works the next time too.
//
//  Driven through the real `SendStore`, its real executor and the real `send`
//  core, the way `SendBackTests` drives them.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(3)))
struct ContactPickerTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private let bob = "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    // MARK: - Fixtures

    private func balance() throws -> BalanceViewWire {
        let json = """
        {
          "address": "\(me)",
          "display_total_usd": 0.5, "balance_unknown": false, "balance_partial": false,
          "notice": null, "hidden": false, "refreshing": false,
          "last_refreshed_at_ms": 1,
          "tokens": [{
            "chain_id": 100, "symbol": "xDAI", "name": "xDAI",
            "balance": "0.5", "decimals": 18, "token_address": null,
            "price_usd": 1.0, "spam": false
          }],
          "unpriced_tokens": [],
          "failed_chain_ids": [], "rate_limited_chain_ids": [], "unreachable_networks": [],
          "holdings_loading": false, "cached_total_usd": null,
          "switcher": { "open": false, "loading": false, "balances": [] }
        }
        """
        return try CoreJSON.decode(BalanceViewWire.self, from: CoreJSON.object(json))
    }

    private func networks() throws -> NetViewWire {
        let json = """
        {
          "loaded": true, "last_added_chain_id": null, "endpoints": [], "providers": [],
          "networks": [{
            "id": "chain-100", "chain_id": 100, "display_name": "Gnosis",
            "native_symbol": "xDAI", "is_custom": false,
            "rpc_url": "", "explorer_url": "", "bundler_url": "",
            "rpc_health": null, "explorer_health": null, "rpc_chain_mismatch": null,
            "rpc_save_deferred": false
          }],
          "wizard": {
            "phase": "idle", "query": "", "custom_rpc": "", "suggestions": [],
            "chain_info": null, "compat": null, "error": null, "can_add": false
          }
        }
        """
        return try CoreJSON.decode(NetViewWire.self, from: CoreJSON.object(json))
    }

    private func store() throws -> SendStore {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        let accountPort = ScriptedAccounts()
        let held = try balance()
        let nets = try networks()
        return SendStore(executor: SendExecutor(
            store: store, relay: relay, pool: pool,
            spine: UserOpSpine(relay: relay, accounts: accountPort, signer: { CountingSigner() }),
            accounts: accountPort,
            fees: FeeStore(relay: relay, accounts: accountPort, settleDeadline: nil, timers: .stopped),
            identity: RecipientIdentity(store: store, pool: pool, accounts: accounts),
            metadata: TokenMetadata(store: store, pool: pool),
            accountStore: accounts,
            balances: { held }, networks: { nets },
            holdingsRound: { _ in 0 }, openHoldings: { _ in },
            ports: SendExecutor.Ports()
        ))
    }

    /// 转账, then the xDAI row: the single form.
    private func form() async throws -> SendStore {
        let send = try store()
        send.open(
            accountId: "cred-0", address: me, name: nil,
            displayCode: "USD", displayRate: 1, fiatDecimals: 2
        )
        await Wait.until({ !(send.view?.tokens.isEmpty ?? true) }, orIdle: { send.isIdle })
        let token = try #require(send.view?.tokens.first?.id)
        send.selectToken(id: token)
        await Wait.until({ send.view?.stage == .enterDetails }, orIdle: { send.isIdle })
        return send
    }

    private func drawn(_ send: SendStore) -> FlowStateId? {
        send.view.map { SendLive.flowState($0, feeSheetOpen: false) }
    }

    private func same(_ a: String?, _ b: String) -> Bool {
        a?.caseInsensitiveCompare(b) == .orderedSame
    }

    // MARK: - The icon, the pick, the close

    /// The icon raises the CORE's picker — the only thing the live router
    /// draws SD2e from — and a pick by address fills the recipient and
    /// lowers it.
    @Test func theIconOpensThePickerAndAPickFillsTheRecipient() async throws {
        let send = try await form()
        #expect(drawn(send) == .sd2)

        send.openContactPicker(target: nil)
        #expect(send.view?.showContactPicker == true)
        #expect(drawn(send) == .sd2e, "the icon opened nothing the router draws")

        send.pickedAddress(alice)
        #expect(send.view?.showContactPicker == false, "the core closes the picker on a pick")
        #expect(drawn(send) == .sd2)
        #expect(same(send.view?.recipient, alice))
    }

    /// × (or a drag) lowers the flag, so the icon opens the picker AGAIN —
    /// left up, the sheet stayed hidden and the icon was dead from then on.
    @Test func aClosedPickerOpensAgain() async throws {
        let send = try await form()
        send.openContactPicker(target: nil)
        #expect(drawn(send) == .sd2e)
        send.closeContactPicker()
        #expect(drawn(send) == .sd2)
        send.openContactPicker(target: nil)
        #expect(drawn(send) == .sd2e, "the second tap on the icon opened nothing")
        send.closeContactPicker()

        // The importer is the same shape of sheet: its close lowers its flag.
        send.openBatchImport()
        if send.view?.showBatchImport == true {
            send.closeBatchImport()
            #expect(send.view?.showBatchImport == false)
        }
    }

    // MARK: - Rows

    private func person(_ address: String, _ name: String?) -> ContactWire {
        ContactWire(
            address: address, name: name, resolvedName: nil, resolvedSource: nil,
            kind: .unknown, favorite: false, note: nil, txCount: 0,
            lastUsedMs: 0, firstSeenMs: 0, source: .manual
        )
    }

    private func book(_ people: [(String, String?)]) throws -> ContactsViewWire {
        ContactsViewWire(
            loaded: true, contacts: people.map { person($0.0, $0.1) }, sections: [],
            groups: [ContactGroupWire(id: "g-payroll", name: "Payroll", color: nil,
                                      members: [person(bob, "Bob")])],
            lastImport: nil, importFailure: nil, export: nil, recipient: nil
        )
    }

    private var drawnSheet: ContactPickModel? {
        if case .contactPick(let sheet)? = WalletFlowFixtures.build(.sd2e, loc: loc).sheet { sheet } else { nil }
    }

    /// A row hands back the ADDRESS it was drawn for, and keeps its identity
    /// across builds — a re-sort of the book never makes a tap name the
    /// neighbour, and a render never gives every row a new identity.
    @Test func rowsAreKeyedByWhoTheyAre() throws {
        let drawn = try #require(drawnSheet)
        let first = SendLive.contactSheet(try book([(alice, "Alice"), (bob, nil)]), on: drawn, loc: loc)
        let again = SendLive.contactSheet(try book([(alice, "Alice"), (bob, nil)]), on: drawn, loc: loc)
        #expect(first.contacts.map(\.id) == again.contacts.map(\.id), "a new identity per render")
        #expect(first.contacts.map(\.address) == [alice, bob])
        #expect(first.contacts.map(\.id) == [alice, bob])
        #expect(first.groups.map(\.id) == ["g-payroll"])
        #expect(first.groups.map(\.id) == again.groups.map(\.id))

        // Re-sorted, Bob is first — and Bob's row still names Bob.
        let resorted = SendLive.contactSheet(try book([(bob, nil), (alice, "Alice")]), on: drawn, loc: loc)
        #expect(resorted.contacts.first?.address == bob)
        #expect(resorted.contacts.first { $0.name == "Alice" }?.address == alice)
    }

    /// While the book is still being read the picker is its chrome and nobody
    /// — never the drawing's Alice, 阿豪 and "hold on".
    @Test func aBookStillReadingShowsNobody() throws {
        let drawn = try #require(drawnSheet)
        #expect(!drawn.contacts.isEmpty, "the drawing this used to fall back to")
        let reading = SendLive.contactSheetReading(on: drawn)
        #expect(reading.contacts.isEmpty)
        #expect(reading.groups.isEmpty)
        #expect(reading.title == drawn.title)
        #expect(reading.scanRow == drawn.scanRow, "the scan row stays: an address can still be scanned")
    }

    /// The stack loses a level only for a state that IS a sheet: the
    /// picker's scan row pushes the scanner, which takes the sheet away — a
    /// pop then would close the camera just opened.
    @Test func theScannerIsNotASheetThePickerIs() {
        #expect(WalletFlowFixtures.build(.s1, loc: loc).sheet == nil)
        #expect(WalletFlowFixtures.build(.sd2e, loc: loc).sheet != nil)
        #expect(WalletFlowFixtures.build(.sd2c, loc: loc).sheet != nil)
    }
}
