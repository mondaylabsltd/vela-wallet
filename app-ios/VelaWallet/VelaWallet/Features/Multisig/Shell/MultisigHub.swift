//
//  MultisigHub.swift
//  VelaWallet
//
//  The app-wide host of the core's long-lived multisig machines — one each,
//  over the one shell (`MultisigExecutor`, the app's pipelines from
//  `MultisigLive`) — the web's `shell/machines.svelte.ts` and
//  `proposals-host.svelte.ts`.
//
//  It boots with the app graph, feeds the machines the app's facts (this
//  device's Vela accounts as `MultisigLocalAccount`, its networks and the
//  space's, the multisig service in use), routes the notices one machine
//  sends another (ONE router, `route`), and ticks the timers the machines
//  name (docs/multisig-machines.md §1.4). It decides nothing: what a fact
//  means is the machines'.
//
//  Screens read the typed views (`accountsView`, `proposalsView`,
//  `membersView`) and send events through the verbs below; a sheet the
//  machines open (a link's transaction, the risk question, an outside
//  member's wallet) is published here for the root view to present.
//

import Foundation
import Observation
import UIKit
import VelaCore

@MainActor
@Observable
final class MultisigHub {

    // MARK: - Machines

    let accounts: MultisigMachine
    let members: MultisigMachine
    let proposals: MultisigMachine
    @ObservationIgnored let live: MultisigLive

    /// The typed views, refreshed on every view the machines commit.
    private(set) var accountsView = MsAccountsView()
    private(set) var proposalsView = MsProposalsView()
    private(set) var membersView: MsJSON = .null

    // MARK: - What the root view presents

    /// The multisig sheet open now (one at a time, as on the web).
    var sheet: MultisigSheet?
    /// The high-risk question (§3.5), while a signature waits on it.
    private(set) var riskAsk: MultisigRiskAsk?
    /// An outside member's wallet asked to sign (M4), while it is open.
    var outsideSession: MultisigOutsideSessionModel?
    /// A link's transaction a device with none of its members reads (ms-21).
    private(set) var openedLink: (chainId: Int, multisig: String, safeTxHash: String)?

    /// The notifications (M6): what changed while the app was away.
    @ObservationIgnored var notifier: MultisigNotifier?
    /// Executed rows were written to the activity: whoever shows it refreshes.
    @ObservationIgnored var onRecorded: (() -> Void)?
    /// The multisig shown changed (the home follows it).
    @ObservationIgnored var onActive: ((String?) -> Void)?
    /// Words for a row's title (the proposal's own): the screens' Loc.
    @ObservationIgnored let loc: Loc

    // MARK: - Facts

    struct Facts {
        var serviceUrl: String
        var accounts: [[String: Any]]
        var networks: [Int]
        var space: [Int]
        var known: [[String: Any]]
    }

    @ObservationIgnored private let facts: () async -> Facts
    @ObservationIgnored private let shelf: VelaStore
    @ObservationIgnored private let tokens: TokenMetadata?
    @ObservationIgnored private var booted = false
    @ObservationIgnored private(set) var loaded = false
    @ObservationIgnored private var followed: String?
    @ObservationIgnored private var recordTimer: Task<Void, Never>?
    @ObservationIgnored private var proposalTimer: Task<Void, Never>?
    @ObservationIgnored private var othersTimer: Task<Void, Never>?
    @ObservationIgnored private var checkedRecords = false
    @ObservationIgnored private var lastAccountsJson = ""
    @ObservationIgnored private var lastWaitingJson = ""
    @ObservationIgnored private var appActive = true
    @ObservationIgnored private var watching = 0

    init(
        live: MultisigLive,
        loc: Loc,
        shelf: VelaStore,
        tokens: TokenMetadata?,
        cores: (accounts: MultisigCore, members: MultisigCore, proposals: MultisigCore)? = nil,
        facts: @escaping () async -> Facts
    ) {
        self.live = live
        self.loc = loc
        self.shelf = shelf
        self.tokens = tokens
        self.facts = facts
        let made = cores ?? (MultisigAccountsCore(), MultisigMembersCore(), MultisigProposalsCore())
        accounts = MultisigMachine(core: made.accounts, ports: live, name: "accounts")
        members = MultisigMachine(core: made.members, ports: live, name: "members")
        proposals = MultisigMachine(core: made.proposals, ports: live, name: "proposals")
        live.onNotice = { [weak self] notice in await self?.route(notice) }
        live.onAskRisk = { [weak self] op in await self?.askRisk(op) ?? false }
        _ = accounts.listen { [weak self] _ in self?.accountsMoved() }
        _ = proposals.listen { [weak self] _ in self?.proposalsMoved() }
        _ = members.listen { [weak self] view in self?.membersView = MsJSON(any: view) }
    }

