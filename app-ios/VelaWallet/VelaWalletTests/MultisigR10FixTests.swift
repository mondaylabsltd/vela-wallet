//
//  MultisigR10FixTests.swift
//  VelaWalletTests
//
//  Round 10's iOS findings and the first real-device pass, each on the model
//  or the pixels the screen draws from:
//
//  - r10-8: a contract call's title is the core's reading ("Approve · USDC",
//    "Batch of 1 call"), never "Contract call" for one the core can say.
//  - r10-9: a dApp's connect sheet for a multisig names it, says its address
//    once, and never says the site can ask it to sign.
//  - r10-1: a network the balance could not read answering again reads the
//    balance again at once (once per network a minute).
//  - S3: every phone-ceremony failure is said — the sheet is held across the
//    turn between the code going and the core's reason arriving.
//  - S3: QR modules on whole pixels; a decoder reads the rendered image.
//  - S3: switching account never draws the previous one's total.
//  - S3: one transport rule — plain http (and ws) only to this device.
//  - S4: "Executed" once; Continue in neutral ink; why only "Approve".
//

import CoreImage
import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
struct MultisigR10FixTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords {
        MsWords(loc: loc, hub: nil, localNames: [MultisigViewsTests.one.lowercased(): "Parallel One"])
    }

    private static let hash = MultisigViewsTests.hash
    private static let one = MultisigViewsTests.one
    private static let two = MultisigViewsTests.two

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    // MARK: - r10-8: the reading's title

    private static let readingVectors = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors/multisig/reading.json")

    private func vector(_ prefix: String) throws -> [String: Any] {
        let data = try Data(contentsOf: Self.readingVectors)
        let object = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let cases = try #require(object["cases"] as? [[String: Any]])
        return try #require(cases.first { ($0["name"] as? String)?.hasPrefix(prefix) == true }, "no vector \(prefix)")
    }

    /// A proposal item whose bytes are the vector's, as the machine lists a
    /// contract call (`action.kind == "call"`).
    private func callItem(_ args: [String: Any]) throws -> MsProposalItem {
        let tx = try #require(args["tx"] as? [String: Any])
        let item: [String: Any] = [
            "chainId": args["chainId"] as? Int ?? 0, "safeTxHash": Self.hash, "tx": tx,
            "review": ["safeTxHash": Self.hash, "nonce": "3", "refusal": NSNull(),
                       "action": ["kind": "call", "to": tx["to"] ?? "", "value": "0", "data": tx["data"] ?? "0x"]],
            "nonce": "3", "proposer": Self.one, "approvals": [], "approved": 0, "threshold": 2,
            "status": "pending", "canApprove": [], "canExecute": [], "cancelledBy": NSNull(),
            "sameNonce": [], "executing": false, "createdMs": 1, "unsent": false, "renameUndeployed": false,
        ]
        return try #require(MultisigMachine.decode(MsProposalItem.self, from: item))
    }

    private func r(_ key: String, _ vars: [String: String] = [:]) -> String {
        words.t("review.reading.\(key)", vars)
    }

    @Test func anApproveIsTitledApproveAndItsTokenNeverContractCall() throws {
        let c = try vector("approve(spender, 2^256-1) of Arc's USDC")
        let args = try #require(c["args"] as? [String: Any])
        let item = try callItem(args)
        let title = words.title(item, safe: args["safe"] as? String)
        #expect(title == "\(r("approve")) · USDC", "\(title)")
        #expect(title != words.t("action.call"))
    }

    @Test func aNestedBatchIsTitledABatchOfOneCall() throws {
        let c = try vector("a batch whose leg calls multiSend of two transfers")
        let args = try #require(c["args"] as? [String: Any])
        let title = words.title(try callItem(args), safe: args["safe"] as? String)
        #expect(title == r("batchOne"), "\(title)")
    }

    @Test func aBatchOfTwoAndATokenTransferAreTitledAsTheCoreReadsThem() throws {
        let batch = try vector("a batch: a transfer and an unlimited approve")
        let batchArgs = try #require(batch["args"] as? [String: Any])
        #expect(words.title(try callItem(batchArgs), safe: batchArgs["safe"] as? String)
            == r("batchMany", ["total": "2"]))
        let send = try vector("Arc's USDC ERC-20 transfer of 10000 base units")
        let sendArgs = try #require(send["args"] as? [String: Any])
        #expect(words.title(try callItem(sendArgs), safe: sendArgs["safe"] as? String) == "\(r("send")) 0.01 USDC")
    }

    /// A call the core reads as nothing more than a call keeps the plain line;
    /// the machine's review, when the sheet has one, is read first.
    @Test func aBlindCallKeepsContractCallAndTheReviewsReadingWins() throws {
        let blind = try vector("a call nobody reads")
        let args = try #require(blind["args"] as? [String: Any])
        let item = try callItem(args)
        #expect(words.title(item, safe: args["safe"] as? String) == words.t("action.call"))

        let approve = try vector("approve(spender, 5 USDC)")
        let read = try #require(MsRules.call("review.reading", approve["args"] as! [String: Any]) as? [String: Any])
        let review = MsJSON(any: ["reading": read["reading"] ?? NSNull()])
        #expect(words.title(item, safe: args["safe"] as? String, review: review) == "\(r("approve")) · USDC")
    }

    /// Anything but a call is the proposal's own line, as before.
    @Test func aTransferKeepsItsOwnLine() throws {
        let view = try #require(decode(MsProposalsView.self, MultisigViewsTests.proposalsJson(
            item: MultisigViewsTests.item(approved: 1, status: "pending"),
            standing: #"{"kind":"progress","approved":1,"threshold":2}"#
        )))
        let item = try #require(view.items.first)
        #expect(words.title(item) == words.actionTitleNow(chainId: 100, action: item.action, nonce: item.nonce))
    }

    // MARK: - S4: "Executed" once; why only Approve

    private func model(item: String, extra: String) throws -> MsProposalModel {
        let view = try #require(decode(MsProposalsView.self, MultisigViewsTests.proposalsJson(
            item: item, standing: #"{"kind":"progress","approved":1,"threshold":2}"#, extra: extra
        )))
        return MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
    }

    @Test func executedIsSaidOnceByTheExecutedRow() throws {
        let tx = "0x" + String(repeating: "cd", count: 32)
        let item = MultisigViewsTests.item(approved: 2, status: "done")
        let view = try #require(decode(MsProposalsView.self, MultisigViewsTests.proposalsJson(
            item: item, standing: #"{"kind":"progress","approved":2,"threshold":2}"#
        ).replacingOccurrences(of: #""executions":{}"#, with: #""executions":{"100:\#(Self.hash)":{"kind":"executed","txHash":"\#(tx)","paid":true}}"#)))
        let model = MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
        #expect(model.executedTx == tx, "the executed row says it, with the explorer")
        #expect(!model.notes.contains { $0.id == "executed" }, "never a second \"Executed\" line above it")
        #expect(model.notes.allSatisfy { $0.text != words.t("proposal.executed") })
    }

    /// This device's member would complete it, but its look says it cannot
    /// pay: the machine offers no one press — and the sheet says why.
    @Test func approveAloneSaysWhyWhenTheMemberCannotPay() throws {
        let item = MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.two], canExecute: [Self.two])
        let look = #","previews":{"100:\#(Self.hash):\#(Self.two.lowercased())":{"kind":"priced","fee":{"amount":"488000000000000000"},"canPay":false,"balance":"190000000000000000"}}"#
        let model = try model(item: item, extra: "").withoutPreviews(replacedBy: look)
        let why = try #require(model.approveOnlyReason(chosen: nil))
        #expect(why.text.contains("0.488"), "\(why.text)")
        #expect(why.text.contains("0.19"), "\(why.text)")
        #expect(why.tone == .warning)
    }

    @Test func approveAloneSaysNothingWhenOnePressIsOfferedOrItWouldNotComplete() throws {
        let look = #","previews":{"100:\#(Self.hash):\#(Self.two.lowercased())":{"kind":"priced","fee":{"amount":"1"},"canPay":false,"balance":"0"}}"#
        // Offered the one press: the button says it all.
        let offered = try model(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.two], canExecute: [Self.two]),
            extra: #","approve_execute":{"100:\#(Self.hash)":["\#(Self.two)"]}"#
        ).withoutPreviews(replacedBy: look)
        #expect(offered.approveOnlyReason(chosen: nil) == nil)
        // Not an executor here: approving is all there is to do.
        let notExecutor = try model(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.two]),
            extra: ""
        ).withoutPreviews(replacedBy: look)
        #expect(notExecutor.approveOnlyReason(chosen: nil) == nil)
        // A payable look: nothing to say.
        let payable = try model(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.two], canExecute: [Self.two]),
            extra: ""
        ).withoutPreviews(replacedBy: look.replacingOccurrences(of: #""canPay":false"#, with: #""canPay":true"#))
        #expect(payable.approveOnlyReason(chosen: nil) == nil)
    }

    /// Real-device S4: Continue moves nothing — the neutral ink, not the accent.
    @Test func theSendFormsContinueIsNeutralInk() throws {
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/Features/Flows/FlowBodies.swift"), encoding: .utf8)
        #expect(source.contains("VelaButton(title: model.cta, kind: .ink, action: onContinue)"))
        #expect(!source.contains("VelaButton(title: model.cta, kind: .primary, action: onContinue)"))
    }

    // MARK: - r10-9: a dApp connecting to a multisig

    private static let multisig = "0xFFB8000000000000000000000000000000005CAa"

    private func connection(asking: Bool, multisigName: String?) -> ConnectionModel {
        let tab = DbrTabViewWire(tab: "t1", origin: "http://127.0.0.1:9398",
                                 connectedAddress: asking ? nil : Self.multisig, chainId: 5_042_002,
                                 secure: false, crashed: false)
        let view = DbrViewWire(
            ready: true,
            consent: asking ? DbrConsentViewWire(tab: "t1", origin: "http://127.0.0.1:9398",
                                                 methods: ["eth_requestAccounts"], address: Self.multisig,
                                                 chainId: 5_042_002) : nil,
            tabs: [tab], sites: [], signing: nil, queuedSigning: 0
        )
        return ExploreLive.connectionModel(
            dbr: view, tab: tab, engine: nil, identity: (name: "Parallel One", address: Self.one),
            multisigName: { $0.caseInsensitiveCompare(Self.multisig) == .orderedSame ? multisigName : nil },
            loc: loc
        )
    }

    @Test func theConnectSheetNamesTheMultisigAndNeverSaysItCanBeAskedToSign() {
        let asking = connection(asking: true, multisigName: "R10 Team")
        #expect(asking.account.name == "R10 Team")
        #expect(asking.account.address == AddressText.short(Self.multisig))
        #expect(asking.explainer == loc.t("connect.browser.multisigConnectNote"))
        #expect(asking.explainer != loc.t("connect.browser.body"))
        #expect(!asking.explainer.contains("ask you to sign"))

        let connected = connection(asking: false, multisigName: "R10 Team")
        #expect(connected.explainer == loc.t("connect.browser.multisigConnectNote"))
    }

    /// An address this device has no name for is named by itself — the row
    /// draws no second line under it (the panel's `address != name`).
    @Test func anUnnamedAccountIsItsAddressOnce() {
        let asking = connection(asking: true, multisigName: nil)
        #expect(asking.account.name == asking.account.address)
        #expect(asking.explainer == loc.t("connect.browser.body"), "a Vela account still signs")
    }

    /// Settings' member rows: a member the core names by its short address
    /// has no name — the address once (the r10 retest's "0x749a…Bca6 /
    /// 0x749a…Bca6").
    @Test func aMemberNamedByItsShortAddressHasNoName() {
        let two = Self.two
        #expect(MultisigSettingsSheet.memberName(MsWords.short(two), address: two) == nil)
        #expect(MultisigSettingsSheet.memberName(MsRules.string("names.short", ["address": two]), address: two) == nil)
        #expect(MultisigSettingsSheet.memberName("", address: two) == nil)
        #expect(MultisigSettingsSheet.memberName(nil, address: two) == nil)
        #expect(MultisigSettingsSheet.memberName("Bob", address: two) == "Bob")
    }

    // MARK: - r10-1: refresh on recovery

    /// The r10 case: the balance never read during the outage (its view
    /// lists nothing failed), the multisig's polls went unanswered, then one
    /// answered — the network came back, and the balance reads at once.
    @Test func aNetworkComingBackReadsTheBalanceAgainOnce() {
        var nudge = RecoveryNudge()
        let arc = 5_042_002
        let healthy = nudge.observe(arc, reached: true, unreadable: [], refreshing: false, nowMs: 0)
        let downOnce = nudge.observe(arc, reached: false, unreadable: [], refreshing: false, nowMs: 1_000)
        let downAgain = nudge.observe(arc, reached: false, unreadable: [], refreshing: false, nowMs: 2_000)
        let back = nudge.observe(arc, reached: true, unreadable: [], refreshing: false, nowMs: 240_000)
        let steady = nudge.observe(arc, reached: true, unreadable: [], refreshing: false, nowMs: 241_000)
        // A flap inside the minute: down and back again — read once a minute.
        _ = nudge.observe(arc, reached: false, unreadable: [], refreshing: false, nowMs: 250_000)
        let flap = nudge.observe(arc, reached: true, unreadable: [], refreshing: false, nowMs: 251_000)
        #expect(!healthy, "a healthy network: nothing")
        // r12-3: going down is the core's own rule (`pool_unreached`).
        #expect(!downOnce && !downAgain, "an outage reads nothing here")
        #expect(back, "back: the balance reads now")
        #expect(!steady, "and only once")
        #expect(!flap, "once a minute")
    }

    /// The core's own word, too: a network its last read could not reach
    /// answering anything reads it again — never while a round is out.
    @Test func aNetworkTheBalanceCouldNotReadAnsweringReadsItAgain() {
        var nudge = RecoveryNudge()
        let arc = 5_042_002
        let duringRound = nudge.observe(arc, reached: true, unreadable: [arc], refreshing: true, nowMs: 0)
        let first = nudge.observe(arc, reached: true, unreadable: [arc], refreshing: false, nowMs: 1_000)
        let again = nudge.observe(arc, reached: true, unreadable: [arc], refreshing: false, nowMs: 2_000)
        let aMinuteOn = nudge.observe(arc, reached: true, unreadable: [arc], refreshing: false, nowMs: 62_000)
        #expect(!duringRound, "never while a round is out")
        #expect(first)
        #expect(!again, "once a minute")
        #expect(aMinuteOn)
    }

    @Test func theUnreadableNetworksAreTheCoresFailedAndUnreachable() {
        var view = BalanceViewWire(
            address: Self.multisig, displayTotalUsd: 1, balanceUnknown: false, balancePartial: true,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: 1, tokens: [],
            unpricedTokens: [], failedChainIds: [5_042_002], rateLimitedChainIds: [],
            holdingsLoading: false, cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        view.unreachableNetworks = [UnreachableNetworkWire(chainId: 100, lastKnown: "held", lastSeenUsd: nil, lineKey: "assets.lastSeen")]
        #expect(WalletStore.unreadable(view) == [5_042_002, 100])
    }

    // MARK: - S3: switching never shows the previous account's money

    @Test func aBalanceViewOfAnotherAccountDrawsNoFigure() {
        let view = BalanceViewWire(
            address: Self.one, displayTotalUsd: 42, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: 1, tokens: [],
            unpricedTokens: [], failedChainIds: [], rateLimitedChainIds: [],
            holdingsLoading: false, cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        #expect(WalletLive.isFor(view, address: Self.one.lowercased()))
        #expect(!WalletLive.isFor(view, address: Self.multisig), "the multisig switched to is not this view's")
        let drawn = WalletLive.apply(view, on: WalletFixtures.buildMobileState(.h1, loc: loc), loc: loc)
        #expect(drawn.balance.integer != nil, "its own account: the figure")
        let waiting = WalletLive.awaitingAccount(drawn)
        #expect(waiting.balance.state == .loading)
        #expect(waiting.balance.integer == nil && waiting.balance.decimals == nil)
        #expect(waiting.balance.refresh == nil, "no \"Updated\" stamp for a read not made")
        #expect(waiting.assetRows.isEmpty)
    }

    // MARK: - S3: one transport rule

    @Test func plainHttpAndWsGoOnlyToThisDevice() {
        #expect(CoreHTTP.allowsTransport("https://genesis.getvela.app/api"))
        #expect(CoreHTTP.allowsTransport("http://127.0.0.1:11887"))
        #expect(CoreHTTP.allowsTransport("http://localhost:8545/rpc"))
        #expect(!CoreHTTP.allowsTransport("http://192.168.1.20:9387"), "a LAN address over plain http")
        #expect(!CoreHTTP.allowsTransport("http://127.0.0.1.evil.com"))
        #expect(CoreHTTP.allowsTransport("wss://relay.peerconnect.dev"))
        #expect(CoreHTTP.allowsTransport("ws://127.0.0.1:11892/v1/room"))
        #expect(!CoreHTTP.allowsTransport("ws://192.168.1.20:9392/v1/room"))
        #expect(!CoreHTTP.allowsTransport("ftp://x"))
        #expect(!CoreHTTP.allowsTransport("https://"))
    }

    /// The multisig service's `http` answers by the same rule as a node.
    @Test func theMultisigServiceRefusesALanAddressOverPlainHttp() {
        #expect(MultisigLive.transportRefusal("http://192.168.1.20:9387/v1/x")?.message.contains("not https") == true)
        #expect(MultisigLive.transportRefusal("http://127.0.0.1:11887/v1/x") == nil)
        #expect(MultisigLive.transportRefusal("https://genesis.getvela.app/v1/x") == nil)
    }

    @Test func aRelayOffThisDeviceOverPlainWsClosesAtOnce() async {
        var events: [PeerSocketEvent] = []
        let socket = URLSessionPeerNet.shared.open(url: URL(string: "ws://192.168.1.20:9392/v1/r")!) { event in
            events.append(event)
            return true
        }
        #expect(socket is RefusedPeerSocket)
        await Wait.until { !events.isEmpty }
        #expect(events == [.closed(code: nil)])
    }

    // MARK: - S3: a phone ceremony's failure is said

    private func onboarding() -> OnboardingModel {
        let defaults = UserDefaults(suiteName: "vela.tests.r10.\(UUID().uuidString)")!
        let accounts = AccountStore(defaults: defaults)
        return OnboardingModel(session: SessionController(store: accounts), store: accounts)
    }

    /// The code goes by itself (the other phone could not connect): the
    /// sheet stays up — as "connecting" — until the core's reason takes its
    /// place, never dismissing in between (a prompt presented into a
    /// dismissal is lost).
    @Test func aCodeEndingByItselfHoldsTheSheetForTheReason() async {
        let model = onboarding()
        model.presentQr("FIDO:/1234", creates: true)
        #expect(model.onboardingSheetPresented)
        model.presentQr(nil, creates: false)
        #expect(model.cableEndHold, "held for what the core says next")
        #expect(model.onboardingSheetPresented, "the sheet never dropped between the code and the reason")
        async let answered = model.prompt(kind: PromptKind(type: "create_failed"), confirmable: false)
        await Wait.until { model.pending != nil }
        #expect(model.onboardingSheetPresented)
        model.answerPrompt(false)
        _ = await answered
        #expect(!model.cableEndHold)
        #expect(!model.onboardingSheetPresented)
    }

    /// "Look at your phone" going by itself is held the same way.
    @Test func thePhonesTouchEndingByItselfHoldsTheSheet() {
        let model = onboarding()
        model.presentTouch(OnboardingModel.UsbTouch(kind: "touch", product: HybridCeremony.hybridProduct))
        model.presentTouch(nil)
        #expect(model.cableEndHold)
        #expect(model.onboardingSheetPresented)
    }

    /// The person's own Cancel holds nothing: the sheet goes, quietly.
    @Test func aCancelHoldsNothing() {
        let model = onboarding()
        model.presentQr("FIDO:/1234", creates: true)
        model.cancelCable()
        model.presentQr(nil, creates: false)
        #expect(!model.cableEndHold)
        #expect(!model.onboardingSheetPresented)
    }

    // MARK: - S3: QR codes on whole pixels

    private func decoded(_ image: CGImage) -> [String] {
        let detector = CIDetector(ofType: CIDetectorTypeQRCode, context: nil,
                                  options: [CIDetectorAccuracy: CIDetectorAccuracyHigh])
        return (detector?.features(in: CIImage(cgImage: image)) ?? [])
            .compactMap { ($0 as? CIQRCodeFeature)?.messageString }
    }

    /// Every module the same whole number of pixels, and nothing grey.
    private func uniform(_ image: CGImage, modules: [[Bool]], modulePx: Int, quiet: Int) -> Bool {
        guard let data = image.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return false }
        let perRow = image.bytesPerRow
        for (row, line) in modules.enumerated() {
            for (col, on) in line.enumerated() {
                for dy in 0..<modulePx {
                    for dx in 0..<modulePx {
                        let x = (col + quiet) * modulePx + dx
                        let y = (row + quiet) * modulePx + dy
                        let value = bytes[y * perRow + x * 4]
                        if value != (on ? 0 : 255) { return false }
                    }
                }
            }
        }
        return true
    }

    @Test func aCodeIsWholePixelModulesThatADecoderReads() throws {
        let link = "https://getvela.app/multisig#invite=" + String(repeating: "a1b2c3d4", count: 12)
        let modules = try #require(QrCode.modules(link))
        // 29+ modules into 260 points on a 2× and a 3× screen: never a fraction.
        for scale in [2.0, 3.0] as [CGFloat] {
            let px = QrRaster.modulePx(count: modules.count, quiet: 2, sidePx: 260 * scale)
            #expect(CGFloat(px * (modules.count + 4)) <= 260 * scale)
            #expect(CGFloat((px + 1) * (modules.count + 4)) > 260 * scale, "the largest that fits")
            let image = try #require(QrRaster.image(modules, modulePx: px, quiet: 2))
            #expect(image.width == px * (modules.count + 4))
            #expect(uniform(image, modules: modules, modulePx: px, quiet: 2), "every module \(px) px, hard-edged")
            #expect(decoded(image) == [link], "a standard decoder reads it")
        }
    }

    /// The view as the screen draws it — rendered, then decoded.
    @Test func theRenderedViewDecodes() throws {
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        let modules = try #require(QrCode.modules(address))
        for scale in [2.0, 3.0] as [CGFloat] {
            let renderer = ImageRenderer(content:
                QrModulesView(modules: modules, quiet: 2)
                    .frame(width: 260, height: 260)
                    .environment(\.displayScale, scale)
            )
            renderer.scale = scale
            let image = try #require(renderer.cgImage)
            #expect(decoded(image) == [address], "scale \(scale)")
        }
        // The receive card (no quiet modules: the card's padding is the zone).
        let card = ImageRenderer(content:
            QrCardView(label: "", modules: modules) { EmptyView() }
                .frame(width: 344, height: 344)
                .environment(\.displayScale, 3)
        )
        card.scale = 3
        #expect(decoded(try #require(card.cgImage)) == [address])
    }
}

// MARK: - Helpers

private extension MsProposalModel {
    /// The same model over a view whose `previews` are `look`'s.
    func withoutPreviews(replacedBy look: String) throws -> MsProposalModel {
        let json = "{" + look.dropFirst(1) + "}"
        let object = try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any] ?? [:]
        var view = self.view
        view.previews = (object["previews"] as? [String: Any] ?? [:]).mapValues { MsJSON(any: $0) }
        return MsProposalModel(item: item, view: view, words: words, account: account)
    }
}
