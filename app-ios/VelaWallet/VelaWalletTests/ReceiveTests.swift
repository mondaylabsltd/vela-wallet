//
//  ReceiveTests.swift
//  VelaWalletTests
//
//  The receive screen: a real address, and a code a camera can read.
//
//  This screen is the one where a fixture is not embarrassing but dangerous —
//  money sent to the drawn address does not come back — so the tests are about
//  the two things that make it safe: the address is the person's own, and the
//  code encodes it rather than being a pattern that looks like it does.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ReceiveTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private var baseList: ReceiveListModel {
        guard case .receive(let model) = WalletFlowFixtures.build(.r1, loc: loc).base
        else { fatalError("the R1 fixture lost its receive list") }
        return model
    }

    private var baseQr: ReceiveQrModel {
        guard case .receiveQr(let model)? = WalletFlowFixtures.build(.r2, loc: loc).sheet
        else { fatalError("the R2 fixture lost its QR sheet") }
        return model
    }

    // MARK: - The code

    /// A real matrix, square and row-major.
    ///
    /// The row-major slicing is the thing worth pinning: get it wrong and the
    /// code still LOOKS like a QR — finder squares and all — while encoding
    /// somebody's address transposed. The finder pattern is checked in all
    /// three corners because a transposed grid keeps the top-left one.
    @Test func anAddressEncodesToASquareGridWithItsFinderPatterns() {
        guard let modules = QrCode.modules(golden) else {
            Issue.record("an ordinary address could not be encoded")
            return
        }
        let width = modules.count
        #expect(width >= 21, "a QR is at least 21 modules across")
        #expect(modules.allSatisfy { $0.count == width }, "the grid is not square")

        // A finder pattern is a filled 7×7 ring: dark border, light gap, dark
        // 3×3 core. Checked at the three corners a QR puts them in.
        func finder(atRow row: Int, column: Int) -> Bool {
            for offset in 0..<7 where !modules[row][column + offset] { return false }
            for offset in 0..<7 where !modules[row + 6][column + offset] { return false }
            for offset in 0..<7 where !modules[row + offset][column] { return false }
            for offset in 0..<7 where !modules[row + offset][column + 6] { return false }
            return modules[row + 3][column + 3]
        }
        #expect(finder(atRow: 0, column: 0), "no top-left finder")
        #expect(finder(atRow: 0, column: width - 7), "no top-right finder")
        #expect(finder(atRow: width - 7, column: 0), "no bottom-left finder")
    }

    /// Two different addresses cannot produce the same code, and the same one
    /// always does.
    @Test func theCodeIsAFunctionOfTheAddress() {
        let a = QrCode.modules(golden)
        let b = QrCode.modules("0x28C6c06298d514Db089934071355E5743bf21d60")
        #expect(a != nil && b != nil)
        #expect(a != b, "two addresses encoded to the same code")
        #expect(a == QrCode.modules(golden), "the same address encoded differently twice")
    }

    /// Nothing to encode is `nil` — and `nil` must NOT become the demo pattern
    /// by accident, which is why the fallback lives in the card and not here.
    @Test func anEmptyStringHasNoCode() {
        #expect(QrCode.modules("") == nil)
    }

    // MARK: - The screens

    /// Every network row carries the person's OWN address. Before this, 收款
    /// listed `WalletFixtures.identity` — an address belonging to nobody, and
    /// money sent to it is gone.
    @Test func theNetworkListShowsTheSignedInAddress() {
        let live = FlowsLive.receiveList(golden, on: baseList, loc: loc)
        #expect(!live.rows.isEmpty)
        #expect(live.rows.allSatisfy { $0.addressDisplay == AddressText.short(golden) })
        #expect(live.rows.allSatisfy { !$0.addressDisplay.contains("14fB1f") },
                "the fixture address is still on the receive screen")
        // One address, every built-in network — which is what the subtitle
        // promises.
        #expect(live.rows.count == ChainCatalog.chains.count)
        #expect(live.subtitle.contains(String(ChainCatalog.chains.count)))
    }

    @Test func theQrSheetCarriesTheAddressAndItsCode() {
        let live = FlowsLive.receiveQr(golden, name: "我", chain: nil, on: baseQr, loc: loc)
        #expect(live.account.identiconSeed == golden)
        #expect(live.account.name == "我")
        #expect(live.account.lines.joined() == golden, "the two mono lines lost a character")
        #expect(live.modules != nil, "the code stayed a picture")
        #expect(live.modules == QrCode.modules(golden))
    }

    /// No address, no swap: the drawn screen stands rather than showing an
    /// empty card. Every developer route reaches these screens with no session.
    @Test func noAddressLeavesTheDrawingAlone() {
        let list = FlowsLive.receiveList("", on: baseList, loc: loc)
        #expect(list.rows.count == baseList.rows.count)
        let qr = FlowsLive.receiveQr("", name: "", chain: nil, on: baseQr, loc: loc)
        #expect(qr.modules == nil, "an empty address produced a code")
        #expect(qr.account.identiconSeed == baseQr.account.identiconSeed)
    }

    /// The two mono lines are computed in ONE place now — the receive sheet and
    /// the contact sheet wrap the same address the same way.
    @Test func theAddressSplitHasOneOwner() {
        #expect(AddressText.lines(golden).count == 2)
        #expect(AddressText.lines(golden).joined() == golden)
        #expect(AddressText.lines("0x1") == ["0x", "1"])
        #expect(AddressText.lines("") == [""])
    }

    /// The sheet names the chain the person tapped, not the fixture's first
    /// one. The address is the same on every network, so what would be wrong is
    /// the sentence and the mark — which is enough: a screen that says
    /// "receive Ethereum assets here" to somebody who picked Gnosis has told
    /// them something untrue about their own money.
    @Test func theQrSheetNamesTheNetworkThatWasTapped() {
        guard let gnosis = ChainCatalog.meta(100) else {
            Issue.record("Gnosis left the catalog")
            return
        }
        let live = FlowsLive.receiveQr(golden, name: "我", chain: gnosis, on: baseQr, loc: loc)
        #expect(live.title == loc.t("receive.qrTitleNetwork", vars: ["network": "Gnosis"]))
        #expect(live.centre.ticker == "xDAI")
        // And the fixture's own network is gone from it.
        #expect(!live.title.contains("Ethereum"))
    }

    // MARK: - The watcher's snapshot

    /// The balance crosses as a NUMBER here, unlike everywhere else in this
    /// cut, because the core's deposit threshold is bit-identical to the
    /// TypeScript's — both sides must round the same way or a real deposit goes
    /// unnoticed.
    @Test func aHoldingBecomesTheCoresBaselineShape() {
        let native = ReceiveWatchStore.snapshot([
            "chain_id": 100, "symbol": "xDAI", "balance": "0.75897",
            "token_address": NSNull(), "price_usd": 1.0,
        ])
        #expect(native?["id"] as? String == "100:native")
        #expect(native?["balance"] as? Double == 0.75897)
        #expect(native?["price_usd"] as? Double == 1.0)

        let token = ReceiveWatchStore.snapshot([
            "chain_id": 1, "symbol": "USDT", "balance": "12.5",
            "token_address": "0xDAC17F958D2ee523a2206206994597C13D831ec7",
            "price_usd": NSNull(),
        ])
        #expect(token?["id"] as? String == "1:0xdac17f958d2ee523a2206206994597c13d831ec7",
                "the baseline key must not depend on how a contract was cased")
        #expect(token?["price_usd"] is NSNull)

        // A row the shell cannot read is dropped rather than sent as a zero: a
        // zero balance in the baseline turns the next real read into a deposit
        // that never happened.
        #expect(ReceiveWatchStore.snapshot(["chain_id": 1, "symbol": "X"]) == nil)
    }

    /// **The rule a recipient at a counter depends on.** When every chain
    /// fails, the shell says so — it does NOT report an empty wallet.
    ///
    /// The core refuses to diff a shrunken set precisely because an RPC
    /// failure would otherwise read as "everything was withdrawn", and the next
    /// successful fetch would read as a deposit of everything. That protection
    /// only works if the shell is honest about failing, which is what this
    /// pins. The pool is left unbooted, so every read fails without a packet
    /// leaving the machine.
    @Test func aSweepWhereEveryChainFailedSaysSoRatherThanReportingAnEmptyWallet() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let watcher = ReceiveWatchStore(
            store: store,
            pool: RpcPool(store: store, accounts: AccountStore(defaults: defaults)),
            held: HeldTokens()
        )
        watcher.watch(address: golden)

        let reply = (try? CoreJSON.object(await watcher.perform(["type": "fetch_tokens"]))) ?? [:]
        // `inactive` is the other legitimate answer — a test host is not always
        // "active" — and it stops the watcher rather than feeding it a lie.
        let type = reply["type"] as? String ?? ""
        #expect(type == "fetch_failed" || type == "inactive", "answered \(type)")
        #expect(reply["tokens"] == nil, "a failed sweep still reported holdings")
    }
}
