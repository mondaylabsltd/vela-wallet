//
//  MultisigViewsTests.swift
//  VelaWalletTests
//
//  The multisig screens' models (native multisig M1–M6): the typed views
//  read the core's wire as it is (snake_case views, camelCase records,
//  records carried back whole), the words are the corpus's for every state
//  the core can say, the proposal sheet's model draws every branch, and the
//  notifier says each change once.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct MultisigViewsTests {

    private let loc = Loc(overrideTag: "en")
    private var words: MsWords { MsWords(loc: loc, hub: nil, localNames: ["0x88cca0eedbf2c4426110bbfc998f048689266894": "Parallel One"]) }

    static let safe = "0xd9fBf76083DAea2ecF7537092475e8fDA85C4Fca"
    static let hash = "0x" + String(repeating: "ab", count: 32)
    static let one = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    static let two = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    static let three = "0x58cd0ce6A27099220543b31710d7860d75Ba1d3d"

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    static let accountRecord = """
    {"address":"\(safe)","genesis":{"factory":"0x1","singleton":"0x2","salt_nonce":"0x5","initializer":"0x","name":"Team fund"},
     "owners":["\(one)","\(two)","\(three)"],"threshold":2,"recorded":true,"chainIds":[100],"firstRun":true,"addedMs":1}
    """

    static let accountsJson = """
    {"loaded":true,"accounts":[{"account":\(accountRecord),"name":null,"shared":true,"record_waiting":false,
      "local_members":["\(one)"],"high_risk_on":[],"refused_on":[],"checked_on":[100],"checking_on":[],"invite_shared":false}],
     "active":"\(safe)","create":{"phase":"idle","draft_address":null,"draft_error":null,"address":null,"refused":null,"error":null,
      "networks":{"100":"present"},"other_space":[11155111],"kept":null},
     "add":{"phase":"idle","address":null,"outcome":null,"found":null,"ens":null},
     "import":{"safes":[],"error":null,"busy":null,"outcomes":{}},"backup":{},"share_again":{},"share_again_failure":{},
     "rename":{"\(safe.lowercased())":{"chain_id":100,"deployed":true,"blocked":"pending","lands_on":100}},
     "service_down":false,"link":null,"member_kinds":{},"known_members":{"\(two.lowercased())":{"name":"Bob"}}}
    """

    static func item(approved: Int, status: String, canApprove: [String] = [], canExecute: [String] = []) -> String {
        """
        {"chainId":100,"safeTxHash":"\(hash)","tx":{"nonce":"0"},
         "review":{"safeTxHash":"\(hash)","nonce":"0","action":{"kind":"transfer","token":null,"to":"\(three)","amount":"1000000000000000000"},"refusal":null},
         "nonce":"0","proposer":"\(one)","approvals":[{"owner":"\(one)"}\(approved > 1 ? ",{\"owner\":\"\(two)\"}" : "")],
         "approved":\(approved),"threshold":2,"status":"\(status)","canApprove":[\(canApprove.map { "\"\($0)\"" }.joined(separator: ","))],
         "canExecute":[\(canExecute.map { "\"\($0)\"" }.joined(separator: ","))],"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
         "unsent":false,"renameUndeployed":false}
        """
    }

    static func proposalsJson(item: String, standing: String, extra: String = "", ready: Int = 1) -> String {
        """
        {"followed":"\(safe)","chains":[{"chainId":100,"members":{"deployed":true,"owners":["\(one)","\(two)","\(three)"],"threshold":2,"nonce":"0"},"items":[],"mailbox":true}],
         "items":[\(item)],"open_count":1,"waiting_on_me":0,"missing":[],"cached_at":null,"unreadable":[],"history_unreadable":[],
         "offline":false,"read_at":1,"loading":false,"stale":false,"unreachable":false,"pace_ms":12000,"outbox":{},"draft":null,
         "actions":{},"reviews":{},"review_states":{},"previews":{},"executions":{},"frozen":{},"last_actor":null,
         "presence":{"\(safe.lowercased())":{"needs_you":0,"ready":\(ready),"waiting_payer":0,"first":{"chain_id":100,"safe_tx_hash":"\(hash)"}}},
         "others_waiting":false,"members_anywhere":[],"standings":{"100:\(hash)":\(standing)},"waiting_note":null,
         "timelines":{"100:\(hash)":[{"kind":"created","who":"\(one)","at_ms":1},{"kind":"approved","who":"\(two)","at_ms":2,"counted":true,"by_executing":false,"unsent":false,"on_chain":false}]},
         "dapp":{},"calls_status":{},"linked":null,"signers":{}\(extra)}
        """
    }

    // MARK: - The typed views read the wire

    @Test func theAccountsViewReadsSnakeCaseRowsAndCamelCaseRecords() throws {
        let view = try #require(decode(MsAccountsView.self, Self.accountsJson))
        #expect(view.loaded)
        let row = try #require(view.activeRow)
        #expect(row.localMembers == [Self.one])
        #expect(row.checkedOn == [100])
        #expect(row.account.threshold == 2)
        #expect(row.account.chainIds == [100])
        #expect(row.account.genesisName == "Team fund")
        #expect(row.account.firstRun == true)
        #expect(view.create.otherSpace == [11155111])
        #expect(view.create.contracts(on: 100) == "present")
        #expect(view.rename[Self.safe.lowercased()]?.blocked == "pending")
        #expect(view.knownMembers[Self.two.lowercased()]?.name == "Bob")
    }

    /// A record goes back to the core whole: its genesis keeps `salt_nonce`
    /// (no key conversion on the way in).
    @Test func aRecordIsCarriedBackAsTheCoreWroteIt() throws {
        let view = try #require(decode(MsAccountsView.self, Self.accountsJson))
        let raw = try #require(view.activeRow?.account.raw.any as? [String: Any])
        #expect((raw["genesis"] as? [String: Any])?["salt_nonce"] as? String == "0x5")
        #expect(raw["chainIds"] as? [Int64] == [100] || (raw["chainIds"] as? [Any])?.count == 1)
    }

    @Test func theProposalsViewReadsItemsStandingsAndPresence() throws {
        let json = Self.proposalsJson(item: Self.item(approved: 2, status: "ready", canExecute: [Self.one]),
                                      standing: #"{"kind":"ready"}"#)
        let view = try #require(decode(MsProposalsView.self, json))
        let item = try #require(view.item(chainId: 100, hash: Self.hash.uppercased().replacingOccurrences(of: "0X", with: "0x")))
        #expect(item.approved == 2)
        #expect(item.action?.kind == "transfer")
        #expect(view.standing(item)?.kind == "ready")
        #expect(view.presence[Self.safe.lowercased()]??.ready == 1)
        #expect(view.timeline(item).count == 2)
        #expect(view.chain(100)?.members?.deployed == true)
    }

    /// core-ms-2: the rules the shells used to derive arrive in the views —
    /// the one-press Approve and execute, the rule chip, the banner and its
    /// parts, set up, the create sheet's threshold, the invite, the hints.
    @Test func theCoresWordsForTheHeaderTheBannerAndTheStepperRead() throws {
        var json = Self.proposalsJson(item: Self.item(approved: 1, status: "pending", canApprove: [Self.one]),
                                      standing: #"{"kind":"yourTurn"}"#)
        json = json.replacingOccurrences(of: #""signers":{}"#, with: """
        "signers":{},"approve_execute":{"100:\(Self.hash)":["\(Self.one)"]},
        "rule_chip":{"kind":"differs","rules":[{"m":2,"n":3,"solo":false},{"m":2,"n":4,"solo":false}]},
        "waiting_parts":{"\(Self.safe.lowercased())":[{"kind":"needsYou","count":1},{"kind":"ready","count":2}]},
        "banner":{"parts":[{"kind":"needsYou","count":1}],"first":{"chain_id":100,"safe_tx_hash":"\(Self.hash)"}},
        "set_up":true
        """)
        let view = try #require(decode(MsProposalsView.self, json))
        let item = try #require(view.items.first)
        #expect(view.approveExecute(item) == [Self.one])
        #expect(view.ruleChip?.kind == "differs")
        #expect(view.setUp)
        #expect(MultisigHomeCard.waitingLine(view.waitingParts[Self.safe.lowercased()] ?? [], words: words)
            == "1 needs your approval · 2 ready to execute")
        #expect(view.banner?["first"]?["safe_tx_hash"]?.string == Self.hash)

        var accounts = Self.accountsJson
        accounts = accounts.replacingOccurrences(of: #""kept":null}"#, with: #""kept":null,"threshold":{"value":2,"members":3,"note":{"kind":"spare","count":1},"risky":false,"can_lower":true,"can_raise":true}}"#)
        accounts = accounts.replacingOccurrences(of: #""known_members":"#, with: #""invite":{"link":{"multisig":"\#(Self.safe)","chainIds":[100,8453],"members":null},"on_device":[100],"missing":8453,"missing_name":"Base"},"recent_networks":[100],"used_networks":[100,8453],"known_members":"#)
        let accountsView = try #require(decode(MsAccountsView.self, accounts))
        #expect(accountsView.create.threshold?["value"]?.int == 2)
        #expect(accountsView.create.threshold?["note"]?.kind == "spare")
        #expect(accountsView.invite?["missing"]?.int == 8453)
        #expect(accountsView.recentNetworks == [100])
        #expect(accountsView.usedNetworks == [100, 8453])
    }

    // MARK: - Words

    @Test func aTransactionsOneLineIsTheCorpus() {
        let action = MsJSON(any: ["kind": "transfer", "token": NSNull(), "to": Self.three, "amount": "1500000000000000000"])
        #expect(words.actionTitleNow(chainId: 100, action: action, nonce: "0") == "Send 1.5 xDAI")
        #expect(words.actionTitleNow(chainId: 100, action: MsJSON(any: ["kind": "cancel"]), nonce: "4") == "Cancel transaction #4")
        let add = MsJSON(any: ["kind": "addOwner", "owner": Self.one, "threshold": "2"])
        #expect(words.actionTitleNow(chainId: 100, action: add, nonce: "1").contains("Parallel One"))
        #expect(words.actionTitleNow(chainId: 100, action: nil, nonce: "0") == words.t("action.unsupported"))
    }

    @Test func everyStandingHasItsWord() throws {
        let item = try #require(decode(MsProposalItem.self, Self.item(approved: 1, status: "pending")))
        let cases: [(String, String)] = [
            (#"{"kind":"unsent"}"#, words.t("status.unsent")),
            (#"{"kind":"executing"}"#, words.t("status.executing")),
            (#"{"kind":"queued"}"#, words.t("status.queued")),
            (#"{"kind":"ready"}"#, words.t("status.ready")),
            (#"{"kind":"yourTurn"}"#, words.t("status.yourTurn")),
            (#"{"kind":"progress","approved":1,"threshold":2}"#, "1/2 approved"),
        ]
        for (json, expected) in cases {
            #expect(words.standing(decode(MsTagged.self, json), item: item) == expected)
        }
        let blocked = decode(MsTagged.self, #"{"kind":"blocked","block":{"kind":"nobodyPays"}}"#)
        #expect(words.standing(blocked, item: item).contains(words.t("status.nobodyPays")))
    }

    @Test func whyItWouldFailIsSaidWithTheCoresFigures() {
        let short = MsJSON(any: ["kind": "short", "have": "280000", "need": "500000", "token": ["symbol": "USDC", "decimals": 6]])
        #expect(words.whyFails(short) == "the multisig holds 0.28 USDC, and this needs 0.5 USDC")
        #expect(words.whyFails(MsJSON(any: ["kind": "reverts", "reason": "GS013"])).contains("GS013"))
        #expect(words.whyFails(MsJSON(any: ["kind": "reverts"])) == words.t("proposal.whyRevertsPlain"))
    }

    @Test func everyActionFailureIsSaidOrSilentAsTheWebSaysIt() {
        func state(_ json: String) -> MsTagged? { decode(MsTagged.self, json) }
        #expect(words.actionFailure(state(#"{"kind":"working"}"#), chainId: 100) == nil)
        #expect(words.actionFailure(state(#"{"kind":"cancelled"}"#), chainId: 100) == nil)
        #expect(words.actionFailure(state(#"{"kind":"mailboxUnreadable"}"#), chainId: 100) == words.t("proposal.mailboxDown"))
        #expect(words.actionFailure(state(#"{"kind":"noMemberHere"}"#), chainId: 100) == words.t("proposal.notMember"))
        #expect(words.actionFailure(state(#"{"kind":"signingBlocked"}"#), chainId: 100) == words.t("review.blocked"))
        let passkey = MultisigShellFailure.passkey(failure: "cancelled", timedOut: false, message: nil).detail
        let failed = MultisigMachine.decode(MsTagged.self, from: ["kind": "failed", "detail": passkey])
        #expect(words.actionFailure(failed, chainId: 100) == words.t("proposal.reason.passkeyClosed"))
        #expect(words.failureText(MultisigShellFailure.notSent.detail) == words.t("proposal.notSent"))
    }

    @Test func aRuleSaysOneMemberHonestly() {
        #expect(words.ruleTitle(m: 1, n: 1) == words.t("panel.titleSolo"))
        #expect(words.ruleChip(m: 2, n: 3) == "Needs 2 of 3")
    }

    @Test func aFindingIsSaidByWhereItWasFound() {
        let risk = MsJSON(any: ["from": "chain", "chainId": 100, "risk": ["kind": "module", "address": "0xm"]])
        #expect(words.finding(risk).contains("0xm"))
        #expect(words.finding(risk).contains("Gnosis"))
        let refusal = MsJSON(any: ["from": "genesis", "chainId": 100, "reason": ["kind": "wrongAddress"]])
        #expect(words.finding(refusal) == words.t("authority.wrongAddress"))
    }

    // MARK: - The proposal sheet's model

    @Test func aReadyProposalOffersExecuteWithTheMachinesLook() throws {
        let preview = #","previews":{"100:\#(Self.hash):\#(Self.one.lowercased())":{"kind":"priced","fee":{"amount":"16800000000000000"},"canPay":true,"setup":{"multisig":true,"members":[],"executor":false,"amount":"868000000000000"}}}"#
        var json = Self.proposalsJson(item: Self.item(approved: 2, status: "ready", canExecute: [Self.one]), standing: #"{"kind":"ready"}"#)
        json = json.replacingOccurrences(of: #""previews":{},"#, with: "") .replacingOccurrences(of: #""signers":{}"#, with: #""signers":{}"# + preview)
        let view = try #require(decode(MsProposalsView.self, json))
        let item = try #require(view.items.first)
        let model = MsProposalModel(item: item, view: view, words: words, account: nil)
        #expect(model.isReady)
        #expect(model.canExecute == [Self.one])
        let fee = try #require(model.feeLine(for: Self.one))
        #expect(fee.text.contains("0.0168"))
        #expect(fee.text.contains("Parallel One"))
        #expect(!model.executeHeld(by: Self.one))
        #expect(model.setupLines(for: Self.one).count == 2)
        #expect(model.stillNeeded.count == 1)
        #expect(model.timeline.compactMap(model.timelineLine).last?.contains("counted") == true)
    }

    @Test func anExecutionThatCannotRunIsHeldAndSaid() throws {
        var json = Self.proposalsJson(item: Self.item(approved: 2, status: "ready", canExecute: [Self.one]), standing: #"{"kind":"ready"}"#)
        let preview = #""previews":{"100:\#(Self.hash):\#(Self.one.lowercased())":{"kind":"fails","why":{"kind":"reverts","reason":"GS013"}}},"#
        json = json.replacingOccurrences(of: #""previews":{},"#, with: preview)
        let view = try #require(decode(MsProposalsView.self, json))
        let model = MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
        #expect(model.executeHeld(by: Self.one))
        #expect(model.feeLine(for: Self.one)?.tone == .danger)
    }

    @Test func anExecutionsEndingIsSaidAsItWas() throws {
        var json = Self.proposalsJson(item: Self.item(approved: 2, status: "done"), standing: #"{"kind":"ready"}"#)
        json = json.replacingOccurrences(of: #""executions":{},"#, with: #""executions":{"100:\#(Self.hash)":{"kind":"executedByOther","txHash":"0xt","paid":true}},"#)
        let view = try #require(decode(MsProposalsView.self, json))
        let model = MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
        #expect(model.notes.contains { $0.id == "lostRace" && $0.text == words.t("proposal.lostRacePaid") })
        #expect(model.executedTx == "0xt")
    }

    // MARK: - Notifications (M6)

    @Test func theNotifierSaysEachChangeOnce() throws {
        let defaults = UserDefaults(suiteName: "ms.notifier.\(UUID().uuidString)")!
        let notifier = MultisigNotifier(loc: loc, defaults: defaults)
        var posted: [MultisigNotifier.Note] = []
        notifier.post = { posted.append($0) }
        let accounts = try #require(decode(MsAccountsView.self, Self.accountsJson))
        let first = try #require(decode(MsProposalsView.self, Self.proposalsJson(
            item: Self.item(approved: 1, status: "pending"), standing: #"{"kind":"progress","approved":1,"threshold":2}"#, ready: 0)))
        // On screen: recorded, not said.
        #expect(notifier.evaluate(accounts: accounts, proposals: first, words: words, post: false).isEmpty)
        // Away: the second approval came, and it is ready.
        let second = try #require(decode(MsProposalsView.self, Self.proposalsJson(
            item: Self.item(approved: 2, status: "ready", canExecute: [Self.one]), standing: #"{"kind":"ready"}"#)))
        notifier.evaluate(accounts: accounts, proposals: second, words: words, post: true)
        #expect(posted.contains { $0.id.hasPrefix("ms-counted-") && $0.body.contains("2/2") })
        #expect(posted.contains { $0.id.hasPrefix("ms-ready-") })
        #expect(posted.allSatisfy { $0.safeTxHash == Self.hash })
        // The same views again: nothing new to say.
        posted = []
        notifier.evaluate(accounts: accounts, proposals: second, words: words, post: true)
        #expect(posted.isEmpty)
    }

    @Test func anExecutionIsSaidOnlyWhenAway() {
        let notifier = MultisigNotifier(loc: loc, defaults: UserDefaults(suiteName: "ms.exec.\(UUID().uuidString)")!)
        var posted = 0
        notifier.post = { _ in posted += 1 }
        notifier.isAway = { false }
        notifier.executed(title: "Send 1 xDAI", body: "Executed", chainId: 100, safeTxHash: Self.hash)
        #expect(posted == 0)
        notifier.isAway = { true }
        notifier.executed(title: "Send 1 xDAI", body: "Executed", chainId: 100, safeTxHash: Self.hash)
        #expect(posted == 1)
    }
}
