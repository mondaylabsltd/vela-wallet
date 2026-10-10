//
//  ScanPathTests.swift
//  VelaWalletTests
//
//  A scanned code, from the payload to what the CORE decides it means.
//
//  The camera itself cannot be driven here — there is none in a simulator, and
//  that is a fact this suite asserts rather than works around. What it does
//  drive is everything between the decoder and the send machine.
//

import CoreGraphics
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ScanPathTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"

    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    /// A receive code this app writes, scanned: the core takes it as a REQUEST
    /// with the chain and the amount the code named.
    @Test func aReceiveCodeReachesTheCoreAsARequest() throws {
        let scan = Eip681.scan(of: "ethereum:\(me)@100?value=1500000000000000000")
        #expect(scan["type"] as? String == "request")

        let core = SendCore()
        let resolved = try core.dispatch(eventJson: CoreJSON.string([
            "type": "scan_resolved", "scan": scan,
        ]))
        // The machine took it — a refusal would come back as an unchanged view
        // with an alert, and this asserts the event was understood at all.
        _ = try CoreJSON.decode(SendViewWire.self, from: try view(from: resolved))
    }

    /// A token request names the CONTRACT as the token and the parameter as the
    /// person — the pair a `/transfer` URI exists to carry.
    @Test func aTokenRequestNamesBothSides() throws {
        let parsed = try #require(
            Eip681.parse("ethereum:\(usdc)@100/transfer?address=\(me)&uint256=1000000")
        )
        let scan = parsed.scan
        #expect(scan["recipient"] as? String == me)
        #expect(scan["token_address"] as? String == usdc)
        #expect(scan["amount_base_units"] as? String == "1000000")
    }

    /// Anything that is not a request still reaches the core, as TEXT — the
    /// send screen drops it into an editable recipient field rather than
    /// refusing a code somebody just held up to a camera.
    @Test func plainTextIsNotRefused() throws {
        let scan = Eip681.scan(of: me)
        #expect(scan["type"] as? String == "text")
        #expect(scan["data"] as? String == me)

        let core = SendCore()
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "scan_resolved", "scan": scan,
        ]))
    }

    /// Issue #332 (the phones' order): Home → Scan opens Send on the picker
    /// with the scanner over it, and the code arrives afterwards. The address
    /// it read is the recipient the picker now SAYS — it used to show no sign
    /// of it, so a scan that worked read as one that had done nothing.
    @Test func aCodeScannedOntoThePickerIsDrawnAsTheRecipient() throws {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let core = SendCore()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "open_scanner"]))
        let resolved = try core.dispatch(eventJson: CoreJSON.string([
            "type": "scan_resolved", "scan": Eip681.scan(of: me),
        ]))
        let wire = try CoreJSON.decode(SendViewWire.self, from: try view(from: resolved))
        #expect(!wire.showScanner)
        #expect(wire.stage == .selectToken, "the asset is chosen next")
        #expect(wire.recipient == me)

        guard case .sendPick(let drawn) = WalletFlowFixtures.build(.sd1, loc: loc).base else {
            Issue.record("SD1 is not the picker")
            return
        }
        let live = SendLive.pick(wire, on: drawn, loc: loc)
        let line = try #require(live.recipient, "the picker showed no sign of the scan")
        #expect(line.label == loc.t("send.toLabel"))
        #expect(line.value == AddressText.short(me))
        #expect(line.mono)
        if case .identicon(let seed)? = line.lead {
            #expect(seed == me)
        } else {
            Issue.record("a real address earns its artwork")
        }

        // Nobody held, no line.
        let empty = try CoreJSON.decode(SendViewWire.self, from: try CoreJSON.object(SendCore().view()))
        #expect(SendLive.pick(empty, on: drawn, loc: loc).recipient == nil)
    }

    /// The scanner's flag is the CORE's. A shell that pushed its own screen
    /// would show a viewfinder the machine does not know is open — and the
    /// close would then not close it.
    @Test func theCoreOwnsWhetherTheScannerIsOpen() throws {
        let core = SendCore()
        let opened = try core.dispatch(eventJson: CoreJSON.string(["type": "open_scanner"]))
        #expect(try CoreJSON.decode(SendViewWire.self, from: try view(from: opened)).showScanner)
        let closed = try core.dispatch(eventJson: CoreJSON.string(["type": "close_scanner"]))
        #expect(!(try CoreJSON.decode(SendViewWire.self, from: try view(from: closed)).showScanner))
    }

    /// Issue #471: a split row's own scan icon names its row. The REAL core
    /// takes `open_scanner` with a target and holds it while the viewfinder
    /// is up; a viewfinder left without a code (`close_scanner`, which this
    /// shell now sends) drops it, so the row steers nothing later; and the
    /// single field's scan — no `target` key at all — aims at no row.
    @Test func aRowsScanNamesItsRowAndLeavingDropsIt() throws {
        let core = SendCore()
        func target(_ view: [String: Any]) -> String? { view["picker_target"] as? String }

        let opened = try view(from: core.dispatch(eventJson: CoreJSON.string([
            "type": "open_scanner", "target": "rcpt_2",
        ])))
        #expect(opened["show_scanner"] as? Bool == true)
        #expect(target(opened) == "rcpt_2")

        let closed = try view(from: core.dispatch(eventJson: CoreJSON.string(["type": "close_scanner"])))
        #expect(closed["show_scanner"] as? Bool == false)
        #expect(target(closed) == nil, "a scan left without a code still aims at its row")

        let plain = try view(from: core.dispatch(eventJson: CoreJSON.string(["type": "open_scanner"])))
        #expect(plain["show_scanner"] as? Bool == true)
        #expect(target(plain) == nil)
    }

    // MARK: - The camera's refusals

    /// There is no camera in a simulator, and that is an ANSWER — not an
    /// error, and not silence. The surface falls back to the photo library.
    @Test func noCameraIsItsOwnAnswer() async {
        let camera = CameraScanner()
        await camera.start()
        #expect(camera.refusal == .noCamera || camera.refusal == .denied,
                "a simulator has no camera and must say which refusal it is")
        #expect(!camera.running)
    }

    /// Four refusals, four sentences. A person who denied permission and a
    /// person on a device without a camera need different things said.
    @Test func eachRefusalHasItsOwnWords() {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        let denied = loc.t("componentsUi.scanner.permissionText")
        let noCamera = loc.t("componentsUi.scanner.noCamera")
        let unavailable = loc.t("componentsUi.scanner.cameraUnavailable")
        #expect(denied != noCamera)
        #expect(noCamera != unavailable)
        #expect(!denied.isEmpty && !noCamera.isEmpty && !unavailable.isEmpty)
        // And the one that offers a way out has a label for it.
        #expect(!loc.t("componentsUi.scanner.grantPermission").isEmpty)
    }
}

