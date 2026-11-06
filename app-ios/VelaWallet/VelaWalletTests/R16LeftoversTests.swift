//
//  R16LeftoversTests.swift
//  VelaWalletTests
//
//  Round 16 leftovers on iOS (the core's commit "every press over no figure
//  signs nothing; busy on the sheet; failures are kinds; Activity names
//  unread networks; one 'Updated' answer"):
//
//  - Item 3: the multisig service answered with its rate limit — a proposal
//    sheet whose transaction is not in the unread list says BUSY (the home's
//    `service.busyNote`) with Try again, never "Couldn't reach the multisig
//    service" (`review.missing {busy}`, a link's `reason_key`).
//  - Item 4: a failure's detail is a stable KIND, worded through the link
//    door `review.failure` — never a detail's text, never an error's English
//    (the shared vectors' `failure` section, replayed here).
//  - Item 5: Activity says which networks the incoming scan could not read
//    (`TrustView.unread_chains` → `sync_completed.unread_chains` →
//    `FeedView.unread_key`): over the rows, or in the empty state's place.
//  - Item 6: "Updated <ago>" is the core's ONE answer, `updated_at_ms`.
//

#if DEBUG
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct R16LeftoversTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    private static let vectorsURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors/multisig/review-state.json")

    private static func vectors() throws -> [String: Any] {
        let data = try Data(contentsOf: vectorsURL)
        return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private static let sources = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("VelaWallet")

    private static func source(_ path: String) throws -> String {
        try String(contentsOf: sources.appendingPathComponent(path), encoding: .utf8)
    }

    // MARK: - Item 3: a busy service is said busy

    /// Every `missing` case of the shared vectors through the shell's own
    /// call (`MsRules.missingSaid`, the link door's `review.missing`) — the
    /// full catalog key, `busy` included.
    @Test func theMissingVectorsReplayThroughTheShellsLinkDoor() throws {
        let cases = try #require(try Self.vectors()["missing"] as? [[String: Any]])
        #expect(cases.contains { ($0["args"] as? [String: Any])?["busy"] as? Bool == true })
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let args = try #require(c["args"] as? [String: Any], "\(name)")
            let expect = try #require(c["expect"] as? [String: Any], "\(name)")
            let said = MsRules.missingSaid(
                loading: args["loading"] as? Bool ?? false,
                unreadable: args["unreadable"] as? Bool ?? false,
                mailbox: args["mailbox"] as? Bool,
                busy: args["busy"] as? Bool ?? false
            )
            #expect(said.key == expect["key"] as? String, "\(name): \(said.key)")
            #expect(said.retry == (expect["retry"] as? Bool), "\(name): retry")
        }
    }

    /// The proposals view carries the core's `busy`, and the sheet asks the
    /// door with it: the home's busy words, with Try again — never "Couldn't
    /// reach the multisig service".
    @Test func aRateLimitedServiceIsSaidBusyOnTheProposalSheet() throws {
        var json = MultisigViewsTests.proposalsJson(item: "", standing: "null")
        json = json.replacingOccurrences(of: #""items":[],"open_count""#, with: #""items":[],"open_count""#)
            .replacingOccurrences(of: #""mailbox":true"#, with: #""mailbox":false"#)
            .replacingOccurrences(of: #""unreachable":false"#, with: #""unreachable":false,"busy":true"#)
        let view = try #require(decode(MsProposalsView.self, json))
        #expect(view.busy, "the core's `busy` is read")
        let missing = MsRules.missingSaid(view: view, chainId: 100)
        #expect(missing.key == "home.multisig.service.busyNote")
        #expect(missing.retry, "Try again is offered")
        let line = words.said(missing.key, chainId: 100)
        #expect(line == loc.t("home.multisig.service.busyNote"), "\(line)")
        #expect(!line.contains("Couldn't reach"), "\(line)")
        #expect(!line.contains("{{"), "\(line)")
        // Not busy: the service is said as the service, as before.
        let quiet = try #require(decode(MsProposalsView.self, json.replacingOccurrences(of: #""busy":true"#, with: #""busy":false"#)))
        #expect(MsRules.missingSaid(view: quiet, chainId: 100).key == "home.multisig.proposal.unread.linkService")
    }

    /// A link's `notFound` while the service is busy: the core's
    /// `reason_key` is outside `home.multisig.proposal.*` — it is said in
    /// full, never a key on the screen.
    @Test func aLinksBusyReasonIsSaidInTheHomesWords() throws {
        let linked = try #require(decode(MsLinkedView.self, """
        {"safe":"\(MultisigViewsTests.one)","chain_id":100,"safe_tx_hash":"\(MultisigViewsTests.hash)","preview":false,
         "stand":"notFound","item":null,"members":null,"stale":false,"reason_key":"home.multisig.service.busyNote","retry":true}
        """))
        #expect(linked.failure(words) == loc.t("home.multisig.service.busyNote"))
        // Any catalog key, wherever it lives, is words — and a leaf still reads.
        #expect(words.unreadText("componentsUi.signing.signerDown", chainId: 100) == loc.t("componentsUi.signing.signerDown"))
        #expect(words.unreadText("proposal.unread.chain", chainId: 100)
            == loc.t("home.multisig.proposal.unread.chain", vars: ["network": words.network(100)]))
    }

    // MARK: - Item 4: failures are kinds

    /// Every `failure` case of the shared vectors through the shell's own
    /// call (`MsRules.failure`, the link door's `review.failure`).
    @Test func theFailureVectorsReplayThroughTheShellsLinkDoor() throws {
        let cases = try #require(try Self.vectors()["failure"] as? [[String: Any]])
        #expect(cases.count >= 20)
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let args = try #require(c["args"] as? [String: Any], "\(name)")
            let expect = try #require(c["expect"] as? [String: Any], "\(name)")
            let said = MsRules.failure(args["detail"] as? String)
            #expect(said.kind == expect["kind"] as? String, "\(name): \(said.kind)")
            #expect(said.key == expect["key"] as? String, "\(name): \(said.key ?? "nil")")
            // And every one is said in words — never the detail, never a key.
            let detail = args["detail"] as? String ?? ""
            let text = words.failureText(detail, chainId: 100)
            #expect(!text.isEmpty && !text.contains("{{"), "\(name): \(text)")
            #expect(!text.hasPrefix("home.") && !text.hasPrefix("componentsUi."), "\(name): \(text)")
            for raw in ["TypeError", "NotAllowedError", "did not check out: bad s", "cannot_execute_now", "something_new"] {
                #expect(!text.contains(raw), "\(name): \(text)")
            }
        }
    }

    /// Plain text, an unknown kind's message, a passkey's English: one
    /// sentence from the catalog — "Something went wrong, so nothing was
    /// sent." — never the text.
    @Test func noFailureDetailIsEverDrawnAsText() {
        let unknown = words.t("proposal.reason.unknown")
        #expect(words.failureText("Error: relay said 500") == unknown)
        #expect(words.failureText("the passkey's signature did not check out: bad s") == unknown)
        #expect(words.failureText(#"{"kind":"unknown","message":"TypeError: x is undefined"}"#) == unknown)
        #expect(words.failureText(#"{"kind":"unknown","reason":"cannot_execute_now"}"#) == unknown)
        #expect(words.failureText(nil) == unknown)
        let english = #"{"kind":"passkey","failure":"other","timedOut":false,"message":"NotAllowedError: blah"}"#
        #expect(words.failureText(english) == words.t("proposal.reason.passkeyFailed"))
        // The shell's own catalog key for the platform's error stays its words.
        let keyed = MultisigShellFailure.passkey(failure: "other", timedOut: false, message: "onboarding.common.unknownBody").detail
        let refined = words.failureText(keyed)
        #expect(refined == words.t("proposal.failed", ["reason": loc.t("onboarding.common.unknownBody")]), "\(refined)")
        // A message that is not a catalog key is never drawn.
        let stray = MultisigShellFailure.passkey(failure: "other", timedOut: false, message: "onboarding.common.noSuchKey").detail
        #expect(words.failureText(stray) == words.t("proposal.reason.passkeyFailed"))
        // A network failing before the passkey is named.
        let chain = words.failureText(#"{"kind":"chain_unreachable","chain_id":100}"#)
        #expect(chain == loc.t("home.multisig.proposal.reason.chainUnreachable", vars: ["network": words.network(100)]))
        // The kinds worded from their facts, as before.
        #expect(words.failureText(#"{"kind":"cannot_pay","need":"2","have":"1"}"#)
            == words.t("proposal.failed", ["reason": words.t("status.nobodyPays")]))
        let moved = words.failureText(#"{"kind":"fee_moved","amount":"97200000000000000","chainId":100,"executor":"0x00000000000000000000000000000000000000a1"}"#)
        #expect(moved.contains("0.0972"), "\(moved)")
    }

    /// The core's own refusal (`refused {reason}`) is its English: said in
    /// the catalog's words, never the reason.
    @Test func aRefusedDraftIsSaidInTheCatalogsWords() {
        let refused = MultisigMachine.decode(MsTagged.self, from: ["kind": "refused", "reason": "0xabc has no code on chain 100"])
        let line = words.actionFailure(refused, chainId: 100)
        #expect(line == words.t("review.refusedWhy.other"), "\(line ?? "nil")")
        #expect(line?.contains("has no code") == false)
    }

    /// The create sheet, the outside signing sheet: no core English, no
    /// transport error's text reaches the screen.
    @Test func theCreateAndOutsideSheetsSayCatalogWordsOnly() {
        #expect(words.createDraftFailed == words.t("proposal.reason.unknown"))
        #expect(words.createRefused == words.t("settings.shareAgainNote"))
        let noToken = MsWords.relayReason(why: "noToken", detail: "NSURLErrorDomain -1009: The Internet connection appears to be offline.", words: words)
        #expect(noToken == words.t("external.relayNoToken"))
        #expect(MsWords.relayReason(why: "refused", detail: "HTTP 4403", words: words) == words.t("external.relayRefused"))
        // `unreachable`'s detail is the relay's host — a name, not an error.
        #expect(MsWords.relayReason(why: "unreachable", detail: "relay.peerconnect.dev", words: words) == "relay.peerconnect.dev")
        // A detail that is not a host (an error's text) is never the reason.
        #expect(MsWords.relayReason(why: "unreachable", detail: "NSURLErrorDomain -1004: Could not connect", words: words) == nil)
        let noRoom = MsJSON(any: ["kind": "noRoom", "reason": "NSPOSIXErrorDomain 61: Connection refused"])
        #expect(MsWords.outsideFailure(noRoom, words: words, chain: "Gnosis", wallet: "MetaMask") == words.t("proposal.reason.unknown"))
    }

    /// The guard: no multisig view hands a detail, a reason, an error's
    /// description or a transport detail to the screen as text.
    @Test func noMultisigViewDrawsRawErrorText() throws {
        let files = ["Features/Multisig/Views/MultisigWords.swift", "Features/Multisig/Views/MultisigCreateSheet.swift",
                     "Features/Multisig/Views/MultisigOutsideSheet.swift", "Features/Multisig/Views/MultisigSettingsSheet.swift",
                     "Features/Multisig/Views/MultisigProposalSheet.swift", "Features/Multisig/Views/MultisigAddSheet.swift",
                     "Features/Multisig/Views/MultisigDappSheet.swift", "Features/Multisig/Views/MultisigDraftSheets.swift"]
        for file in files {
            let text = try Self.source(file)
            for raw in [#"["reason": detail]"#, #"["reason": message]"#, "MsNote(text: error", #""reason": refused"#,
                        #"default: relay["detail"]"#, #"failure["reason"]"#, #"state["reason"]"#, "localizedDescription",
                        "describe(error)"] {
                #expect(!text.contains(raw), "\(file) draws raw text: \(raw)")
            }
        }
    }

    // MARK: - Item 5: Activity names the networks it could not read

    private static let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func feed(rows: Bool, unread: [Int], key: String?, settled: Bool = false) throws -> FeedViewWire {
        let row = #"""
        {"type":"header","id":"day-1","day_start_ms":1789948800000,"timestamp":1790000000},
        {"type":"item","item":{"id":"r1","direction":"in","counterparty":"0x3e9b9555EC4b80d0aaf59Da8E956A494ab31692D","alias":null,
          "value":"0.3","symbol":"USDC","decimals":6,"usd_value":0.3,"priced":true,"chain_id":100,"timestamp":1790000000,
          "day_start_ms":1789948800000,"tx_hash":"0x01","batch":null,"kind":"receive","status":"confirmed","site":null,
          "counterparty_role":"recipient","dapp":null,"subtitle":[],"figure_maskable":true}}
        """#
        let list = rows ? "[\(row)]" : "[]"
        let keyJson = key.map { "\"\($0)\"" } ?? "null"
        let json = """
        {"rows":\(list),"home_rows":\(list),"transactions":[],"new_item_id":null,"toast":null,"hidden":false,
         "history_empty_key":"history.emptyTitle","home_empty_key":"home.emptyNoActivity","contact_rows":[],
         "account":"\(Self.account.lowercased())","settled":\(settled),"unread_chains":\(unread),"unread_key":\(keyJson)}
        """
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(FeedViewWire.self, from: Data(json.utf8))
    }

    private func balance(lastRefreshed: Double? = nil, updated: Double? = nil) -> BalanceViewWire {
        BalanceViewWire(
            address: Self.account,
            displayTotalUsd: 0, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: lastRefreshed, updatedAtMs: updated,
            tokens: [], unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    private func home(_ feed: FeedViewWire) -> WalletHomeModel {
        let drawn = WalletFixtures.buildMobileState(.h1, loc: loc)
        return WalletLive.apply(balance(), feed: feed, feedRead: true, on: drawn, loc: loc, shownAccount: Self.account)
    }

    @Test func theFeedAndTrustViewsReadTheUnreadNetworks() throws {
        let one = try feed(rows: false, unread: [100], key: "home.activityUnreadOne")
        #expect(one.unreadChains == [100])
        #expect(one.unreadKey == "home.activityUnreadOne")
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let trust = try decoder.decode(TrustViewWire.self, from: Data(#"""
        {"address":"0xab","scanning":false,"incoming":[],"sim":null,"unread_chains":[1,100]}
        """#.utf8))
        #expect(trust.unreadChains == [1, 100])
        // A view without the field (an older core) reads none.
        let older = try decoder.decode(TrustViewWire.self, from: Data(#"{"address":null,"scanning":false,"incoming":[],"sim":null}"#.utf8))
        #expect(older.unreadChains.isEmpty)
    }

    /// The scan's answer carries what the poll could not read.
    @Test func theScansAnswerCarriesTheUnreadNetworks() throws {
        let answer = ActivityExecutor.syncCompleted(newCount: 2, unreadChains: [100, 1])
        #expect(answer["type"] as? String == "sync_completed")
        #expect(answer["new_count"] as? Int == 2)
        #expect(answer["unread_chains"] as? [Int] == [100, 1])
        // Through the real core: the scan's answer is what the feed says.
        let core = ActivityFeedCore()
        let switched = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": Self.account])))
        let effects = switched["effects"] as? [[String: Any]] ?? []
        let ids = effects.compactMap { effect -> (String, UInt64)? in
            guard let type = (effect["operation"] as? [String: Any])?["type"] as? String,
                  let id = (effect["id"] as? NSNumber)?.uint64Value else { return nil }
            return (type, id)
        }
        for (type, id) in ids {
            switch type {
            case "read_tx_store":
                _ = try core.resolveEffect(effectId: id, resultJson: CoreJSON.string(["type": "store_loaded", "records": [], "now_ms": 1_800_000_000_000, "read_id": 0]))
            case "scan_incoming_transfers":
                _ = try core.resolveEffect(effectId: id, resultJson: CoreJSON.string(ActivityExecutor.syncCompleted(newCount: 0, unreadChains: [100])))
            default: break
            }
        }
        let view = try CoreJSON.decode(FeedViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(view.unreadChains == [100])
        #expect(view.unreadKey == "home.activityUnreadOne")
        #expect(!view.settled, "never 'No activity yet' over a network not read")
    }

    @Test func withRowsTheLineIsDrawnAboveThem() throws {
        let model = home(try feed(rows: true, unread: [100], key: "home.activityUnreadOne"))
        #expect(model.activitySection.mode == .rows)
        let name = WalletNetworks.builtin.meta(100)?.displayName ?? ""
        #expect(!name.isEmpty)
        #expect(model.activitySection.note == "Activity can't be read on \(name) right now.", "\(model.activitySection.note ?? "nil")")
        // Every network read: no line.
        #expect(home(try feed(rows: true, unread: [], key: nil, settled: true)).activitySection.note == nil)
    }

    @Test func withNoRowsTheLineTakesTheEmptyStatesPlace() throws {
        let model = home(try feed(rows: false, unread: [100, 1], key: "home.activityUnreadMany"))
        #expect(model.activitySection.mode == .empty, "never the skeleton")
        #expect(model.activitySection.empty?.title == "Activity can't be read on 2 networks right now.")
        #expect(model.activitySection.empty?.title != loc.t("home.emptyNoActivity"), "never 'No activity yet'")
        #expect(model.activitySection.note == nil, "said once")
        // History says it the same way.
        let drawn = HistoryModel(header: FlowHeaderModel(title: "History", backLabel: "", action: nil, pill: nil),
                                 mode: .rows, emptyText: "", groups: [])
        let empty = FlowsLive.history(try feed(rows: false, unread: [100], key: "home.activityUnreadOne"),
                                      on: drawn, loc: loc, hidden: false, shownAccount: Self.account)
        #expect(empty.mode == .empty)
        #expect(empty.emptyText.hasPrefix("Activity can't be read on "), "\(empty.emptyText)")
        let rows = FlowsLive.history(try feed(rows: true, unread: [100], key: "home.activityUnreadOne"),
                                     on: drawn, loc: loc, hidden: false, shownAccount: Self.account)
        #expect(rows.mode == .rows)
        #expect(rows.note?.hasPrefix("Activity can't be read on ") == true)
    }

    // MARK: - Item 6: "Updated" is the core's one answer

    @Test func updatedIsSaidOnlyFromTheCoresAnswer() {
        // The round settled (`last_refreshed_at_ms`) but a network was not
        // read: the core withholds `updated_at_ms` — no "Updated".
        let unread = WalletLive.refresh(balance(lastRefreshed: 1_000), loc: loc, now: Date(timeIntervalSince1970: 100))
        #expect(unread.updated == nil, "\(unread.updated ?? "nil")")
        let read = WalletLive.refresh(balance(lastRefreshed: 1_000, updated: 1_000), loc: loc, now: Date(timeIntervalSince1970: 100))
        #expect(read.updated?.hasPrefix("Updated") == true, "\(read.updated ?? "nil")")
    }
}
#endif