    // MARK: - Boot

    /// Idempotent: the machines start from the app's facts; the accounts
    /// machine reads its store and says `loaded`.
    func boot() async {
        guard !booted else { return }
        booted = true
        let facts = await facts()
        accounts.dispatch([
            "type": "start", "service_url": facts.serviceUrl,
            "accounts": facts.accounts, "networks": facts.networks,
        ])
        accounts.dispatch(["type": "space", "chain_ids": facts.space])
        proposals.dispatch([
            "type": "start", "service_url": facts.serviceUrl,
            "accounts": facts.accounts, "networks": facts.networks, "known": facts.known,
        ])
        _ = await accounts.until(timeout: 30) { $0["loaded"] as? Bool == true ? true : nil }
        loaded = true
        accountsMoved()
        followRecords()
        followOthers()
    }

    /// This device's Vela accounts, read again (one was added or signed in).
    func refreshLocal() async {
        let facts = await facts()
        accounts.dispatch(["type": "local_accounts", "accounts": facts.accounts])
        members.dispatch(["type": "local_accounts", "local": facts.accounts])
        proposals.dispatch(["type": "local_accounts", "accounts": facts.accounts])
        proposals.dispatch(["type": "known", "known": facts.known])
    }

    /// The device's networks moved (one added, the space switched).
    func networksChanged() async {
        let facts = await facts()
        accounts.dispatch(["type": "networks", "networks": facts.networks])
        accounts.dispatch(["type": "space", "chain_ids": facts.space])
        proposals.dispatch(["type": "networks", "networks": facts.networks])
    }

    /// The multisig service changed (Settings → Service endpoints).
    func serviceChanged() async {
        let url = await facts().serviceUrl
        accounts.dispatch(["type": "service", "url": url])
        proposals.dispatch(["type": "service", "url": url])
    }

    // MARK: - Views moved

    private func accountsMoved() {
        accountsView = accounts.accountsView
        // Every multisig on this device, to the proposals machine (what waits
        // on each is its to read) — said again only when they changed.
        let records = (accounts.view["accounts"] as? [[String: Any]] ?? []).compactMap { $0["account"] }
        let json = CoreJSON.string(["a": records])
        if json != lastAccountsJson {
            lastAccountsJson = json
            proposals.dispatch(["type": "accounts", "accounts": records])
            if let shown = shownRecord(), followed?.lowercased() == accountsView.active?.lowercased() {
                proposals.dispatch(["type": "account", "account": shown])
            }
        }
        let active = accountsView.active
        if active?.lowercased() != followed?.lowercased() {
            followed = active
            proposals.dispatch(["type": "follow", "account": shownRecord() ?? NSNull()])
            onActive?(active)
            restartProposalTimer()
        }
    }

    private func proposalsMoved() {
        let before = proposalsView.paceMs
        proposalsView = proposals.proposalsView
        if !dappAnswers.isEmpty { settleDapps() }
        // On screen, the views say it: the notifier only keeps up (M6).
        if appActive, loaded { notifier?.evaluate(accounts: accountsView, proposals: proposalsView, words: MsWords(loc: loc, hub: self), post: false) }
        if proposalsView.paceMs != before { restartProposalTimer() }
        // The member changes waiting, as the members machine reads them.
        let waiting = proposalsView.items.map { item -> [String: Any] in
            [
                "chain_id": item.chainId, "safe_tx_hash": item.safeTxHash, "nonce": item.nonce,
                "action": item.action?.any ?? NSNull(), "approved": item.approved,
                "threshold": item.threshold, "status": item.status,
            ]
        }
        // Said once per change (the web's `#tell`): a dispatch per commit
        // would be a members view per proposals view.
        let told = CoreJSON.string(["w": waiting])
        guard told != lastWaitingJson else { return }
        lastWaitingJson = told
        members.dispatch(["type": "waiting", "proposals": waiting])
    }

