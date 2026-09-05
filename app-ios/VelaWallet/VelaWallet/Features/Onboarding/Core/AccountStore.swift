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

    /// Read the account list. Order is the core's, never re-sorted here.
    func loadAccounts() -> [[String: Any]] { readList(Key.accounts) }

    /// Upsert by id. The whole record is written — see the invariant above.
    func saveAccount(_ account: [String: Any]) {
        let id = account["id"] as? String ?? ""
        var accounts = loadAccounts()
        if let at = accounts.firstIndex(where: { ($0["id"] as? String) == id }) {
            accounts[at] = account
        } else {
            accounts.append(account)
        }
        writeList(Key.accounts, accounts)
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
        defaults.removeObject(forKey: Key.accounts)
        defaults.removeObject(forKey: Key.activeIndex)
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

    /// Corrupt JSON reads as an empty list rather than throwing.
    ///
    /// A damaged list must not make the wallet permanently unopenable, and every
    /// write replaces the whole list anyway — but the wallet itself is not lost
    /// either way: its address derives from the passkey, so signing in rebuilds
    /// the record.
    private func readList(_ key: String) -> [[String: Any]] {
        guard let raw = defaults.string(forKey: key),
              let data = raw.data(using: .utf8),
              let list = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
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
