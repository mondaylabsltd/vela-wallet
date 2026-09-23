//
//  SessionExecutor.swift
//  VelaWallet
//
//  The session machine's seven operations.
//
//  A separate vocabulary from onboarding's eighteen, and a separate executor,
//  because the session machine is **app-resident** — one per process, outliving
//  every screen — while an onboarding core exists only for the length of a flow.
//
//  Five of the seven are best effort by contract: the session is already in the
//  state the write was meant to record, and a failed write cannot put it back.
//

import Foundation

@MainActor
final class SessionExecutor {

    /// Every session operation this executor is required to handle (contract §2).
    static let operations = [
        "load_accounts",
        "load_active_index",
        "save_account",
        "save_active_index",
        "check_pending_uploads",
        "remove_account",
        "clear_signed_in_wallet",
        "clear_extension_cache",
    ]

    private let store: AccountStore

    init(store: AccountStore) {
        self.store = store
    }

    func perform(_ operation: [String: Any]) async -> String {
        let type = operation["type"] as? String ?? ""
        switch type {
        case "load_accounts":
            return CoreJSON.string(["type": "accounts_loaded", "accounts": await store.loadAccounts()])

        case "load_active_index":
            return CoreJSON.string(["type": "active_index_loaded", "index": await store.loadActiveIndex()])

        // A best-effort migration write-back. If it fails, the in-memory
        // correction the core made still stands.
        case "save_account":
            await store.saveAccount(operation["account"] as? [String: Any] ?? [:])
            return CoreJSON.string(["type": "account_saved"])

        case "save_active_index":
            await store.saveActiveIndex((operation["index"] as? NSNumber)?.intValue ?? 0)
            return CoreJSON.string(["type": "active_index_saved"])

        case "check_pending_uploads":
            return CoreJSON.string([
                "type": "pending_uploads",
                "has_pending": await store.hasPendingUploads(),
            ])

        case "remove_account":
            await store.removeAccount(address: operation["address"] as? String ?? "")
            return CoreJSON.string(["type": "account_removed"])

        case "clear_signed_in_wallet":
            await store.clearSignedInWallet()
            return CoreJSON.string(["type": "signed_in_wallet_cleared"])

        // The Safari extension's account snapshot is the iOS artifact this
        // operation exists for. It has no shared store yet, so this is an
        // honest no-op — answered rather than skipped, because the core is
        // waiting for the ack and would otherwise never leave the sign-out.
        case "clear_extension_cache":
            return CoreJSON.string(["type": "extension_cache_cleared"])

        default:
            // Fail closed: an unknown session operation must not silently
            // succeed. `accounts_unavailable` is the variant that leaves the app
            // in onboarding rather than in a wallet it cannot prove exists.
            return CoreJSON.string(["type": "accounts_unavailable"])
        }
    }
}
