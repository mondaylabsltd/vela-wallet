//
//  R10CoreAdoptionTests.swift
//  VelaWalletTests
//
//  The round-10 core rules this shell draws (docs/multisig-machines.md §9),
//  each replayed from the core's shared vectors where it has them:
//
//  - passkey errors: the core's words for the platform's error facts
//    (`passkey-errors.json`), the failure sheet drawing `words_key`;
//  - the "Updated" stamp: `lives_on`, `read_chain_ids`, `read_at_ms`
//    (`balance-stamp.json` through the balance machine);
//  - names: one name per address (`names.json`), this device's own accounts
//    named by `member_labels`;
//  - the rule chip from the chains (`AccountRow.rule`);
//  - `reading::Call.leg`;
//  - the incoming scan (`incoming-scan.json`) and its cadence (`next_scan`);
//  - the simulation's `state_overrides`, the page's owners and threshold;
//  - `short_with_amount`; lists in the reader's language; the two accents.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct R10CoreAdoptionTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private static let vectors = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors")

    private func vectors(_ name: String) throws -> [String: Any] {
        let data = try Data(contentsOf: Self.vectors.appendingPathComponent(name))
        return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    // MARK: - Passkey errors

    private static func kindName(_ kind: PasskeyErrorKind) -> String {
        switch kind {
        case .cancelled: "cancelled"
        case .notSupported: "not_supported"
        case .noCredential: "no_credential"
        case .appNotAssociated: "app_not_associated"
        case .noNetwork: "no_network"
        case .phoneUnreachable: "phone_unreachable"
        case .alreadyRegistered: "already_registered"
        case .timedOut: "timed_out"
        case .other: "other"
        }
    }

    @Test func everyPasskeyErrorVectorReadsTheCoresWords() throws {
        let cases = try #require(try vectors("rules/passkey-errors.json")["cases"] as? [[String: Any]])
        #expect(cases.count >= 30)
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let words = passkeyErrorWords(
                platform: c["platform"] as? String ?? "", domain: c["domain"] as? String ?? "",
                code: (c["code"] as? NSNumber).map { Int32(truncating: $0) }, message: c["message"] as? String ?? ""
            )
            let expect = try #require(c["expect"] as? [String: Any])
            #expect(Self.kindName(words.kind) == expect["kind"] as? String, "\(name)")
            if let key = expect["key"] as? String { #expect(words.key == key, "\(name)") }
            if let failure = expect["failure"] as? String { #expect(words.failure == failure, "\(name)") }
            // Every key the sheet would draw is in the corpus.
            #expect(loc.t(words.key) != words.key, "\(name): \(words.key) not in the catalog")
        }
    }

    /// The platform's error, classified by the core's reading of its facts —
    /// and the facts ride to the core for the sheet's sentence.
    @Test func aSystemPasskeyErrorIsClassifiedByTheCoreAndCarriesItsFacts() throws {
        let cancelled = PasskeyExecutor.classify(NSError(domain: "com.apple.AuthenticationServices.AuthorizationError", code: 1001))
        #expect(cancelled.kind == .cancelled)
        let failed = PasskeyExecutor.classify(NSError(domain: "ASAuthorizationErrorDomain", code: 1004,
                                                       userInfo: [NSLocalizedDescriptionKey: "无法完成操作。"]))
        #expect(failed.kind == .other)
        #expect(failed.facts?.code == 1004)
        let offline = PasskeyExecutor.classify(NSError(domain: NSURLErrorDomain, code: -1009))
        #expect(offline.facts?.words.kind == .noNetwork)

        let answer = OnboardingExecutor.failure(for: ["type": "authenticate_passkey"], error: failed)
        let error = try #require(answer["error"] as? [String: Any])
        #expect(error["platform"] as? String == "ios")
        #expect(error["domain"] as? String == "ASAuthorizationErrorDomain")
        #expect(error["code"] as? Int == 1004)
    }

    /// The failure sheet: the phone link first, then the core's sentence,
    /// never the system's English.
    @Test func theFailureSheetDrawsTheCoresSentence() {
        let key = "onboarding.common.passkeyError.noNetwork"
        let signIn = promptCopy(PromptKind(json: ["type": "sign_in_failed", "detail": "The Internet connection appears to be offline.", "words_key": key]), loc: loc)
        #expect(signIn.message == loc.t(key))
        #expect(!signIn.message.contains("Internet connection appears"))
        let create = promptCopy(PromptKind(json: ["type": "create_failed", "detail": "raw", "words_key": key]), loc: loc)
        #expect(create.message == loc.t(key))
        let phone = promptCopy(PromptKind(json: ["type": "sign_in_failed", "detail": "x", "phone_link": true, "words_key": key]), loc: loc)
        #expect(phone.message == loc.t(I18nKeys.Flow.phoneLinkFailed), "the phone link is said as that")
    }

    /// A multisig approval's passkey error: the core's words, never
    /// "domain code: description".
    @Test func aMultisigPasskeyErrorIsSaidInTheCoresWords() {
        let failure = MultisigLive.passkeyError(NSError(domain: NSURLErrorDomain, code: -1009,
                                                        userInfo: [NSLocalizedDescriptionKey: "offline"]))
        let words = MsWords(loc: loc, hub: nil)
        let text = words.failureText(failure.detail)
        #expect(text.contains(loc.t("onboarding.common.passkeyError.noNetwork")), "\(text)")
        #expect(!text.contains("NSURLErrorDomain"))
    }

    // MARK: - The "Updated" stamp

    /// `balance-stamp.json`, replayed through the balance machine this app
    /// runs: an event is dispatched, an answer resolves the operation of its
    /// kind, and the view's `last_refreshed_at_ms` is the stamp.
    @Test func theBalanceStampVectorsHold() throws {
        let cases = try #require(try vectors("balance-stamp.json")["cases"] as? [[String: Any]])
        #expect(cases.count >= 8)
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let address = try #require(c["address"] as? String)
            let core = BalanceDashboardCore()
            var pending: [(id: UInt64, type: String)] = []
            func take(_ json: String) throws -> [String: Any] {
                let answer = try CoreJSON.object(json)
                for raw in answer["cancelled_effect_ids"] as? [Any] ?? [] {
                    let id = (raw as? NSNumber)?.uint64Value
                    pending.removeAll { $0.id == id }
                }
                for effect in answer["effects"] as? [[String: Any]] ?? [] {
                    guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                          let op = effect["operation"] as? [String: Any], let type = op["type"] as? String
                    else { continue }
                    pending.append((id, type))
                }
                return answer
            }
            _ = try take(core.dispatch(eventJson: CoreJSON.string(["type": "account_changed", "address": address])))
            for (i, step) in (c["steps"] as? [[String: Any]] ?? []).enumerated() {
                if let event = step["event"] as? [String: Any] {
                    _ = try take(core.dispatch(eventJson: CoreJSON.string(event)))
                } else if let result = step["answer"] as? [String: Any] {
                    let wanted = result["type"] as? String == "fetch_settled" ? "fetch_tokens" : "read_balance_cache"
                    let index = try #require(pending.firstIndex { $0.type == wanted }, "\(name) step \(i): no \(wanted)")
                    let id = pending.remove(at: index).id
                    _ = try take(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(result)))
                }
                if step.keys.contains("stamp") {
                    let view = try CoreJSON.object(core.view())
                    let stamp = view["last_refreshed_at_ms"] as? NSNumber
                    let expected = step["stamp"] as? NSNumber
                    #expect(stamp?.doubleValue == expected?.doubleValue, "\(name), step \(i)")
                }
            }
        }
    }

    /// The recovery re-read speaks for the same networks as the stamp: a
    /// network coming back that the account does not live on reads nothing;
    /// one it lives on (or any, while it lives on none) reads it again.
    @Test func aNetworkComingBackReadsAgainOnlyWhereTheAccountLives() {
        var nudge = RecoveryNudge()
        _ = nudge.observe(11_155_111, reached: false, unreadable: [], home: [5_042_002], refreshing: false, nowMs: 0)
        let sepoliaBack = nudge.observe(11_155_111, reached: true, unreadable: [], home: [5_042_002], refreshing: false, nowMs: 1_000)
        _ = nudge.observe(5_042_002, reached: false, unreadable: [], home: [5_042_002], refreshing: false, nowMs: 2_000)
        let arcBack = nudge.observe(5_042_002, reached: true, unreadable: [], home: [5_042_002], refreshing: false, nowMs: 3_000)
        var nowhere = RecoveryNudge()
        _ = nowhere.observe(100, reached: false, unreadable: [], home: [], refreshing: false, nowMs: 0)
        let anyBack = nowhere.observe(100, reached: true, unreadable: [], home: [], refreshing: false, nowMs: 1_000)
        #expect(!sepoliaBack, "Sepolia back: not this account's network")
        #expect(arcBack, "Arc back: its stamp's network")
        #expect(anyBack, "an account living nowhere: any network back")
    }

    // MARK: - Names

    @Test func everyNamesVectorIsTheOneLabel() throws {
        let cases = try #require(try vectors("multisig/names.json")["cases"] as? [[String: Any]])
        for c in cases {
            let args = try #require(c["args"] as? [String: Any])
            #expect(MsRules.string("names.label", args) == c["expect"] as? String, "\(c["name"] ?? "?")")
        }
    }

    /// This device's own accounts are named by the accounts machine's one
    /// name — the header, the switcher and Settings read the session's view.
    @Test func theSessionsAccountsAreNamedByTheOneLabel() throws {
        let json: [String: Any] = [
            "loading": false, "has_wallet": true, "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": [
                ["index": 0, "account": ["name": "Parallel One", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894"]],
                ["index": 1, "account": ["name": "Mine", "address": "0x031d7D57c99CAF891e1C250554691Fd12D84772b"]],
            ],
        ]
        let view = try CoreJSON.decode(SessionView.self, from: json)
        let labelled = view.labelled([
            "0x88cca0eedbf2c4426110bbfc998f048689266894": "Interleave",
            // A label that is only the short address is no name: the local one stands.
            "0x031d7d57c99caf891e1c250554691fd12d84772b": AddressText.short("0x031d7D57c99CAF891e1C250554691Fd12D84772b"),
        ])
        #expect(labelled.activeName == "Interleave")
        #expect(labelled.accounts[1].account.name == "Mine")
        #expect(view.labelled([:]) == view)
    }

    // MARK: - The rule chip from the chains

    @Test func theRowsRuleIsTheChainsNotTheFounding() throws {
        let row = try #require(MultisigMachine.decode(MsAccountsView.self, from: try JSONSerialization.jsonObject(
            with: Data(MultisigViewsTests.accountsJson.replacingOccurrences(
                of: #""invite_shared":false}"#,
                with: #""invite_shared":false,"rule":{"chip":{"kind":"needs","rule":{"m":2,"n":4,"solo":false}},"from_chain":true,"last_known":true}}"#
            ).utf8)))?.activeRow)
        let words = MsWords(loc: loc, hub: nil)
        let rule = words.rowRule(row)
        #expect(rule.chip == words.ruleChip(m: 2, n: 4), "the chains' 2 of 4, never the founding 2 of 3")
        #expect(rule.title == words.ruleTitle(m: 2, n: 4))
        #expect(rule.lastKnown == words.t("presence.ruleLastKnown"))
        #expect(words.t("presence.ruleLastKnown") != "home.multisig.presence.ruleLastKnown", "the corpus has it")
        #expect(words.chipText(MsJSON(any: ["kind": "differs", "rules": []]), fallback: row.account) == words.t("presence.ruleDiffers"))
    }

    // MARK: - The leg number

    @Test func aLegIsNumberedByTheCoreAndTheLoneCallHasNone() {
        let words = MsWords(loc: loc, hub: nil)
        let leg = MsJSON(any: ["leg": "2.1", "path": [2, 1], "intent": ["kind": "call"]])
        #expect(MsReading.cardTitle(leg, words: words) == "\(words.t("review.callNumber", ["n": "2.1"])) · \(MsReading.intentTitle(MsJSON(any: ["kind": "call"]), words: words))")
        let lone = MsJSON(any: ["leg": NSNull(), "path": [], "intent": ["kind": "batch", "calls": [[:]]]])
        let title = MsReading.cardTitle(lone, words: words)
        #expect(title == MsReading.intentTitle(MsJSON(any: ["kind": "batch", "calls": [[:]]]), words: words))
        #expect(!title.contains(words.t("review.callNumber", ["n": ""]).trimmingCharacters(in: .whitespaces)), "no empty number")
    }

    // MARK: - The incoming scan

    @Test func theIncomingScanVectorsHoldThroughTheExports() throws {
        let v = try vectors("incoming-scan.json")
        for c in v["plans"] as? [[String: Any]] ?? [] {
            let answer = try CoreJSON.object(incomingScanPlan(factsJson: CoreJSON.string(c["facts"] as? [String: Any] ?? [:])))
            let expect = c["expect"] as? [String: Any] ?? [:]
            #expect((answer["scan_chain_ids"] as? [Int]) == (expect["scan_chain_ids"] as? [Int]), "\(c["name"] ?? "?")")
        }
        for c in v["windows"] as? [[String: Any]] ?? [] {
            let answer = try CoreJSON.object(incomingScanWindows(askJson: CoreJSON.string(c["ask"] as? [String: Any] ?? [:])))
            #expect(NSDictionary(dictionary: answer).isEqual(to: c["expect"] as? [String: Any] ?? [:]), "\(c["name"] ?? "?")")
        }
        for c in v["cadence"] as? [[String: Any]] ?? [] {
            let ms = incomingNextScanMs(visible: c["visible"] as? Bool ?? false,
                                        quietScans: UInt32((c["quiet_scans"] as? NSNumber)?.uint32Value ?? 0))
            #expect(Int(ms) == c["expect_ms"] as? Int, "\(c["name"] ?? "?")")
        }
    }

    /// The facts this app hands the core: a multisig's network is scanned
    /// though it is 8th in its space and holds nothing yet.
    @Test func aMultisigsNetworkIsScannedWhereverItIsInTheSpace() {
        let space = ActivityExecutor.Space(all: [11_155_111, 84_532, 421_614, 11_155_420, 80_002, 10_200, 1_315, 5_042_002], added: [])
        let facts = ActivityExecutor.scanFacts(held: [], accountNetworks: [5_042_002], space: space)
        #expect(TokenTrustStore.scanChainIds(facts: facts).contains(5_042_002))
        #expect(TokenTrustStore.cursorsKey("0xABC") == "vela.scanCursors.0xabc")
    }

    /// The cadence is the core's: once the app says it follows, the feed
    /// names when the next scan is due — armed when the hand-off's scan lands
    /// (r10 follow-up: a back-off, 15 s first) — and its timer answers with
    /// `scan_due`.
    @Test func theFeedNamesTheNextScanOnceTheAppFollows() throws {
        let core = ActivityFeedCore()
        let switched = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894"])))
        let presence: [String: Any] = ["visible": true, "since_interaction_ms": 0, "now_ms": 1_800_000_000_000]
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "presence_changed", "presence": presence, "change": "started"]))
        // The hand-off's scan lands, having found nothing: the timer is armed.
        let effects = switched["effects"] as? [[String: Any]] ?? []
        let scan = try #require(effects.first { ($0["operation"] as? [String: Any])?["type"] as? String == "scan_incoming_transfers" })
        let scanId = try #require((scan["id"] as? NSNumber)?.uint64Value)
        _ = try core.resolveEffect(effectId: scanId, resultJson: CoreJSON.string(["type": "sync_completed", "new_count": 0]))
        let view = try CoreJSON.decode(FeedViewWire.self, from: try CoreJSON.object(core.view()))
        let next = try #require(view.nextScan)
        #expect(next.afterMs == 15_000, "a quiet first scan: 15 s")
        let due = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "scan_due", "seq": next.seq, "presence": presence])))
        let ops = (due["effects"] as? [[String: Any]] ?? []).compactMap { ($0["operation"] as? [String: Any])?["type"] as? String }
        #expect(ops.contains("scan_incoming_transfers") || ops.contains("read_tx_store"), "\(ops)")
    }

    // MARK: - The page and the simulation

    @Test func theSimulationCarriesTheCoresStateOverridesAsTheyAre() throws {
        let overrides: [String: Any] = ["0x48559Bf4034a2ac8ca295E823676D93Ad416E659": ["code": "0x60", "stateDiff": ["0x04": "0x01"]]]
        let payload = try #require(MultisigLive.simulatePayload(
            from: "0x48559Bf4034a2ac8ca295E823676D93Ad416E659",
            calls: [SimDeltas.Call(to: "0x000000000000000000000000000000000000dEaD", value: "0", data: "0x")],
            stateOverrides: overrides
        ))
        let body = try #require(payload.first as? [String: Any])
        let block = try #require((body["blockStateCalls"] as? [[String: Any]])?.first)
        #expect(NSDictionary(dictionary: block["stateOverrides"] as? [String: Any] ?? [:]).isEqual(to: overrides))
        let bare = try #require(MultisigLive.simulatePayload(
            from: "0x1", calls: [SimDeltas.Call(to: "0x2", value: "0", data: "0x")], stateOverrides: nil
        ))
        #expect(((bare.first as? [String: Any])?["blockStateCalls"] as? [[String: Any]])?.first?["stateOverrides"] == nil)
    }

    @Test func thePagesExecuteCarriesTheMembersNow() {
        let params = MultisigLive.executeParams(["kind": "execute", "tx": [:], "multisig": "0x1", "threshold": 2, "owners": ["0xa", "0xb"]])
        #expect(params["owners"] as? [String] == ["0xa", "0xb"])
    }

    // MARK: - Amount + fee

    @Test func aShortfallTheAmountMadeIsSaidAsAmountPlusFee() throws {
        func option(_ withAmount: Bool?) throws -> FeeOptionWire {
            var json: [String: Any] = [
                "symbol": "USDC", "contract": NSNull(), "decimals": 6, "balance": "1", "recipient": "0x1",
                "usd_balance": "0", "usd_price": NSNull(), "amount": "1", "insufficient": true, "selected": true,
            ]
            if let withAmount { json["short_with_amount"] = withAmount }
            return try CoreJSON.decode(FeeOptionWire.self, from: json)
        }
        #expect(try option(true).shortSentence(loc) == loc.t("send.warnInsufficientForGas", vars: ["sym": "USDC"]))
        #expect(try option(false).shortSentence(loc) == loc.t("send.warnInsufficientGas", vars: ["sym": "USDC"]))
        #expect(try option(nil).shortSentence(loc) == loc.t("send.warnInsufficientGas", vars: ["sym": "USDC"]))
    }

    // MARK: - Lists in the reader's language

    @Test func aListInsideASentenceIsTheLanguagesOwn() {
        #expect(Loc.list(["#1", "#2", "#3"], language: "en") == "#1, #2, and #3")
        #expect(Loc.list(["#1", "#2", "#3"], language: "zh") != "#1, #2, #3")
        #expect(!Loc.list(["#1", "#2", "#3"], language: "zh").contains(", "))
        #expect(Loc.list(["Gnosis"], language: "zh") == "Gnosis")
    }

    // MARK: - The accent is for value only

    private func source(_ path: String) throws -> String {
        try String(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/\(path)"), encoding: .utf8)
    }

    @Test func connectingAndClosingANoticeAreNeutralInk() throws {
        let panel = try source("Components/Explore/ConnectionPanelView.swift")
        #expect(!panel.contains("theme.accentBase"), "the consent's Connect moves no value")
        let sheet = try source("Features/Onboarding/FlowSheet.swift")
        #expect(sheet.contains("VelaButton(title: loc.t(I18nKeys.Flow.close), kind: .ink)"))
    }
}