    /// The stored record of the multisig shown, whole (as the core keeps it).
    func shownRecord() -> Any? {
        record(accountsView.active)
    }

    func record(_ address: String?) -> Any? {
        guard let address else { return nil }
        let rows = accounts.view["accounts"] as? [[String: Any]] ?? []
        return rows.first {
            (($0["account"] as? [String: Any])?["address"] as? String)?.caseInsensitiveCompare(address) == .orderedSame
        }?["account"]
    }

    // MARK: - Timers (docs/multisig-machines.md §1.4)

    /// The app came to the front or went away.
    func appActive(_ active: Bool) {
        appActive = active
        guard loaded else { return }
        // Away with a multisig to follow: a background refresh is asked for (M6).
        if !active, !accountsView.accounts.isEmpty { MultisigNotifier.schedule() }
        if active {
            accounts.dispatch(["type": "tick"])
            proposals.dispatch(["type": "refresh", "fresh": false])
            proposals.dispatch(["type": "online", "online": true])
        }
        restartProposalTimer()
    }

    private func followRecords() {
        if !checkedRecords {
            checkedRecords = true
            accounts.dispatch(["type": "check_records"])
        }
        accounts.dispatch(["type": "tick"])
        recordTimer?.cancel()
        recordTimer = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 60_000_000_000)
                guard let self, !Task.isCancelled else { return }
                if self.appActive { self.accounts.dispatch(["type": "tick"]) }
            }
        }
    }

    private func followOthers() {
        othersTimer?.cancel()
        proposals.dispatch(["type": "others_tick"])
        othersTimer = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 60_000_000_000)
                guard let self, !Task.isCancelled else { return }
                if self.appActive { self.proposals.dispatch(["type": "others_tick"]) }
            }
        }
    }

    /// The shown multisig's list, at the machine's pace, while the app is up.
    private func restartProposalTimer() {
        proposalTimer?.cancel()
        guard followed != nil, appActive else { return }
        let pace = max(2_000, proposalsView.paceMs)
        proposalTimer = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: UInt64(pace) * 1_000_000)
                guard let self, !Task.isCancelled else { return }
                self.proposals.dispatch(["type": "tick"])
            }
        }
    }

    /// A sheet holds a transaction: the machine's faster pace until let go.
    func watch() -> () -> Void {
        watching += 1
        proposals.dispatch(["type": "watch"])
        var released = false
        return { [weak self] in
            guard let self, !released else { return }
            released = true
            self.watching -= 1
            self.proposals.dispatch(["type": "unwatch"])
        }
    }

    // MARK: - Verbs (accounts)

    func send(_ event: [String: Any]) { accounts.dispatch(event) }

    /// Show `address` (or the Vela account again, `nil`).
    func activate(_ address: String?) {
        accounts.dispatch(["type": "activate", "address": address ?? NSNull()])
    }

    func openLink(hash: String, takeKept: Bool = false) {
        accounts.dispatch(["type": "open_link", "hash": hash, "take_kept": takeKept])
    }

    // MARK: - Verbs (members)

    /// The multisig Settings shows: its members read on each of its networks,
    /// named from this device's accounts and what the multisigs here keep.
    func showMembers(_ address: String) async {
        guard let record = record(address) as? [String: Any] else { return }
        let facts = await facts()
        let chainIds = Array(Set((record["chainIds"] as? [Any] ?? []).compactMap { ($0 as? NSNumber)?.intValue }))
            .sorted { (facts.networks.firstIndex(of: $0) ?? .max) < (facts.networks.firstIndex(of: $1) ?? .max) }
        let known = (accounts.view["accounts"] as? [[String: Any]] ?? []).compactMap { $0["account"] }
        members.dispatch([
            "type": "show", "account": record, "chain_ids": chainIds, "local": facts.accounts, "known": known,
        ])
        let contacts = shelf.readList(VelaStore.Key.contacts).compactMap { contact -> [String: Any]? in
            guard let address = contact["address"] as? String,
                  address.range(of: "^0x[0-9a-fA-F]{40}$", options: .regularExpression) != nil else { return nil }
            return [
                "address": address, "name": contact["name"] ?? NSNull(),
                "resolved_name": contact["resolved_name"] ?? NSNull(), "kind": contact["kind"] ?? NSNull(),
            ]
        }
        members.dispatch(["type": "contacts", "contacts": contacts])
    }

    func sendMembers(_ event: [String: Any]) { members.dispatch(event) }

    /// A member change's proposals as they go, per network (its words).
    var membersProposing: [Int: String] = [:]

    // MARK: - Verbs (proposals)

    func sendProposal(_ event: [String: Any]) { proposals.dispatch(event) }

    /// Propose `proposal` on `chainId` for the multisig shown: drawn with its
    /// review first (`draft`); the person confirms it on the review sheet
    /// (`confirmDraft`). Answers once the machine settled it (`done`: its
    /// safeTxHash) or why not.
    func propose(
        chainId: Int, proposal: [String: Any], address: String? = nil,
        review: Bool = true, as member: String? = nil
    ) async -> MsTagged? {
        if let address, address.lowercased() != followed?.lowercased(), let record = record(address) {
            proposals.dispatch(["type": "follow", "account": record])
            followed = address
        }
        func state(_ view: [String: Any]) -> MsTagged? {
            ((view["actions"] as? [String: Any])?["propose"]).flatMap { MultisigMachine.decode(MsTagged.self, from: $0) }
        }
        let cancelled = MultisigMachine.decode(MsTagged.self, from: ["kind": "cancelled"])
        proposals.dispatch(["type": "draft", "chain_id": chainId, "proposal": proposal])
        // Drawn (its draft up), or why not (its state).
        let drafted: MsTagged? = await proposals.until { view in
            if view["draft"] is [String: Any] { return MsTagged.drawn }
            guard let tagged = state(view), tagged.kind != "working" else { return nil }
            return tagged
        }
        guard drafted?.kind == MsTagged.drawn.kind else { return drafted }
        if review {
            drafting = chainId
            reviewReturn = sheet
            sheet = .review(chainId: chainId)
            defer { if drafting == chainId { drafting = nil } }
            // The person's word on its review: confirmed (working), or withdrawn.
            let asked: MsTagged? = await proposals.until { view in
                if state(view)?.kind == "working" || !(view["draft"] is [String: Any]) {
                    return state(view) ?? cancelled
                }
                return nil
            }
            if sheet == .review(chainId: chainId) { sheet = reviewReturn }
            reviewReturn = nil
            guard asked?.kind == "working" || asked?.kind == "done" else { return asked }
        } else {
            // A cancellation asked from its sheet: signed at once, as `member`.
            confirmDraft(as: member)
        }
        // No member here: an outside member's wallet proposes it — its answer
        // settles it.
        let done: MsTagged? = await proposals.until { view in
            guard let tagged = state(view), tagged.kind != "working", tagged.kind != "outsideAsked" else { return nil }
            return tagged
        }
        // The first proposal made here: there is now something to say (M6).
        if done?.kind == "done" { MultisigNotifier.askOnce() }
        if done?.kind != "done", proposals.view["draft"] is [String: Any] {
            proposals.dispatch(["type": "withdraw"])
        }
        return done
    }

    /// The chain a proposal is drawn on for the person's review, while it is.
    private(set) var drafting: Int?
    /// The sheet the review covered, shown again once it is answered.
    @ObservationIgnored private var reviewReturn: MultisigSheet?

    /// The send machine's `propose_safe_tx` (a multisig's confirm): the
    /// reviewed transfers, proposed — answered `proposed` / `propose_failed`.
    func proposeFromSend(_ op: [String: Any]) async -> [String: Any] {
        let chainId = MultisigExecutor.int(op["chain_id"]) ?? 0
        let safe = op["multisig"] as? String
        let transfers = (op["transfers"] as? [[String: Any]] ?? []).map { transfer -> [String: Any] in
            ["token": transfer["token"] ?? NSNull(), "to": transfer["to"] ?? "", "amount": transfer["amount"] ?? "0"]
        }
        let settled = await propose(chainId: chainId, proposal: ["kind": "transfers", "transfers": transfers], address: safe)
        if settled?.kind == "done", let hash = settled?["safe_tx_hash"]?.string {
            return ["type": "proposed", "safe_tx_hash": hash]
        }
        switch settled?.kind {
        case nil, "cancelled", "riskDeclined":
            return ["type": "propose_failed", "cancelled": true, "reason": NSNull()]
        case "mailboxUnreadable":
            return ["type": "propose_failed", "cancelled": false, "reason": "mailbox_down"]
        case "failed" where MultisigShellFailure.read(settled?["detail"]?.string)?["kind"] as? String == "passkey"
            && MultisigShellFailure.read(settled?["detail"]?.string)?["failure"] as? String == "cancelled":
            return ["type": "propose_failed", "cancelled": true, "reason": NSNull()]
        default:
            return ["type": "propose_failed", "cancelled": false, "reason": proposalsView.offline ? "offline" : NSNull()]
        }
    }

    /// One background round (M6): every multisig here read again, the one
    /// followed refreshed, then what changed is said — bounded, as iOS bounds
    /// a refresh.
    func backgroundRound() async {
        proposals.dispatch(["type": "others_tick"])
        proposals.dispatch(["type": "refresh", "fresh": true])
        try? await Task.sleep(nanoseconds: 2_000_000_000)
        _ = await proposals.until(timeout: 20) { $0["loading"] as? Bool == false ? true : nil }
        notifier?.evaluate(accounts: accountsView, proposals: proposalsView, words: MsWords(loc: loc, hub: self), post: true)
    }

    func confirmDraft(as member: String?) {
        proposals.dispatch(["type": "confirm_draft", "as_member": member ?? NSNull()])
    }

    func withdrawDraft() {
        proposals.dispatch(["type": "withdraw"])
    }

    // MARK: - A dApp's request to a multisig (M5)

    /// The requests held by the machine, by id: who answers the site.
    @ObservationIgnored private var dappAnswers: [String: ([String: Any]) -> Void] = [:]

    /// Whether `address` is a multisig on this device (a site granted it).
    func isMultisig(_ address: String) -> Bool { accountsView.row(address) != nil }

    /// A site's request to a multisig (`eth_sendTransaction`,
    /// `wallet_sendCalls`, …): the machine decides everything — refused (a
    /// message), drafted with its review, proposed on the person's word, held
    /// until executed, told "proposed" before the request's limit — and its
    /// answer goes to the site exactly once (`answer`, the page's payload).
    func dappRequest(
        id: String, address: String, method: String, paramsJson: String, origin: String, chainId: Int,
        answer: @escaping ([String: Any]) -> Void
    ) {
        guard let record = record(address) else {
            answer(["type": "err", "code": 4100, "kind": "unauthorized_account", "message": "not a multisig on this device"])
            return
        }
        let params = (try? JSONSerialization.jsonObject(with: Data(paramsJson.utf8), options: [.fragmentsAllowed])) ?? []
        dappAnswers[id] = answer
        proposals.dispatch([
            "type": "dapp_request", "id": id, "account": record, "method": method, "params": params,
            "origin": origin, "chain_id": max(0, chainId), "limit_ms": NSNull(),
        ])
        sheet = .dapp(id: id)
    }

    /// The person left the request (declined, closed, read the refusal):
    /// the machine answers the site as it stands.
    func dappLeave(_ id: String) {
        proposals.dispatch(["type": "dapp_leave", "id": id])
    }

    /// Each held request the machine answered: to its site, once.
    private func settleDapps() {
        for (id, answer) in dappAnswers {
            guard let held = proposalsView.dapp[id], let result = held["answer"], !result.isNull else { continue }
            dappAnswers.removeValue(forKey: id)
            let payload: [String: Any]
            if let error = result["error"], !error.isNull {
                let code = error["code"]?.int ?? 4001
                let kind = switch code {
                case 4001: "user_rejected"
                case 4100: "unauthorized_account"
                case -32602: "invalid_params"
                case 4200: "unsupported_capability"
                default: "submit_failed"
                }
                payload = ["type": "err", "code": code, "kind": kind, "message": error["message"]?.string ?? NSNull()]
                // An error the site was not told of takes the sheet with it.
                if held["told"]?.bool != true, sheet == .dapp(id: id) { sheet = nil }
            } else {
                payload = ["type": "ok", "result": result["result"]?.any ?? NSNull()]
            }
            answer(payload)
            Task { @MainActor [weak self] in self?.proposals.dispatch(["type": "dapp_settled", "id": id]) }
        }
    }

    // MARK: - Risk (§3.5)

    private func askRisk(_ op: [String: Any]) async -> Bool {
        await withCheckedContinuation { continuation in
            riskAsk?.answer(false)
            riskAsk = MultisigRiskAsk(
                chainId: MultisigExecutor.int(op["chain_id"]) ?? 0,
                safe: op["safe"] as? String ?? "",
                findings: MsJSON(any: op["findings"]).array
            ) { [weak self] signs in
                self?.riskAsk = nil
                continuation.resume(returning: signs)
            }
        }
    }

    // MARK: - The router (docs/multisig-machines.md §1.2)

    func route(_ notice: [String: Any]) async {
        switch notice["kind"] as? String {
        case "account":
            let address = notice["address"] as? String ?? ""
            accounts.dispatch(["type": "apply", "address": address, "update": notice["update"] ?? NSNull()])
        case "activity_rows":
            await writeRows(notice["rows"] as? [[String: Any]] ?? [])
        case "mailbox_took":
            if let safe = notice["safe"] as? String, safe.lowercased() == followed?.lowercased() {
                proposals.dispatch(["type": "refresh", "fresh": true])
            }
        case "activated":
            break
        case "open_proposal":
            let chainId = MultisigExecutor.int(notice["chain_id"]) ?? 0
            let address = notice["address"] as? String ?? ""
            let hash = notice["safe_tx_hash"] as? String ?? ""
            openedLink = (chainId, address, hash)
            proposals.dispatch([
                "type": "open_linked", "chain_id": chainId, "safe_tx_hash": hash,
                "preview": notice["preview"] ?? NSNull(),
            ])
            sheet = .proposal(chainId: chainId, safeTxHash: hash)
        case "outside_sign":
            let request = notice["request"] as? [String: Any] ?? [:]
            openOutside(request)
        default:
            break
        }
    }

    /// An outside member's wallet signs it (M4): a session of its own; its
    /// answer back to the proposals machine.
    private func openOutside(_ request: [String: Any]) {
        let model = MultisigOutsideSessionModel(request: request, ports: live, relay: shelf.readString(DevRelayKey.relay))
        model.onOutcome = { [weak self] outcome in
            self?.proposals.dispatch([
                "type": "outside_answered",
                "chain_id": request["chainId"] ?? request["chain_id"] ?? 0,
                "safe_tx_hash": request["safeTxHash"] ?? request["safe_tx_hash"] ?? "",
                "outcome": outcome,
            ])
            if self?.outsideSession === model { self?.outsideSession = nil }
        }
        outsideSession?.close()
        outsideSession = model
        model.start()
    }

    // MARK: - Activity rows

    /// The machine's executed rows, worded as Activity rows (the web's
    /// `executionRows`): a token's symbol and decimals, the proposal's title.
    private func writeRows(_ executed: [[String: Any]]) async {
        var out: [[String: Any]] = []
        for e in executed {
            guard let rows = e["rows"] as? [[String: Any]] else {
                out.append(e)
                continue
            }
            let chainId = MultisigExecutor.int(e["chainId"]) ?? 0
            let hash = e["safeTxHash"] as? String ?? ""
            for row in rows {
                let index = MultisigExecutor.int(row["index"]) ?? 0
                var base: [String: Any] = [
                    "id": MsRules.executedRowId(chainId: chainId, safeTxHash: hash, index: index),
                    "userOpHash": e["userOpHash"] ?? "",
                    "txHash": e["txHash"] ?? "",
                    "from": e["from"] ?? "",
                    "to": row["to"] ?? "",
                    "chainId": chainId,
                    "timestamp": e["timestamp"] ?? Int(Date().timeIntervalSince1970),
                    "status": "confirmed",
                ]
                if row["kind"] as? String == "send" {
                    let token = row["token"] as? String
                    let info = await tokenInfo(chainId: chainId, token: token)
                    base["value"] = MsRules.plainUnits(row["amount"] as? String ?? "0", decimals: info.decimals)
                    base["symbol"] = info.symbol
                    base["decimals"] = info.decimals
                    if let token { base["token"] = token }
                    base["type"] = "send"
                } else {
                    let sends = row["sends"] as? String ?? "0"
                    let info = await tokenInfo(chainId: chainId, token: nil)
                    let some = sends != "0" && !sends.isEmpty
                    base["value"] = some ? MsRules.plainUnits(sends, decimals: info.decimals) : "0"
                    base["symbol"] = some ? info.symbol : ""
                    base["decimals"] = info.decimals
                    base["type"] = "dapp_tx"
                    base["intent"] = await MsWords(loc: loc, hub: self).actionTitle(
                        chainId: chainId, action: MsJSON(any: e["action"]), nonce: (e["nonce"] as? String) ?? "0"
                    )
                    base["dappSummary"] = [
                        "action": "call", "calls": row["calls"] ?? 1, "contract": row["contract"] ?? "",
                    ]
                }
                out.append(base)
            }
        }
        guard !out.isEmpty else { return }
        TxRecords.writeRecords(out, store: shelf)
        onRecorded?()
        // Executed while away: said (M6).
        for e in executed {
            let chainId = MultisigExecutor.int(e["chainId"]) ?? 0
            let hash = e["safeTxHash"] as? String ?? ""
            let title = await MsWords(loc: loc, hub: self).actionTitle(
                chainId: chainId, action: MsJSON(any: e["action"]), nonce: e["nonce"] as? String ?? "0"
            )
            notifier?.executed(title: title, body: loc.t("home.multisig.proposal.executed"), chainId: chainId, safeTxHash: hash)
        }
    }

    /// A token's symbol and decimals: the chain's coin from the catalogue,
    /// an ERC-20's from the chain.
    func tokenInfo(chainId: Int, token: String?) async -> (symbol: String, decimals: Int) {
        guard let token, !token.isEmpty else {
            return (ChainCatalog.meta(chainId)?.nativeSymbol ?? "ETH", 18)
        }
        if let meta = await tokens?.resolve(chainId: chainId, addresses: [token])[token.lowercased()] {
            return (meta.symbol, meta.decimals)
        }
        return (AddressText.short(token), 18)
    }
}

