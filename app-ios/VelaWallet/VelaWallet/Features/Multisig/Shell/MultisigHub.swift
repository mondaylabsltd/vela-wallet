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
        if proposalsView.paceMs != before { restartProposalTimer() }
        // The member changes waiting, as the members machine reads them.
        let waiting = proposalsView.items.map { item -> [String: Any] in
            [
                "chain_id": item.chainId, "safe_tx_hash": item.safeTxHash, "nonce": item.nonce,
                "action": item.action?.any ?? NSNull(), "approved": item.approved,
                "threshold": item.threshold, "status": item.status,
            ]
        }
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
        if active {
            accounts.dispatch(["type": "tick"])
            proposals.dispatch(["type": "refresh", "fresh": false])
            proposals.dispatch(["type": "online"])
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

    // MARK: - Verbs (proposals)

    func sendProposal(_ event: [String: Any]) { proposals.dispatch(event) }

    /// Propose `proposal` on `chainId` for the multisig shown: drawn with its
    /// review first (`draft`); the person confirms it on the review sheet
    /// (`confirmDraft`). Answers once the machine settled it (`done`: its
    /// safeTxHash) or why not.
    func propose(chainId: Int, proposal: [String: Any], address: String? = nil) async -> MsTagged? {
        if let address, address.lowercased() != followed?.lowercased(), let record = record(address) {
            proposals.dispatch(["type": "follow", "account": record])
            followed = address
        }
        proposals.dispatch(["type": "draft", "chain_id": chainId, "proposal": proposal])
        let drafted: MsTagged? = await proposals.until { view in
            if view["draft"] is [String: Any] { return MsTagged.drawn }
            guard let state = (view["actions"] as? [String: Any])?["propose"] else { return nil }
            let tagged = MultisigMachine.decode(MsTagged.self, from: state)
            return tagged?.kind == "working" ? nil : tagged
        }
        guard drafted?.kind == MsTagged.drawn.kind else { return drafted }
        sheet = .review(chainId: chainId)
        // The person's word on its review: confirmed (working), or withdrawn.
        let settled: MsTagged? = await proposals.until { view in
            guard let state = (view["actions"] as? [String: Any])?["propose"] else {
                return view["draft"] is [String: Any] ? nil : MultisigMachine.decode(MsTagged.self, from: ["kind": "cancelled"])
            }
            let tagged = MultisigMachine.decode(MsTagged.self, from: state)
            switch tagged?.kind {
            case "working", "outsideAsked": return nil
            default: return view["draft"] is [String: Any] && tagged?.kind != "done" ? nil : tagged
            }
        }
        return settled
    }

    func confirmDraft(as member: String?) {
        proposals.dispatch(["type": "confirm_draft", "as_member": member ?? NSNull()])
    }

    func withdrawDraft() {
        proposals.dispatch(["type": "withdraw"])
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
                "type": "open_linked", "safe": address, "chain_id": chainId, "safe_tx_hash": hash,
                "preview": notice["preview"] as? Bool ?? false,
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