// MARK: - When the code names something this wallet cannot do

/// A scanned request for a chain this wallet does not have.
///
/// The core has always computed `lock_error`; until 055 this client did not
/// read it, so the flow landed on the token picker with nothing at all to say
/// why the code had not worked.
@MainActor
struct ScanLockTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func view(_ patch: [String: Any]) -> SendViewWire {
        var object = try! CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }

    @Test func anUnknownChainIsExplained() throws {
        let view = view(["lock_error": ["type": "network", "chain_id": 424242]])
        let notice = try #require(
            SendLive.lockNotice(view, loc: loc),
            "a code for a chain this wallet does not have said nothing"
        )
        #expect(notice.text.contains(loc.t("send.lock.netTitle")))
        #expect(notice.text.contains("424242"), "the chain id is what a person would look up")
    }

    @Test func anUnknownTokenIsExplained() throws {
        let notice = try #require(SendLive.lockNotice(view(["lock_error": ["type": "token"]]), loc: loc))
        #expect(notice.text.contains(loc.t("send.lock.tokenTitle")))
    }

    /// An add-network attempt that failed still owes a sentence: the person
    /// asked for something and it did not happen.
    @Test func aFailedAddNetworkSaysWhy() throws {
        for (tag, key) in [
            ("net_not_found", "send.lock.netNotFound"),
            ("net_not_compatible", "send.lock.netNotCompatible"),
            ("net_add_error", "send.lock.netAddError"),
        ] {
            let notice = try #require(
                SendLive.lockNotice(view(["add_network_msg": ["type": tag]]), loc: loc),
                "\(tag) said nothing"
            )
            #expect(notice.text == loc.t(key))
        }
    }

    /// Nothing wrong, nothing said — and the sweep's own chain notice still
    /// gets the slot it always had.
    @Test func aQuietPickerSaysNothing() {
        #expect(SendLive.lockNotice(view([:]), loc: loc) == nil)
    }
}

// MARK: - A code that names a network, and the token card (issues #312, #326)