extension MsTagged {
    /// A marker: the draft is drawn.
    static let drawn: MsTagged = MultisigMachine.decode(MsTagged.self, from: ["kind": "__drawn"])!
}

/// The relay key a profile may point at (`vela.peerconnectRelay`).
enum DevRelayKey {
    static let relay = "vela.peerconnectRelay"
}

/// What the root view presents for the multisig.
enum MultisigSheet: Identifiable, Equatable {
    case create
    case add(prefill: String?)
    case invite(address: String)
    case settings(address: String)
    case proposal(chainId: Int, safeTxHash: String)
    /// The proposal drafted for the person's review, before its passkey.
    case review(chainId: Int)
    case members(address: String)
    case link
    /// A site's request to a multisig, held by the proposals machine (M5).
    case dapp(id: String)

    var id: String {
        switch self {
        case .create: "create"
        case .add: "add"
        case .invite(let a): "invite:\(a)"
        case .settings(let a): "settings:\(a)"
        case .proposal(let c, let h): "proposal:\(c):\(h.lowercased())"
        case .review(let c): "review:\(c)"
        case .members(let a): "members:\(a)"
        case .link: "link"
        case .dapp(let id): "dapp:\(id)"
        }
    }
}

/// The high-risk question (§3.5): on this network something besides the
/// members can act for the multisig. The person signs anyway, or not.
@MainActor
final class MultisigRiskAsk: Identifiable {
    let id = UUID()
    let chainId: Int
    let safe: String
    let findings: [MsJSON]
    private var resume: ((Bool) -> Void)?

    init(chainId: Int, safe: String, findings: [MsJSON], resume: @escaping (Bool) -> Void) {
        self.chainId = chainId
        self.safe = safe
        self.findings = findings
        self.resume = resume
    }

    func answer(_ signs: Bool) {
        let resume = self.resume
        self.resume = nil
        resume?(signs)
    }
}
