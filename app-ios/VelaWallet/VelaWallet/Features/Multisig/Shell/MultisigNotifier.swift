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
//  created or added (by hand, from an invite, or found as a member here), or
//  the first proposal made here, which needs the others (`askOnce`). A
//  refusal is answered; nothing asks again.
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
        /// r11-1: per multisig, the transactions already said (or seen on
        /// screen) as needing this device / ready — the core's keys.
        var needsYouKeys: [String: [String]] = [:]
        var readyKeys: [String: [String]] = [:]

        init() {}

        /// A record kept by an older build (no keys) still reads.
        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            needsYou = try c.decodeIfPresent([String: Int].self, forKey: .needsYou) ?? [:]
            ready = try c.decodeIfPresent([String: Int].self, forKey: .ready) ?? [:]
            approved = try c.decodeIfPresent([String: Int].self, forKey: .approved) ?? [:]
            needsYouKeys = try c.decodeIfPresent([String: [String]].self, forKey: .needsYouKeys) ?? [:]
            readyKeys = try c.decodeIfPresent([String: [String]].self, forKey: .readyKeys) ?? [:]
        }
    }

    /// The most keys kept per multisig and kind (the oldest go first).
    static let keptKeys = 200

    /// r11-1: what a presence names that was not said before — by key when
    /// the core names them, else (an older core) by a rise in the count.
    static func news(keys: [String], count: Int, saidKeys: [String]?, saidCount: Int?) -> [String]? {
        if !keys.isEmpty || count == 0 {
            let said = Set(saidKeys ?? [])
            let fresh = keys.filter { !said.contains($0) }
            return fresh.isEmpty ? nil : fresh
        }
        return count > (saidCount ?? 0) ? [] : nil
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
    /// Whether the app is in the background — not merely inactive under a
    /// system sheet or Control Center (r9-8: a read landing then is said).
    var inBackground: () -> Bool = { UIApplication.shared.applicationState == .background }

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
        let before = seen
        var state = before
        var notes: [Note] = []
        for row in accounts.accounts {
            let key = row.account.address.lowercased()
            guard let waiting = proposals.presence[key] ?? nil else { continue }
            // r11-1: a list kept from an earlier page is no news, and is not
            // recorded as said either (a round that evaluated a kept list
            // recorded a proposal it never announced).
            let keyed = !waiting.needsYouKeys.isEmpty || !waiting.readyKeys.isEmpty
            if keyed && waiting.readAt == nil { continue }
            let name = words.name(of: row.account, rowName: row.name)
            let first = waiting.first
            // A note opens the first NEW transaction it names, else the first.
            func target(_ fresh: [String]) -> (chain: Int?, hash: String?) {
                if let one = fresh.first, let colon = one.firstIndex(of: ":") {
                    return (Int(one[..<colon]), String(one[one.index(after: colon)...]))
                }
                return (first?["chain_id"]?.int ?? first?["chainId"]?.int,
                        first?["safe_tx_hash"]?.string ?? first?["safeTxHash"]?.string)
            }
            if let fresh = Self.news(keys: waiting.needsYouKeys, count: waiting.needsYou,
                                     saidKeys: state.needsYouKeys[key], saidCount: state.needsYou[key]) {
                let (chain, hash) = target(fresh)
                notes.append(Note(
                    // The new transaction (and the count) in the id: a request
                    // with the id of one still in Notification Center replaces
                    // it without a banner, and a second new proposal went unsaid.
                    id: "ms-needs-\(key)-\(hash ?? "")-\(waiting.needsYou)", title: name,
                    body: words.t("presence.needsYou", count: waiting.needsYou, ["count": String(waiting.needsYou)]),
                    chainId: chain, safeTxHash: hash, address: row.account.address
                ))
            }
            if let fresh = Self.news(keys: waiting.readyKeys, count: waiting.ready,
                                     saidKeys: state.readyKeys[key], saidCount: state.ready[key]) {
                let (chain, hash) = target(fresh)
                notes.append(Note(
                    id: "ms-ready-\(key)-\(hash ?? "")-\(waiting.ready)", title: name,
                    body: words.t("presence.ready", count: waiting.ready, ["count": String(waiting.ready)]),
                    chainId: chain, safeTxHash: hash, address: row.account.address
                ))
            }
            state.needsYou[key] = waiting.needsYou
            state.ready[key] = waiting.ready
            state.needsYouKeys[key] = Array(((state.needsYouKeys[key] ?? []).filter { !waiting.needsYouKeys.contains($0) }
                + waiting.needsYouKeys).suffix(Self.keptKeys))
            state.readyKeys[key] = Array(((state.readyKeys[key] ?? []).filter { !waiting.readyKeys.contains($0) }
                + waiting.readyKeys).suffix(Self.keptKeys))
        }
        // The multisig followed: an approval counted on a transaction this
        // device has approved (another member's, while it was away).
        let followedName = accounts.row(proposals.followed).map { words.name(of: $0.account, rowName: $0.name) } ?? ""
        for item in proposals.items where item.status != "done" {
            let before = state.approved[item.id]
            if let before, item.approved > before {
                notes.append(Note(
                    id: "ms-counted-\(item.id)-\(item.approved)", title: followedName,
                    body: "\(words.title(item, safe: proposals.followed)) · "
                        + words.t("status.progress", ["approved": String(item.approved), "threshold": String(item.threshold)]),
                    chainId: item.chainId, safeTxHash: item.safeTxHash, address: proposals.followed
                ))
            }
            state.approved[item.id] = item.approved
        }
        if state != before { seen = state }
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
