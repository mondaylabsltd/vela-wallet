//
//  SendPayeeTests.swift
//  VelaWalletTests
//
//  Spec 097 F, the real-money pass's two Send findings, on this client:
//
//  - S2: the confirm's To row said "Wallet" — a name from the public passkey
//    registry, where anyone can register any name — with the address only
//    behind a tap on the identicon. The core now says who is paid
//    (`SendView.payees`): the address always, a name only beside it, and
//    whose word that name is. The shell draws it; it decides none of it.
//  - S3: a two-coin sweep's success screen said "Send ETH | Sent 0.000418
//    ETH" although 0.034929 USDC moved in the same operation. The receipt now
//    lists every coin (`SendReceiptView.coins`).
//
//  Views are the core's own, patched — no field is invented — and one run
//  drives the real core from a typed address to the confirm's To row.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SendPayeeTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// The developer wallet of the pass, which the registry calls "Wallet".
    private static let devWallet = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
    private static let alice = "0x1111111111111111111111111111111111111111"
    private static let bob = "0x2222222222222222222222222222222222222222"
    private static let baseUsdc = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"

    private let xdai: [String: Any] = [
        "symbol": "xDAI", "network": "Gnosis", "chain_id": 100,
        "token_address": NSNull(), "decimals": 18, "balance": "10",
        "price_usd": 1.0, "logo_urls": [], "spam": false,
    ]

    private func payee(_ address: String, _ name: String?, _ source: [String: Any]?) -> [String: Any] {
        ["address": address, "name": name ?? NSNull(), "name_source": source ?? NSNull()]
    }

    private let registry: [String: Any] = ["type": "registry"]

    private func sendView(_ patch: [String: Any]) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    private func confirm(_ view: SendViewWire, on state: FlowStateId = .sd3) throws -> SendConfirmModel {
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(state, loc: loc).base else {
            throw Missing()
        }
        return SendLive.confirm(
            view, from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: nil),
            display: .usd, on: drawn, loc: loc
        )
    }

    private func toRow(_ model: SendConfirmModel) throws -> FactRowModel {
        try #require(model.facts.first { $0.label == loc.t("send.toLabel") })
    }

    private struct Missing: Error {}

    private func expectFace(_ row: FactRowModel, _ address: String) {
        if case .identicon(let seed)? = row.lead {
            #expect(seed == address, "the face is the full address's — a tap shows it whole")
        } else {
            Issue.record("the To row lost its identicon")
        }
    }

    // MARK: - S2: who is paid

    @Test func aRegistryNameIsTaggedAndTheAddressStaysOnTheConfirm() throws {
        let view = try sendView([
            "stage": "confirm", "selected_token": xdai, "recipient": Self.devWallet,
            "confirm_amount": "1",
            "recipient_identity": ["name": "Wallet", "source": "passkey"],
            "payees": [payee(Self.devWallet, "Wallet", registry)],
        ])
        let row = try toRow(try confirm(view))
        #expect(row.value == "Wallet", "the name alone — it may be cut")
        #expect(row.detail == "Vela User · 0x14fB…eA5c", "whose word it is, and the address: never cut")
        #expect(!row.mono)
        expectFace(row, Self.devWallet)
    }

    /// Issue #423: on the Xiaomi "Wallet" over "Vela User · 0x14fb…ea5c" was
    /// read as an account type over the person's name. "Wallet" IS the name
    /// the registry holds for that address; "Vela User" — or a service's own
    /// label — is only whose word it is. The first line, beside the face, is
    /// the name; the kind is only ever on the line under it. The gallery's
    /// confirm draws the same case.
    @Test func theToRowsFirstLineIsTheNameNeverTheWordForWhoseNameItIs() throws {
        let kinds: [([String: Any], String)] = [
            (registry, loc.t("send.velaUser")),
            (["type": "service", "label": "ENS"], "ENS"),
        ]
        for (source, kind) in kinds {
            let view = try sendView([
                "stage": "confirm", "selected_token": xdai, "recipient": Self.devWallet,
                "confirm_amount": "0.8",
                "payees": [payee(Self.devWallet, "Wallet", source)],
            ])
            let row = try toRow(try confirm(view))
            #expect(row.value == "Wallet", "the name is the first line")
            #expect(row.value != kind)
            #expect(row.detail == "\(kind) · 0x14fB…eA5c")
            #expect(!row.mono)
            expectFace(row, Self.devWallet)
        }

        let gallery = WalletFlowFixtures.toFact(.single, loc: loc)
        #expect(gallery.value == "Alice")
        #expect(gallery.detail == "\(loc.t("send.velaUser")) · 0x9F3c…21aE")
        #expect(!gallery.mono)
        if case .identicon? = gallery.lead {} else { Issue.record("the gallery's To row lost its face") }
    }

    /// The core allows a registry name of 64 characters. Cut with the name,
    /// a tag on the name's line would vanish exactly when someone made the
    /// name long on purpose; it rides with the address instead.
    @Test func aLongRegistryNameCannotPushTheTagOutOfSight() throws {
        let long = String(repeating: "W", count: 64)
        let view = try sendView([
            "stage": "confirm", "selected_token": xdai, "recipient": Self.devWallet,
            "payees": [payee(Self.devWallet, long, registry)],
        ])
        let row = try toRow(try confirm(view))
        #expect(row.value == long)
        #expect(row.detail == "Vela User · 0x14fB…eA5c")
    }

    @Test func anOwnNameIsUntaggedAndStillCarriesTheAddress() throws {
        let view = try sendView([
            "stage": "confirm", "selected_token": xdai, "recipient": Self.alice,
            "confirm_amount": "1",
            "payees": [payee(Self.alice, "Savings", ["type": "own"])],
        ])
        let row = try toRow(try confirm(view))
        #expect(row.value == "Savings")
        #expect(row.detail == AddressText.short(Self.alice), "no tag: the person's own word")
        expectFace(row, Self.alice)
    }

    @Test func aNameServiceSaysWhichAndNobodyNamedIsTheAddressInMono() throws {
        let ens = try toRow(try confirm(try sendView([
            "stage": "confirm", "selected_token": xdai, "recipient": Self.alice,
            "payees": [payee(Self.alice, "bob.eth", ["type": "service", "label": "ENS"])],
        ])))
        #expect(ens.value == "bob.eth")
        #expect(ens.detail == "ENS · \(AddressText.short(Self.alice))")

        let unnamed = try toRow(try confirm(try sendView([
            "stage": "confirm", "selected_token": xdai, "recipient": Self.alice,
            // The resolver's name is not the core's say-so: it is not drawn.
            "recipient_identity": ["name": "Alice", "source": NSNull()],
            "payees": [payee(Self.alice, nil, nil)],
        ])))
        #expect(unnamed.value == AddressText.short(Self.alice))
        #expect(unnamed.mono)
        #expect(unnamed.detail == nil)
    }

    /// A source this build has never heard of decodes, and its name is not
    /// drawn: untagged it would read as the person's own.
    @Test func aNameOfASourceThisBuildDoesNotKnowIsNotDrawn() throws {
        let view = try sendView([
            "stage": "confirm", "selected_token": xdai, "recipient": Self.alice,
            "payees": [payee(Self.alice, "Mallory", ["type": "a_future_source"])],
        ])
        #expect(view.payees?.first?.nameSource == .unknown)
        let row = try toRow(try confirm(view))
        #expect(row.value == AddressText.short(Self.alice))
        #expect(row.mono)
    }

    @Test func theFormLineSaysWhoseWordTheNameIsNeverTheResolversLabel() throws {
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            throw Missing()
        }
        let view = try sendView([
            "stage": "enter_details", "selected_token": xdai, "recipient": Self.devWallet,
            "recipient_identity": ["name": "Wallet", "source": "passkey"],
            "payees": [payee(Self.devWallet, "Wallet", registry)],
        ])
        let note = SendLive.form(view, fee: nil, display: .usd, on: drawn, loc: loc).recipient?.note
        #expect(note == "Wallet · Vela User")
        #expect(note?.contains("passkey") == false)
    }

    @Test func thePickersToLineIsTheConfirmsToRow() throws {
        let view = try sendView([
            "recipient": Self.devWallet,
            "payees": [payee(Self.devWallet, "Wallet", registry)],
        ])
        let line = try #require(SendLive.pickRecipient(view, loc: loc))
        #expect(line.label == loc.t("send.toLabel"))
        #expect(line.value == "Wallet")
        #expect(line.detail == "Vela User · 0x14fB…eA5c")
        expectFace(line, Self.devWallet)
    }

    /// The sweep's confirm had the same "To Wallet" row.
    @Test func aSweepConfirmNamesItsOnePayeeWithTheAddress() throws {
        let view = try sendView([
            "stage": "confirm", "multi_select_mode": true, "recipient": Self.devWallet,
            "multi_selected_ids": ["Gnosis_native_xDAI"], "multi_chain_id": 100, "tokens": [xdai],
            "payees": [payee(Self.devWallet, "Wallet", registry)],
        ])
        let row = try toRow(try confirm(view, on: .sd3c))
        #expect(row.value == "Wallet")
        #expect(row.detail == "Vela User · 0x14fB…eA5c")
    }

    @Test func aSplitConfirmRowNamesThePayeeOverItsAddress() throws {
        let view = try sendView([
            "stage": "confirm", "split_mode": true, "selected_token": xdai, "confirm_amount": "3.5",
            "recipients": [
                ["id": "r1", "address": Self.alice, "amount": "1.5", "name": "Alice"],
                ["id": "r2", "address": Self.bob, "amount": "2", "name": NSNull()],
            ],
            "payees": [payee(Self.alice, "Alice", ["type": "own"]), payee(Self.bob, nil, nil)],
        ])
        let model = try confirm(view, on: .sd3b)
        #expect(model.breakdown.map(\.label) == ["Alice", AddressText.short(Self.bob)])
        #expect(model.breakdown.map(\.detail) == [AddressText.short(Self.alice), nil])
        #expect(model.breakdown.map(\.mono) == [false, true])
        #expect(model.breakdown.map(\.value) == ["1.5 xDAI", "2 xDAI"])
        #expect(!model.facts.contains { $0.label == loc.t("send.toLabel") }, "no one To row on a split")
    }

    // MARK: - S2, through the real core

    /// The core answered as the app answers it: a typed whole address is
    /// looked up, the registry says "Wallet" (`source: passkey`), and the
    /// confirm's To row — built over the view that crossed the wire — tags
    /// the name and keeps the address.
    @Test func theRealCoresRegistryPayeeReachesTheConfirmTagged() throws {
        let core = SendCore()
        var latest: [String: Any] = [:]
        func run(_ event: [String: Any]) throws {
            var result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(event)))
            var queue = result["effects"] as? [[String: Any]] ?? []
            latest = result["view"] as? [String: Any] ?? latest
            while !queue.isEmpty {
                let effect = queue.removeFirst()
                guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                      let op = effect["operation"] as? [String: Any],
                      let reply = answer(op) else { continue }
                result = try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(reply)))
                latest = result["view"] as? [String: Any] ?? latest
                queue += result["effects"] as? [[String: Any]] ?? []
            }
        }
        // Only what shapes this view is answered; the rest stays in flight.
        func answer(_ op: [String: Any]) -> [String: Any]? {
            switch op["type"] as? String {
            case "fetch_tokens":
                return [
                    "type": "tokens_loaded",
                    "tokens": [[
                        "network": "chain-100", "chain_id": 100, "symbol": "xDAI", "balance": "2",
                        "decimals": 18, "token_address": NSNull(), "price_usd": 1.0,
                        "logo_urls": [String](), "spam": false,
                    ] as [String: Any]],
                    "chains": [["network": "chain-100", "chain_id": 100, "native_symbol": "xDAI"]],
                ]
            case "resolve_identity":
                return ["type": "identity_resolved", "identity": ["name": "Wallet", "source": "passkey"]]
            default:
                return nil
            }
        }

        try run([
            "type": "open",
            "account": ["id": "cred-0", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894", "name": NSNull()] as [String: Any],
            "params": [
                "preselected_symbol": NSNull(), "preselected_network": NSNull(),
                "prefilled_recipient": NSNull(), "prefilled_chain_id": NSNull(),
                "prefilled_token_address": NSNull(), "prefilled_amount_base": NSNull(),
                "locked": false, "preselected_multi": NSNull(),
            ] as [String: Any],
            "display": ["code": "USD", "rate": 1.0, "fiat_decimals": 2] as [String: Any],
        ])
        try run(["type": "select_token", "token_id": "chain-100_native_xDAI"])
        try run(["type": "set_recipient", "recipient": Self.devWallet])

        let view = try CoreJSON.decode(SendViewWire.self, from: latest)
        #expect(view.payees == [SendPayeeWire(address: Self.devWallet, name: "Wallet", nameSource: .registry)])
        let row = try toRow(try confirm(view))
        #expect(row.value == "Wallet")
        #expect(row.detail == "Vela User · 0x14fB…eA5c")
        expectFace(row, Self.devWallet)

        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            throw Missing()
        }
        let note = SendLive.form(view, fee: nil, display: .usd, on: drawn, loc: loc).recipient?.note
        #expect(note == "Wallet · Vela User", "never \"Wallet · passkey\"")
    }

    // MARK: - S3: every coin on the receipt

    private func receipt(_ patch: [String: Any]) throws -> SendReceiptModel {
        guard case .sendReceipt(let drawn) = WalletFlowFixtures.build(.sd4a, loc: loc).base else {
            throw Missing()
        }
        return SendLive.receipt(try sendView(patch), display: .usd, on: drawn, loc: loc)
    }

    private func coin(_ amount: String, _ symbol: String, _ token: String?, _ usd: Double) -> [String: Any] {
        ["amount": amount, "symbol": symbol, "logo_urls": [String](), "token_address": token ?? NSNull(), "usd_value": usd]
    }

    private func sweepReceipt(status: String) throws -> SendReceiptModel {
        let eth: [String: Any] = [
            "symbol": "ETH", "network": "Base", "chain_id": 8453, "token_address": NSNull(),
            "decimals": 18, "balance": "0.0005", "price_usd": 2500.0, "logo_urls": [], "spam": false,
        ]
        func leg(_ amount: String, _ symbol: String) -> [String: Any] {
            ["to": Self.devWallet, "to_name": "Wallet", "amount": amount, "symbol": symbol,
             "logo_urls": [String](), "usd_value": 0]
        }
        return try receipt([
            "stage": "receipt", "multi_select_mode": true, "multi_chain_id": 8453,
            "selected_token": eth, "recipient": Self.devWallet, "tx_status": "confirmed",
            "recipient_identity": ["name": "Wallet", "source": "passkey"],
            "payees": [payee(Self.devWallet, "Wallet", registry)],
            "receipt": [
                "status": status, "hold_reason": NSNull(), "kind": "multi_select",
                "transfers": [leg("0.000418", "ETH"), leg("0.034929", "USDC")],
                "coins": [coin("0.000418", "ETH", nil, 1.045), coin("0.034929", "USDC", Self.baseUsdc, 0.034929)],
                "amount": "", "usd_value": 1.079929,
                "submitted_at_ms": NSNull(), "typical_inclusion_s": NSNull(),
            ] as [String: Any],
        ])
    }

    @Test func aTwoCoinSweepReceiptListsEveryCoin() throws {
        let model = try sweepReceipt(status: "confirmed")
        #expect(model.header.title == "Send tokens", "not \"Send ETH\"")
        #expect(model.title == loc.t("componentsTx.detail.sent"))
        #expect(model.title != "Sent 0.000418 ETH", "one coin never stands for the batch")
        #expect(model.breakdownTitle == "2 assets")
        #expect(model.breakdown.map(\.label) == ["ETH", "USDC"])
        #expect(model.breakdown.map(\.value) == ["0.000418 ETH", "0.034929 USDC"])
        #expect(model.breakdown.allSatisfy { $0.lead != nil }, "each coin with its mark")
        // Its one recipient stays the caption — not the asset count.
        #expect(model.captions == ["Wallet · Base"])

        // Already while the relay has it.
        let submitted = try sweepReceipt(status: "submitted")
        #expect(submitted.breakdown.map(\.label) == ["ETH", "USDC"])
        #expect(submitted.header.title == "Send tokens")
    }

    /// A sweep whose native line the gas reserve dropped sent one coin: the
    /// title names it, and no "1 assets" list repeats it.
    @Test func aSweepThatSentOneCoinIsTitledByIt() throws {
        let usdc: [String: Any] = [
            "symbol": "USDC", "network": "Base", "chain_id": 8453, "token_address": Self.baseUsdc,
            "decimals": 6, "balance": "0.05", "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
        let model = try receipt([
            "stage": "receipt", "multi_select_mode": true, "multi_chain_id": 8453,
            "selected_token": usdc, "recipient": Self.devWallet, "tx_status": "confirmed",
            "receipt": [
                "status": "confirmed", "hold_reason": NSNull(), "kind": "multi_select",
                "transfers": [[String: Any]](),
                "coins": [coin("0.034929", "USDC", Self.baseUsdc, 0.034929)],
                "amount": "0.034929", "usd_value": 0.034929,
                "submitted_at_ms": NSNull(), "typical_inclusion_s": NSNull(),
            ] as [String: Any],
        ])
        #expect(model.title == "Sent 0.034929 USDC")
        #expect(model.breakdown.isEmpty)
        #expect(model.breakdownTitle == nil)
    }

    /// A split's headline is the one coin it sent, at its total — the core's
    /// `coins[0]` (it used to be the first row).
    @Test func aSplitReceiptHeadsWithItsTotal() throws {
        let usdc: [String: Any] = [
            "symbol": "USDC", "network": "Gnosis", "chain_id": 100, "token_address": "0xusdc",
            "decimals": 6, "balance": "10", "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
        func leg(_ to: String, _ amount: String) -> [String: Any] {
            ["to": to, "to_name": NSNull(), "amount": amount, "symbol": "USDC", "logo_urls": [String](), "usd_value": 0]
        }
        let model = try receipt([
            "stage": "receipt", "split_mode": true, "selected_token": usdc, "tx_status": "confirmed",
            "receipt": [
                "status": "confirmed", "hold_reason": NSNull(), "kind": "split",
                "transfers": [leg(Self.alice, "2"), leg(Self.bob, "3")],
                "coins": [coin("5", "USDC", "0xusdc", 5)],
                "amount": "5", "usd_value": 5,
                "submitted_at_ms": NSNull(), "typical_inclusion_s": NSNull(),
            ] as [String: Any],
        ])
        #expect(model.title == "Sent 5 USDC")
        #expect(model.header.title == "Send USDC")
        #expect(model.breakdownTitle == loc.t("send.recipientCount_other", vars: ["count": "2"]))
        #expect(model.breakdown.map(\.value) == ["2 USDC", "3 USDC"], "the people are still listed")
    }
}
