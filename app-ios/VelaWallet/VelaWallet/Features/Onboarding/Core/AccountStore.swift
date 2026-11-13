//
//  AccountStore.swift
//  VelaWallet
//
//  On-device storage for the wallet's account list.
//
//  Keys and record shapes are byte-compatible with the other three clients
//  (data-model §6), so a person who created a wallet on the web or the desktop
//  is not stranded here — and, more sharply, so the SAME wallet reads back the
//  same on all four.
//
//  ONE invariant governs every function below. `Account` carries both the legacy
//  scalar key fields and the full `keys` array, and the core derives the address
//  from **all** keys. A mapper that copies an account field by field and drops
//  `keys` does not merely lose data — it silently "repairs" a multi-key account
//  into a different, wrong, single-key Safe on the next restore, at an address
//  nothing can deploy. So nothing here reshapes an account: **records go in and
//  come out whole**, as the JSON the core emitted. That is why the store's
//  vocabulary is `[String: Any]` rather than a Swift struct — a struct is
//  exactly the shape that invites a field-by-field copy.
//

import Foundation
import Observation
import VelaCore

/// §18 item 2: how many stored account records this device cannot read, as of
/// the last read or write — what the welcome and the account switcher say
/// instead of looking signed out (the desktop's `storage::unreadable_accounts`,
/// Android's `AccountStore.unreadable`). Process-wide, as the records are.
@MainActor @Observable
final class StoredAccountsWatch {
    static let shared = StoredAccountsWatch()
    var unreadable = 0
}

/// §18 item 2: when the welcome says stored accounts could not be read — the
/// core's `accounts_unreadable` (the read itself failed), or a stored record
/// this device cannot read (kept as stored). Pure.
enum StoredAccountsNotice {
    static func shown(sessionUnreadable: Bool, storedUnreadable: Int) -> Bool {
        sessionUnreadable || storedUnreadable > 0
    }
}

