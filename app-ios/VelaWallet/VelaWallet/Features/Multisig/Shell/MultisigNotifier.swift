//
//  MultisigNotifier.swift
//  VelaWallet
//
//  The multisig's notifications (M6, AUDIT §6 decision 3): local background
//  polling now — a `BGAppRefreshTask` wakes the app, the proposals machine
//  reads every multisig here as it does on screen (`others_tick`, the
//  followed list's `refresh`) — and a local notification when something
//  changed for this device: a transaction needs its approval, an approval
//  was counted, one is ready to execute, one was executed. No push service
//  and no server learns anything: what changed is read from the mailbox and
//  the chain exactly as the screen reads it, and decided by the same core.
//
//  ## When it asks
//
//  Not at launch: when there is first something to say — the first multisig
//  created or added here, or the first proposal made (`askOnce`). A refusal
//  is answered; nothing asks again.
//
//  ## Only when away
//
//  The core's views drive the screen while the app is up; a banner over it is
//  noise (the shared notification delegate presents nothing in front).
//

import BackgroundTasks
import Foundation
import UIKit
import UserNotifications

@MainActor
final class MultisigNotifier {
    /// The identifier `Info.plist` permits and the scene registers.
    static let task = "app.getvela.VelaWallet.multisig"
    /// What was last said, per multisig and transaction (so nothing is said twice).
    static let stateKey = "vela.multisigNotified.v1"

    struct Seen: Codable, Equatable {
        var needsYou: [String: Int] = [:]
        var ready: [String: Int] = [:]
        /// Approvals counted per `chainId:hash`, for the multisig followed.
        var approved: [String: Int] = [:]
    }

    /// One notification to post (testable without the center).
    struct Note: Equatable {
        let id: String
        let title: String
        let body: String
        let chainId: Int?
        let safeTxHash: String?
        var address: String? = nil
    }

    private let loc: Loc
    private let defaults: UserDefaults
    /// Posts a note (the system center; a test's list).
    var post: (Note) -> Void = MultisigNotifier.postToCenter
    /// Whether the app is away from the screen (notes are only posted then).
    var isAway: () -> Bool = { UIApplicationStateProbe.isAway }

    init(loc: Loc, defaults: UserDefaults = .standard) {
        self.loc = loc
        self.defaults = defaults
    }

    // MARK: - Asking

    /// The first time there is something to say: ask once (the system's own
    /// "not determined" is the once).
    static func askOnce() {
        let center = UNUserNotificationCenter.current()
        center.getNotificationSettings { settings in
            guard settings.authorizationStatus == .notDetermined else { return }
            center.requestAuthorization(options: [.alert, .sound]) { _, _ in }
        }
    }

    // MARK: - Deciding what to say

    var seen: Seen {
        get {
            guard let data = defaults.data(forKey: Self.stateKey),
                  let seen = try? JSONDecoder().decode(Seen.self, from: data)
            else { return Seen() }
            return seen
        }
        set {
            if let data = try? JSONEncoder().encode(newValue) { defaults.set(data, forKey: Self.stateKey) }
        }
    }

    /// What the machines' views say now, against what was said last: the
    /// notes to post. `post` false records the state without saying anything
    /// (the app is on screen: the views said it).
    @discardableResult
    func evaluate(accounts: MsAccountsView, proposals: MsProposalsView, words: MsWords, post: Bool) -> [Note] {
        var state = seen
        var notes: [Note] = []
        for row in accounts.accounts {
            let key = row.account.address.lowercased()
            guard let waiting = proposals.presence[key] ?? nil else { continue }
            let name = words.name(of: row.account, rowName: row.name)
            let first = waiting.first
            let chain = first?["chain_id"]?.int ?? first?["chainId"]?.int
            let hash = first?["safe_tx_hash"]?.string ?? first?["safeTxHash"]?.string
            if waiting.needsYou > (state.needsYou[key] ?? 0) {
                notes.append(Note(
                    id: "ms-needs-\(key)-\(hash ?? "")", title: name,
                    body: words.t("presence.needsYou", count: waiting.needsYou, ["count": String(waiting.needsYou)]),
                    chainId: chain, safeTxHash: hash, address: row.account.address
                ))
            }
            if waiting.ready > (state.ready[key] ?? 0) {
                notes.append(Note(
                    id: "ms-ready-\(key)-\(hash ?? "")", title: name,
                    body: words.t("presence.ready", count: waiting.ready, ["count": String(waiting.ready)]),
                    chainId: chain, safeTxHash: hash, address: row.account.address
                ))
            }
            state.needsYou[key] = waiting.needsYou
            state.ready[key] = waiting.ready
        }
        // The multisig followed: an approval counted on a transaction this
        // device has approved (another member's, while it was away).
        let followedName = accounts.row(proposals.followed).map { words.name(of: $0.account, rowName: $0.name) } ?? ""
        for item in proposals.items where item.status != "done" {
            let before = state.approved[item.id]
            if let before, item.approved > before {
                notes.append(Note(
                    id: "ms-counted-\(item.id)-\(item.approved)", title: followedName,
                    body: "\(words.actionTitleNow(chainId: item.chainId, action: item.action, nonce: item.nonce)) · "
                        + words.t("status.progress", ["approved": String(item.approved), "threshold": String(item.threshold)]),
                    chainId: item.chainId, safeTxHash: item.safeTxHash, address: proposals.followed
                ))
            }
            state.approved[item.id] = item.approved
        }
        seen = state
        guard post else { return [] }
        notes.forEach(self.post)
        return notes
    }

    /// An executed transaction (the proposals machine's `activity_rows`).
    func executed(title: String, body: String, chainId: Int, safeTxHash: String) {
        guard isAway() else { return }
        post(Note(id: "ms-executed-\(chainId)-\(safeTxHash.lowercased())", title: title, body: body,
                  chainId: chainId, safeTxHash: safeTxHash))
    }

    static func postToCenter(_ note: Note) {
        let content = UNMutableNotificationContent()
        content.title = note.title
        content.body = note.body
        content.sound = .default
        content.threadIdentifier = "multisig"
        if let chain = note.chainId, let hash = note.safeTxHash {
            content.userInfo = [
                "vela.multisig.chain": chain, "vela.multisig.hash": hash,
                "vela.multisig.address": note.address ?? "",
            ]
        }
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: note.id, content: content, trigger: nil))
    }

    // MARK: - The background poll

    /// Ask iOS for a refresh while there is a multisig to follow.
    static func schedule() {
        let request = BGAppRefreshTaskRequest(identifier: task)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        try? BGTaskScheduler.shared.submit(request)
    }
}

/// The seam between SwiftUI's `backgroundTask` and the resident hub.
@MainActor
enum MultisigBackgroundBridge {
    static weak var hub: MultisigHub?
    /// A tapped notification: its multisig shown, its transaction's sheet open.
    static func open(userInfo: [AnyHashable: Any]) {
        guard let hub, let hash = userInfo["vela.multisig.hash"] as? String,
              let chainId = (userInfo["vela.multisig.chain"] as? NSNumber)?.intValue
        else { return }
        if let address = userInfo["vela.multisig.address"] as? String, !address.isEmpty, hub.isMultisig(address) {
            hub.activate(address)
        }
        hub.sheet = .proposal(chainId: chainId, safeTxHash: hash)
    }

    /// One background round: the machines read every multisig here, then
    /// what changed is said. Bounded: iOS gives a refresh about 30 s.
    static func run() async {
        guard let hub else { return }
        await hub.boot()
        await hub.backgroundRound()
        if !hub.accountsView.accounts.isEmpty { MultisigNotifier.schedule() }
    }
}