@MainActor
struct ScanNetworkAndTokenCardTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func view(_ patch: [String: Any]) -> SendViewWire {
        var object = try! CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }

    private let bnb: [String: Any] = [
        "network": "chain-56", "chain_id": 56, "symbol": "BNB", "balance": "1",
        "decimals": 18, "token_address": NSNull(), "price_usd": 600.0,
        "logo_urls": [], "spam": false,
    ]

    /// Issue #312: the picker says which network the scanned code named, in
    /// the receive card's own words — and above an empty list, that is why.
    @Test func thePickerSaysWhichNetworkTheCodeNamed() throws {
        guard case .sendPick(let drawn) = WalletFlowFixtures.build(.sd1, loc: loc).base else {
            Issue.record("SD1 is not the picker")
            return
        }
        let named = view(["request_chain_id": 56, "tokens": [bnb]])
        // No pill: a network sheet could choose nothing the list would follow.
        #expect(SendLive.pick(named, on: drawn, loc: loc).header.pill == nil)
        let notice = try #require(SendLive.pick(named, on: drawn, loc: loc).notice)
        #expect(notice.text == loc.t("receive.shareCardNetworkNote", vars: ["network": "BNB Chain"]))

        let nothing = view(["request_chain_id": 56, "tokens": []])
        #expect(SendLive.pick(nothing, on: drawn, loc: loc).rows.isEmpty)
        #expect(SendLive.pick(nothing, on: drawn, loc: loc).notice != nil)
        #expect(SendLive.pick(nothing, on: drawn, loc: loc).empty == loc.t("send.noTokensWithBalance"))
        // Not while the core is still looking.
        #expect(SendLive.pick(view(["request_chain_id": 56, "tokens": [], "loading": true]), on: drawn, loc: loc).empty == nil)

        #expect(SendLive.pick(view([:]), on: drawn, loc: loc).notice == nil, "no network named, nothing said")
    }

    /// Issue #326: the token card is the way to another asset wherever the
    /// core says — and only there.
    @Test func theTokenCardOffersAnotherAssetOnlyWhereTheCoreDoes() {
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("SD2 is not the form")
            return
        }
        let open = view(["stage": "enter_details", "selected_token": bnb, "can_change_token": true])
        #expect(SendLive.form(open, fee: nil, display: .usd, on: drawn, loc: loc).token?.change
            == loc.t("send.selectTokenTitle"))
        let fixed = view(["stage": "enter_details", "selected_token": bnb, "can_change_token": false])
        #expect(SendLive.form(fixed, fee: nil, display: .usd, on: drawn, loc: loc).token?.change == nil)
    }

    /// The event the card sends is one the core reads.
    @Test func theCoreTakesTheTokenCardsEvent() throws {
        let core = SendCore()
        let answer = try core.dispatch(eventJson: CoreJSON.string(["type": "change_token"]))
        let object = try CoreJSON.object(answer)["view"] as? [String: Any] ?? [:]
        let wire = try CoreJSON.decode(SendViewWire.self, from: object)
        #expect(wire.canChangeToken == false, "no form, nothing to change")
        #expect(wire.requestChainId == nil)
    }
}

// MARK: - The controls the audit found dead (spec 057)

/// Three of the forty affordances, asserted where the shell decides them.
@MainActor
struct DeadControlTests {

    private func token(
        symbol: String, address: String?, chain: Int = 100
    ) -> SendTokenWire {
        var object = try! CoreJSON.object(SendCore().view())
        object["selected_token"] = [
            "network": "gnosis", "chain_id": chain, "symbol": symbol, "balance": "1",
            "decimals": 18, "token_address": address as Any? ?? NSNull(),
            "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
        return try! CoreJSON.decode(SendViewWire.self, from: object).selectedToken!
    }

    /// The picker's class chips were drawn in 021 and filtered nothing.
    ///
    /// Three classes, and every holding lands in exactly one of them — a chip
    /// that showed the same list as another would be a choice that changes
    /// nothing.
    @Test func theClassChipsPartitionTheHoldings() {
        let native = token(symbol: "xDAI", address: nil)
        let stable = token(symbol: "USDC", address: "0xdd")
        let other = token(symbol: "GNO", address: "0x9c")

        for candidate in [native, stable, other] {
            #expect(SendLive.matches("all", token: candidate), "`all` hides nothing")
        }
        // The chain's own coin is what pays for gas.
        #expect(SendLive.matches("gas", token: native))
        #expect(!SendLive.matches("gas", token: stable))

        #expect(SendLive.matches("stable", token: stable))
        #expect(!SendLive.matches("stable", token: other))
        // xDAI is a stablecoin AND Gnosis's gas coin. Gas wins, or the chips
        // are not a partition and the same coin appears in two lists — which
        // reads as a filter that does not work.
        #expect(!SendLive.matches("stable", token: native),
                "the native coin is in `gas`, not in two classes at once")

        #expect(SendLive.matches("other", token: other))
        #expect(!SendLive.matches("other", token: stable))
        #expect(!SendLive.matches("other", token: native))

        // Exactly one class each, which is what makes the chips a partition.
        for candidate in [native, stable, other] {
            let classes = ["stable", "gas", "other"].filter {
                SendLive.matches($0, token: candidate)
            }
            #expect(classes.count == 1, "\(candidate.symbol) landed in \(classes)")
        }
    }

    /// Lower case, mixed case, the bridged spelling — a stablecoin is a
    /// stablecoin however its symbol is written.
    @Test func stablecoinsAreRecognisedHoweverTheyAreSpelled() {
        // Given a contract address — a NATIVE coin of any name is gas first.
        for symbol in ["usdc", "USDC.e", "xDAI", "PathUSD"] {
            #expect(SendLive.matches("stable", token: token(symbol: symbol, address: "0xdd")),
                    "\(symbol) was not read as a stablecoin")
        }
    }
}