actor AccountStore {

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    private enum Key {
        static let accounts = VelaStore.Key.accounts
        static let activeIndex = VelaStore.Key.activeIndex
        static let pendingUploads = VelaStore.Key.pendingUploads
        static let serviceEndpoints = VelaStore.Key.serviceEndpoints
    }

    /// Read the account list: the records the core reads, exactly as stored,
    /// in the stored order. §18 item 2: a record this build cannot read — or a
    /// stored value that is no list — is counted (`unreadable`) and kept,
    /// never dropped; the welcome and the account switcher say it. The rule
    /// is the core's (`stored_accounts::read`), the same on every shell.
    func loadAccounts() -> [[String: Any]] {
        let answer = Self.object(storedAccountsRead(raw: rawAccounts()))
        let count = (answer["unreadable"] as? NSNumber)?.intValue ?? 0
        noteUnreadable(count, at: answer["unreadable_at"] as? [[String: Any]])
        return answer["accounts"] as? [[String: Any]] ?? []
    }

    /// Upsert by id. The whole record is written — see the invariant above —
    /// and every record this build cannot read stays where it was, its bytes
    /// as they were (§18 item 2, the core's `stored_accounts::upsert`): a list
    /// shortened by what could not be read is never written. Signing in again
    /// is the way back: it writes the account's record beside them.
    func saveAccount(_ account: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: account),
              let text = String(data: data, encoding: .utf8)
        else { return }
        write(storedAccountsUpsert(raw: rawAccounts(), accountJson: text), "upsert")
    }

    /// How many stored records the last read or write could not read.
    private(set) var unreadable = 0

    /// The stored list as stored: a string, or — written by something else —
    /// whatever JSON the value holds, never read as "nothing".
    private func rawAccounts() -> String? {
        if let text = defaults.string(forKey: Key.accounts) { return text }
        guard let object = defaults.object(forKey: Key.accounts) else { return nil }
        if JSONSerialization.isValidJSONObject(object),
           let data = try? JSONSerialization.data(withJSONObject: object) {
            return String(data: data, encoding: .utf8)
        }
        return String(describing: object)
    }

    private static func object(_ json: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any] ?? [:]
    }

    /// Store the core's answer to a write, and log its counts. `null` (a
    /// record with no `id`) writes nothing — never an empty list.
    private func write(_ answer: String, _ what: String) {
        let write = Self.object(answer)
        guard let stored = write["stored"] as? String else {
            VelaLog.notice(.accounts, "\(what) refused: the record cannot be written")
            return
        }
        let n = { (key: String) in (write[key] as? NSNumber)?.intValue ?? 0 }
        // ANDROID-8's lesson, on this store too: every write leaves a count.
        VelaLog.notice(
            .accounts,
            "\(what) before=\(n("before")) after=\(n("after")) unreadable_kept=\(n("unreadable_kept"))"
        )
        defaults.set(stored, forKey: Key.accounts)
        noteUnreadable(n("unreadable_kept"), at: nil)
    }

    /// Keep the count, and log it once as it changes: positions and sizes,
    /// never contents (a record holds addresses and public keys).
    private func noteUnreadable(_ count: Int, at: [[String: Any]]?) {
        let was = unreadable
        unreadable = count
        lastReadFailed = count > 0
        if count != was, count > 0 {
            let where_ = (at ?? []).map {
                "#\(($0["position"] as? NSNumber)?.intValue ?? -1) (\(($0["bytes"] as? NSNumber)?.intValue ?? 0) bytes)"
            }.joined(separator: ", ")
            VelaLog.notice(.accounts, "\(count) stored record(s) could not be read and are kept as stored: \(where_)")
        }
        Task { @MainActor in StoredAccountsWatch.shared.unreadable = count }
    }

    /// Remove exactly one record, by id.
    ///
    /// Narrow on purpose. The only caller is the parallel space's exit (spec
    /// 052 FR-003), and the alternative it exists to prevent is a caller
    /// rewriting the whole list: this door is opened on a phone that holds the
    /// founder's real wallet, and a list replacement there loses it. A record
    /// that is not there is not an error — leaving twice is leaving once.
    func removeAccount(id: String) {
        // §18 item 2: through the core, every other record — one this build
        // cannot read too — kept as it was.
        guard let by = Self.json(["id": id]) else { return }
        let answer = storedAccountsRemove(raw: rawAccounts(), byJson: by)
        let counts = Self.object(answer)
        // Nothing named, nothing written.
        guard (counts["after"] as? NSNumber)?.intValue != (counts["before"] as? NSNumber)?.intValue else { return }
        write(answer, "remove fixture")
    }

    private static func json(_ object: [String: Any]) -> String? {
        (try? JSONSerialization.data(withJSONObject: object)).flatMap { String(data: $0, encoding: .utf8) }
    }

    /// Missing, garbage and negative all read as 0.
    ///
    /// A negative index would make the session render an empty address with a
    /// wallet present, which the core forbids — so it fails closed here rather
    /// than arriving at the wire.
    func loadActiveIndex() -> Int {
        let raw = defaults.integer(forKey: Key.activeIndex)
        return raw > 0 ? raw : 0
    }

    func saveActiveIndex(_ index: Int) {
        defaults.set(max(0, index), forKey: Key.activeIndex)
    }

    func loadPendingUploads() -> [[String: Any]] { readList(Key.pendingUploads) }

    /// Keyed by `id`, which for a pending upload IS the credential id of its
    /// first founding key — the scalar fields mirror `members[0]`.
    func savePendingUpload(_ record: [String: Any]) {
        let id = record["id"] as? String ?? ""
        var kept = loadPendingUploads().filter { ($0["id"] as? String) != id }
        kept.append(record)
        writeList(Key.pendingUploads, kept)
    }

    func removePendingUpload(credentialIdHex: String) {
        writeList(
            Key.pendingUploads,
            loadPendingUploads().filter { ($0["id"] as? String) != credentialIdHex }
        )
    }

    func hasPendingUploads() -> Bool { !loadPendingUploads().isEmpty }

    /// Forget which wallet this device is signed into — the account list and the
    /// active index, and NOTHING else.
    ///
    /// The scope is the decision, not an implementation detail. Contacts,
    /// history, custom tokens and networks, endpoints and preferences belong to
    /// the ACCOUNT rather than to the session, and the account comes back intact
    /// because its address derives from the passkey rather than from disk. The
    /// pending-upload outbox is excluded for a second, independent reason: a
    /// record there is a public key the registry never confirmed, and the next
    /// launch can still retry it — but a deleted record can never be retried,
    /// and that credential becomes unfindable at sign-in.
    func clearSignedInWallet() {
        // A sign-out empties the list on purpose — said in the log, so an
        // empty list later reads as that and not as a lost record.
        let stored = Self.object(storedAccountsRead(raw: rawAccounts()))
        let before = ((stored["accounts"] as? [Any])?.count ?? 0) + ((stored["unreadable"] as? NSNumber)?.intValue ?? 0)
        VelaLog.notice(.accounts, "sign out before=\(before) after=0")
        defaults.removeObject(forKey: Key.accounts)
        defaults.removeObject(forKey: Key.activeIndex)
        noteUnreadable(0, at: nil)
    }

    /// Drop ONE account from the stored list, by ADDRESS — spec 017's narrow
    /// half (2026-09-23: 「有时候不想退出所有，只想退出单个」).
    ///
    /// By address, not by id or position, because a row's identity is its
    /// address (session invariant ⑨): a write that raced a re-sorted display
    /// must not take a stranger. `removeAccount(id:)` above is a different job
    /// — the parallel space dropping the record it appended, which it knows by
    /// id. A row that is no longer there is a no-op: the person asked for it to
    /// be gone, and it is.
    func removeAccount(address: String) {
        // §18 item 2: through the core — a record this build cannot read stays.
        guard let by = Self.json(["address": address]) else { return }
        write(storedAccountsRemove(raw: rawAccounts(), byJson: by), "remove")
    }

    // MARK: - `vela.serviceEndpoints`, which has two writers

    /// The whole endpoints blob, as stored — camelCase, partial, absent fields
    /// absent.
    ///
    /// This key is the one place onboarding and `network_admin` (spec 050) both
    /// write, so **both go through here**. Two independent writers on one key is
    /// how a person's custom passkey-index endpoint disappears the first time
    /// they open 设置 → 端点 — silently, because both writes succeed.
    func loadServiceEndpoints() -> [String: Any] {
        guard let raw = defaults.string(forKey: Key.serviceEndpoints),
              let data = raw.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return [:] }
        return object
    }

    /// Merge fields in; a `nil` value removes its field.
    ///
    /// Merging rather than replacing is the whole point: a caller that only
    /// knows about one endpoint must not erase the other three.
    func saveServiceEndpoints(_ fields: [String: String?]) {
        var endpoints = loadServiceEndpoints()
        for (name, value) in fields {
            if let value, !value.isEmpty {
                endpoints[name] = value
            } else {
                endpoints.removeValue(forKey: name)
            }
        }
        if let data = try? JSONSerialization.data(withJSONObject: endpoints),
           let text = String(data: data, encoding: .utf8) {
            defaults.set(text, forKey: Key.serviceEndpoints)
        }
    }

    /// The passkey-index endpoint override, when the person set one.
    func loadRegistryURL() -> String? {
        let url = loadServiceEndpoints()["passkeyIndexURL"] as? String
        return (url?.isEmpty ?? true) ? nil : url
    }

    func saveRegistryURL(_ url: String?) {
        saveServiceEndpoints(["passkeyIndexURL": url])
    }

    // MARK: - Raw access

    /// Whether the last read of `vela.accounts` found a record it could not
    /// read, as opposed to finding nothing (§18 item 2: `unreadable > 0`).
    ///
    /// **They are not the same fact and a wallet must never confuse them**
    /// (ANDROID-8, 2026-09-13: a test device's own account record went
    /// missing). "No accounts" sends somebody to the create-a-wallet screen;
    /// "this device's wallet data cannot be read" is a fault they can act on —
    /// retry, or reset this device's copy. Reading the second as the first is
    /// how a person is told they have no wallet.
    private(set) var lastReadFailed = false

    /// The pending-upload outbox's list (the account list is the core's to
    /// read, above). Corrupt JSON reads as empty.
    private func readList(_ key: String) -> [[String: Any]] {
        guard let raw = defaults.string(forKey: key),
              let data = raw.data(using: .utf8),
              let list = (try? JSONSerialization.jsonObject(with: data)) as? [[String: Any]]
        else { return [] }
        return list
    }

    private func writeList(_ key: String, _ value: [[String: Any]]) {
        guard let data = try? JSONSerialization.data(withJSONObject: value),
              let text = String(data: data, encoding: .utf8)
        else { return }
        defaults.set(text, forKey: key)
    }
}
