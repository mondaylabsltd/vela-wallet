//
//  NativeP4Tests.swift
//  VelaWalletTests
//
//  iOS lane 4: the core's asks, taken by this shell.
//
//  - The AI advisor's card (spec 103, P2.3): the sheet hands the agent the
//    REQUEST with its facts as `sheet`, the core binds an opinion to that
//    sheet's fee, and the card words the core's standing.
//  - "Check again" (dapp-15): only where the core says the could-not-check
//    line is about the network (`SimOutcomeRecord.recheck`).
//  - A spent nonce (`RelayRejection::NonceUsed`) and a proposal this app
//    cannot make (`SendTxErrorKey::ProposalNotSupported`), worded.
//  - The passkey prompt's deadline (`passkey_ceremony_end`).
//  - A refused `/pay` link, in the web's words for the core's reason.
//
//  Through the real cores; the relay, the simulation and the clock are the
//  test's.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct NativeP4Tests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let recipient = "0x2222222222222222222222222222222222222222"
    private let payee = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    private let transferTopic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

    // MARK: - The rig (SimVerdictWaitTests', trimmed)

    @MainActor
    final class Simulation {
        var immediate: RpcOutcome?
        private(set) var asked = 0
        private var waiting: [CheckedContinuation<RpcOutcome, Never>] = []

        func run() async -> RpcOutcome {
            asked += 1
            if let immediate { return immediate }
            return await withCheckedContinuation { waiting.append($0) }
        }

        func answer(_ outcome: RpcOutcome) {
            let held = waiting
            waiting = []
            for resume in held { resume.resume(returning: outcome) }
        }
    }

    private struct Rig {
        let controller: SigningController
        let simulation: Simulation
        let trust: TokenTrustStore
    }

    private func scriptedRelay() -> RelayClient {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080604052")
        port.rpc["eth_call"] = .ok("0x" + String(repeating: "0", count: 63) + "1")
        port.rpc["eth_gasPrice"] = .ok("0x1fcbc261a00")
        port.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x0"] as [String: Any])
        port.rpc["eth_maxPriorityFeePerGas"] = .ok("0x0")
        let tier: [String: Any] = [
            "maxFeePerGas": "0x3f9784c3400", "networkFeePerGas": "0x1fcbc261a00",
            "relayerFeePerGas": "0x1fcbc261a00",
        ]
        port.rpc["pimlico_getUserOperationGasPrice"] = .ok([
            "fast": tier, "standard": tier, "slow": tier,
        ] as [String: Any])
        port.rpc["vela_getInBandGasQuote"] = .ok([
            ["recipient": recipient, "asset": "native", "feeToken": NSNull(),
             "balance": "0x56bc75e2d63100000", "decimals": 18, "symbol": "POL",
             "usdBalance": "10.75", "usdPrice": "0.1075"] as [String: Any],
        ])
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x186a0", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        port.rest["/v1/account/137/\(safe.lowercased())"] = .ok([
            "activeDepositAddress": recipient, "status": "ACTIVE",
        ])
        return RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
    }

    private func rig() -> Rig {
        let defaults = UserDefaults(suiteName: "vela.tests.p4.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let appAccounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: appAccounts, offline: true)
        let trust = TokenTrustStore(store: store, pool: pool, accounts: appAccounts, held: HeldTokens())
        let relay = scriptedRelay()
        let accounts = ScriptedAccounts()
        let signer = CountingSigner()
        let simulation = Simulation()
        let controller = SigningController(
            wallet: (address: safe, credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { signer }),
            store: store, pool: pool,
            ports: SigningController.Ports(
                nativeSymbol: { _ in "POL" },
                knownChains: { [137] },
                dataBase: { "https://data.example" },
                simJudged: { address, chainId, deltas in
                    await trust.simJudged(address: address, chainId: chainId, deltas: deltas)
                },
                simulate: { _, _ in await simulation.run() }
            ),
            timers: .stopped
        )
        return Rig(controller: controller, simulation: simulation, trust: trust)
    }

    private var transaction: SigningController.Incoming {
        SigningController.Incoming(
            id: "tx-1", method: "eth_sendTransaction",
            paramsJson: #"[{"from":"\#(safe)","to":"\#(payee)","value":"0x14d1120d7b160000"}]"#,
            origin: "https://app.example", transportId: "tab-1", chainId: 137, grantedAddress: safe
        )
    }

    private func bare(_ address: String) -> String { String(address.dropFirst(2)).lowercased() }

    /// The node's checked answer: 1.5 POL leaves the wallet.
    private func checked() -> RpcOutcome {
        let log: [String: Any] = [
            "address": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "topics": [transferTopic, "0x" + String(repeating: "0", count: 24) + bare(safe),
                       "0x" + String(repeating: "0", count: 24) + bare(payee)],
            "data": "0x" + String(repeating: "0", count: 48) + "14d1120d7b160000",
        ]
        return .ok([["calls": [["status": "0x1", "returnData": "0x", "logs": [log]] as [String: Any]]] as [String: Any]])
    }

    private func drawn(_ live: SigningController) throws -> SigningModel {
        var context = SigningLive.Context(
            loc: en, chainName: "Polygon", chainDot: .purple, nativeSymbol: "POL",
            walletName: "Me", walletAddress: safe
        )
        context.chainId = 137
        context.origin = live.request?.origin
        context.sim = live.simVerdict
        context.simulation = live.simulation
        context.simRecheck = live.simRecheck
        context.simRechecking = live.simRechecking
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: en), request: try #require(live.request),
            sign: live.shownSign, clear: live.clear, guard: live.guardView, fee: live.fee, context: context,
            gate: live.confirmState
        )
    }

    private func object(_ json: String?) throws -> [String: Any] {
        try CoreJSON.object(try #require(json))
    }

    // MARK: - "Check again" (dapp-15)

    /// No node answered: the core marks the line as the network's, the sheet
    /// offers "Check again" under it, a tap asks once more (the line stays,
    /// "Checking again…"), and the checked answer replaces it.
    @Test func aCheckNoNodeAnsweredIsAskedAgainOnTheTap() async throws {
        let rig = rig()
        let controller = rig.controller
        rig.simulation.immediate = .failed(rateLimited: false)
        controller.open(transaction)
        await Wait.until({ controller.simRecheck && !controller.sign.checkingSim }, orIdle: { controller.isIdle })
        #expect(controller.simRecheck, "unreachable is the network's: recheck")
        #expect(controller.simRecord?.recheck == true)
        let offered = try drawn(controller)
        #expect(offered.simRecheck == SimRecheckModel(label: "Check again", checking: false))

        rig.simulation.immediate = nil
        controller.simulateAgain()
        #expect(controller.simRechecking)
        await Wait.until({ rig.simulation.asked == 2 }, orIdle: { false })
        #expect(rig.simulation.asked == 2, "asked once more")
        let stale = try drawn(controller)
        #expect(stale.simRecheck == SimRecheckModel(label: "Checking again…", checking: true))
        // The line it is about to replace stays meanwhile.
        if case .notice = controller.simulation {} else { Issue.record("the last line went: \(controller.simulation)") }
        // A second tap while one is out asks nothing.
        controller.simulateAgain()
        try await Task.sleep(for: .milliseconds(50))
        #expect(rig.simulation.asked == 2)

        rig.simulation.answer(checked())
        await Wait.until({ controller.simVerdict != nil }, orIdle: { controller.isIdle })
        #expect(controller.simulation == .answered)
        #expect(!controller.simRecheck && !controller.simRechecking)
        #expect(try drawn(controller).simRecheck == nil)
    }

    /// A node that spoke and would not simulate would say the same again:
    /// no "Check again" (the core's `recheck` is false), and a tap asks nothing.
    @Test func aNodeThatSpokeIsNotAskedAgain() async throws {
        let rig = rig()
        let controller = rig.controller
        rig.simulation.immediate = .rpcError(code: -32601, message: "the method eth_simulateV1 does not exist")
        controller.open(transaction)
        await Wait.until({ controller.simRecord != nil && !controller.sign.checkingSim }, orIdle: { controller.isIdle })
        #expect(controller.simRecord?.recheck == false)
        #expect(!controller.simRecheck)
        #expect(try drawn(controller).simRecheck == nil)
        controller.simulateAgain()
        try await Task.sleep(for: .milliseconds(50))
        #expect(rig.simulation.asked == 1)
    }

    // MARK: - The advisor's card (spec 103)

    /// What the agent reads of the sheet is the REQUEST — the desktop's and
    /// the web's shape — with the sheet's facts as `sheet`; through the
    /// core's review, an opinion given now is current, and the same opinion
    /// over a fee that moved past the band is "the fee changed".
    @Test func anOpinionIsBoundToTheSheetsFeeThroughTheCore() async throws {
        let rig = rig()
        let controller = rig.controller
        rig.simulation.immediate = checked()
        controller.open(transaction)
        await Wait.until({ controller.confirmState.enabled }, orIdle: { controller.isIdle })
        #expect(controller.confirmState.enabled)

        let signing = try object(controller.agentSigningJson(judged: rig.trust.simJson))
        #expect(signing["id"] as? String == "tx-1", "the request itself, not the sign view")
        #expect(signing["method"] as? String == "eth_sendTransaction")
        let sheet = try #require(signing["sheet"] as? [String: Any])
        #expect(sheet["descriptor_base"] as? String == "https://data.example")
        #expect(sheet["fee"] is [String: Any], "the fee in force")
        #expect(sheet["simulation_pending"] as? Bool == false)

        let review = try object(intentReview(inputJson: CoreJSON.string([
            "account": safe, "signing": signing, "sheet": sheet,
        ])))
        let read = try #require(review["signing"] as? [String: Any], "\(review)")
        let digest = try #require(read["review_digest"] as? String)
        let fee = try #require(read["fee"] as? [String: Any], "the review reads the sheet's fee")
        #expect((fee["amount_raw"] as? String)?.isEmpty == false)
        #expect((read["simulation"] as? [String: Any])?["outcome"] as? String == "balance_changes",
                "the sheet's own simulation reaches the review")

        let advised = try object(intentAdvice(inputJson: CoreJSON.string([
            // The person lets the agent use this wallet (E8).
            "world": ["agent_wallets": [safe], "session": ["address": safe]] as [String: Any],
            "current_digest": digest, "current_fee": fee,
            "args": ["review_digest": digest, "verdict": "caution", "summary": "A plain transfer.",
                     "sources": ["https://docs.example/a"]] as [String: Any],
        ])))
        #expect(advised["ok"] as? Bool == true, "\(advised)")
        let advice = try #require(advised["advice"] as? [String: Any])

        func card(_ currentFee: [String: Any]) throws -> AgentAdvice {
            let standing = try object(intentAdviceStanding(inputJson: CoreJSON.string([
                "advice": advice, "current_digest": digest, "current_fee": currentFee,
            ])))
            // `AgentCore::advice()`'s shape.
            return try #require(AgentAdvice(json: CoreJSON.string([
                "advice": advice, "standing": standing["standing"] ?? NSNull(),
                "fee_when_given": standing["fee_when_given"] ?? NSNull(),
            ])))
        }
        let now = try card(fee)
        #expect(now.standing == "current")
        let current = AgentAdviceCardModel.of(now, loc: en)
        #expect(current.staleLine == nil)
        #expect(current.verdictWord == "Caution")
        #expect(current.sources == "Sources: docs.example")
        // The figure the review states, as the core shortens it (its coin is
        // named from the wallet's networks, which this review was not given).
        let shown = try #require(current.feeLine?.split(separator: " ").last.map(String.init))
        #expect(current.feeLine == "Fee when given: \(shown)")
        #expect((fee["amount"] as? String)?.hasPrefix(shown) == true, "\(shown) of \(fee)")

        var moved = fee
        moved["amount_raw"] = (fee["amount_raw"] as? String).map { $0 + "0" }
        let changed = AgentAdviceCardModel.of(try card(moved), loc: en)
        #expect(changed.staleLine == "The fee changed since this opinion")
        #expect(changed.feeLine == current.feeLine, "the fee it was given for, either way")
    }

    /// The card's words, every branch — and an answer it cannot read is no card.
    @Test func theCardWordsTheCoresStanding() throws {
        func advice(_ verdict: String, standing: String, fee: Bool = false) -> AgentAdvice? {
            AgentAdvice(json: CoreJSON.string([
                "advice": ["verdict": verdict, "summary": "s", "points": ["one", "two"], "source_hosts": []] as [String: Any],
                "standing": standing,
                "fee_when_given": fee ? ["amount": "0.01", "symbol": "xDAI"] as [String: Any] : NSNull(),
            ]))
        }
        let fine = AgentAdviceCardModel.of(try #require(advice("looks_fine", standing: "current")), loc: en)
        #expect(fine.title == "Your AI agent's opinion" && fine.verdictWord == "Looks fine")
        #expect(fine.points == ["one", "two"] && fine.sources == nil && fine.feeLine == nil && !fine.stale)
        #expect(fine.footer.hasPrefix("An opinion from your AI agent."))
        let earlier = AgentAdviceCardModel.of(try #require(advice("danger", standing: "earlier_version", fee: true)), loc: en)
        #expect(earlier.verdictWord == "Danger")
        #expect(earlier.staleLine == "Given for an earlier version of this request")
        #expect(earlier.feeLine == "Fee when given: 0.01 xDAI")
        let unknown = AgentAdviceCardModel.of(try #require(advice("odd", standing: "current")), loc: zh)
        #expect(unknown.verdictWord == zh.t("componentsUi.signing.agentAdvice.unsure"))
        #expect(AgentAdvice(json: nil) == nil)
        #expect(AgentAdvice(json: "{}") == nil)
        #expect(AgentAdvice(json: "not json") == nil)
    }

    // MARK: - A spent nonce, a proposal not made here, the passkey's deadline

    /// send-15: the core's `NonceUsed` is its own failure — Send hears
    /// `nonce_used` ("went first", Review again), never "other".
    @Test func aSpentNonceIsWentFirstAndAProposalNotMadeHereIsSaid() throws {
        #expect(SendExecutor.failureWire(.nonceUsed)["type"] as? String == "nonce_used")
        #expect(SendExecutor.failureWire(.passkeyTimedOut)["type"] as? String == "passkey_timed_out")
        #expect(userOpNonceUsedDetail().contains("went first"))

        func view(_ error: String) throws -> SendViewWire {
            var raw = try CoreJSON.object(try SendCore().view())
            raw["tx_status"] = "error"
            raw["tx_error"] = error
            return try CoreJSON.decode(SendViewWire.self, from: raw)
        }
        #expect(SendLive.confirmNotice(try view("nonce_used"), loc: en) == en.t("componentsUi.signing.wentFirst"))
        #expect(SendLive.confirmNoticeTitle(try view("nonce_used"), loc: en) == en.t("componentsTx.receipt.statusNotSent"))
        let notSupported = try view("proposal_not_supported")
        #expect(SendLive.confirmNotice(notSupported, loc: en)
                == "This app can't propose multisig transactions yet, so nothing was signed or proposed.")
    }

    /// send-11: a cancel at the core's deadline is the deadline's; any other
    /// cancel is the person's. The sheet's open confirm says which.
    @Test func aCancelAtTheDeadlineIsATimeout() throws {
        let deadline = passkeySignTimeoutMs()
        #expect(deadline > 0)
        #expect(UserOpSpine.cancelEnd(elapsedMs: 900) == .passkeyCancelled)
        #expect(UserOpSpine.cancelEnd(elapsedMs: deadline) == .passkeyTimedOut)
        #expect(UserOpSpine.cancelEnd(elapsedMs: .nan) == .passkeyCancelled)

        var sign = SignViewWire.empty
        sign.passkeyNote = "timed_out"
        #expect(SigningLive.passkeyNote(sign, loc: en) == en.t("componentsUi.signing.passkeyTimedOut"))
        sign.passkeyNote = "cancelled"
        #expect(SigningLive.passkeyNote(sign, loc: en) == en.t("componentsUi.signing.passkeyClosed"))
        sign.passkeyNote = nil
        #expect(SigningLive.passkeyNote(sign, loc: en) == nil)
    }

    // MARK: - A refused /pay link (spec 039 Part B)

    /// The core's reason, in the web's words: a token that is not one and an
    /// amount that cannot be read say so; a recipient or a network say the
    /// general sentence. Through the real machine, as a link opens.
    @Test func aRefusedPayLinkSaysWhy() async throws {
        let me = safe
        let store = PaymentRequestStore(
            executor: PaymentRequestExecutor(store: VelaStore(defaults: UserDefaults(suiteName: "vela.tests.p4.pay.\(UUID().uuidString)")!))
        )
        func verdict(_ raw: String) async throws -> PayLinkVerdict {
            let before = store.linkVerdict?.serial ?? 0
            store.linkOpened(try #require(PayLink.parse(raw)?.linkOpened, "\(raw)"))
            await Wait.until({ (store.linkVerdict?.serial ?? 0) > before }, orIdle: { false })
            return try #require(store.linkVerdict)
        }
        let amount = try await verdict("https://wallet.getvela.app/pay?to=\(me)&chain=100&amount=1e3")
        #expect(amount.accepted == nil && amount.refusal == "amount")
        #expect(PayLinkVerdict.refusalBody(amount.refusal, loc: en)
                == "The amount in this link can't be read exactly. Ask the sender for a new one.")
        let token = try await verdict("velawallet://pay?to=\(me)&chain=100&token=usdc")
        #expect(token.refusal == "token")
        #expect(PayLinkVerdict.refusalBody(token.refusal, loc: en)
                == "The token in this link isn't a contract address. Ask the sender for a new one.")
        let network = try await verdict("velawallet://pay?to=\(me)&chain=mainnet")
        #expect(network.refusal == "network")
        #expect(PayLinkVerdict.refusalBody(network.refusal, loc: en) == en.t("receive.pay.invalidBody"))
        // The same refused link opened again is answered again.
        let again = try await verdict("velawallet://pay?to=\(me)&chain=mainnet")
        #expect(again.serial == network.serial + 1)
        // An accepted one is the request, and no refusal.
        let good = try await verdict("https://getvela.app/pay?to=\(me)&chain=100&amount=1.5")
        #expect(good.refusal == nil && good.accepted?.amountBase == "1500000000000000000")
        #expect(PayLinkVerdict.refusalTitle(loc: en) == "Invalid payment link")
    }

    /// Every multisig link sheet says where the link is acted on — a damaged
    /// one too, whose Open goes to the same web wallet.
    @Test func theLinkSheetSaysMultisigOpensOnTheWeb() throws {
        let url = try #require(URL(string: "https://wallet.getvela.app/en/wallet#ms=100.\(safe)"))
        let proposal = MultisigProposalRef(chainId: 100, multisig: safe, safeTxHash: "0x" + String(repeating: "ab", count: 32))
        for link in [AppLink.proposal(proposal, url: url), .broken(.proposal, url: url)] {
            let model = try #require(MultisigLinkModel.of(link, loc: en, chainName: { _ in "Gnosis" }))
            #expect(model.onWeb == "Multisig links open in the web wallet for now.")
        }
        let zhModel = try #require(MultisigLinkModel.of(.proposal(proposal, url: url), loc: zh, chainName: { _ in nil }))
        #expect(zhModel.onWeb == "多签链接目前会在网页版钱包中打开。")
    }

    /// A Vela payment link scanned in Explore is a payment, not a page.
    @Test func aScannedPayLinkIsAPayment() {
        let link = "https://wallet.getvela.app/pay?to=\(safe)&chain=100"
        #expect(ExploreScan.route(link) == .send(link))
        #expect(ExploreScan.route("https://wallet.getvela.app/en/wallet") == .open("https://wallet.getvela.app/en/wallet"))
    }

    // MARK: - One rule for the person's node

    /// The row's notice is the core's standing: own-only and answering says
    /// nothing; unreachable outranks everything; falling back is a warning.
    @Test func theOwnNodeNoticeIsTheCoresStanding() {
        #expect(SettingsLive.OwnNodes(ownOnly: [100]).standing(100) == "own_only")
        #expect(SettingsLive.OwnNodes(unreached: [100]).standing(100) == "own_node_unreached")
        #expect(SettingsLive.OwnNodes(fallback: [100]).standing(100) == "falling_back")
        #expect(SettingsLive.OwnNodes().standing(100) == "pooled")
        #expect(SettingsLive.OwnNodes(viewJson: "not a view").standing(100) == "")
        #expect(CoreHTTP.isLocalhostHttp("http://127.0.0.1:8545") && !CoreHTTP.isLocalhostHttp("http://127.0.0.1.evil.com"))
    }
}
