//
//  SessionExecutor.swift
//  VelaWallet
//
//  The session machine's eleven operations.
//
//  A separate vocabulary from onboarding's eighteen, and a separate executor,
//  because the session machine is **app-resident** — one per process, outliving
//  every screen — while an onboarding core exists only for the length of a flow.
//
//  The writes are best effort by contract: the session is already in the state
//  the write was meant to record, and a failed write cannot put it back.
//
//  Three belong to the landing watch (issue #409): a one-key wallet is entered
//  at the registry's 202, and the session — still here when the create screen
//  is gone — reads the outbox, waits on each accepted record's registry task,
//  and removes a record once its task has landed. A read and a local write; no
//  passkey anywhere.
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
        "load_pending_uploads",
        "await_registry_landing",
        "remove_pending_upload",
    ]

    private let store: AccountStore
    /// How the landing watch reaches the registry — the app's session, or a
    /// test's answers from memory.
    private let registryTransport: RegistryClient.Transport

    init(store: AccountStore, registryTransport: @escaping RegistryClient.Transport = RegistryClient.urlSession) {
        self.store = store
        self.registryTransport = registryTransport
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

        // Issue #409 — the landing watch. The outbox goes over as stored; the
        // core decides which records a read can settle, and a record it cannot
        // read costs only itself.
        case "load_pending_uploads":
            return CoreJSON.string([
                "type": "pending_uploads_loaded",
                "records": await store.loadPendingUploads(),
            ])

        // The same poll, interval and budget the create's publish always
        // waited with — now after "Wallet created" instead of before it, at the
        // endpoint Settings names.
        case "await_registry_landing":
            let registry = RegistryClient(
                baseURL: await store.loadRegistryURL() ?? RegistryClient.defaultURL,
                transport: registryTransport
            )
            do {
                try await registry.awaitTask(id: operation["task_id"] as? String ?? "")
                return CoreJSON.string(["type": "registry_landed"])
            } catch {
                return CoreJSON.string([
                    "type": "registry_landing_unconfirmed",
                    "message": (error as? RegistryFailure)?.message ?? error.localizedDescription,
                ])
            }

        // Only ever after `registry_landed` — the core's rule. Best effort: a
        // record that survives is confirmed again next launch.
        case "remove_pending_upload":
            await store.removePendingUpload(credentialIdHex: operation["credential_id"] as? String ?? "")
            return CoreJSON.string(["type": "pending_upload_removed"])

        default:
            // Fail closed: an unknown session operation must not silently
            // succeed. `accounts_unavailable` is the variant that leaves the app
            // in onboarding rather than in a wallet it cannot prove exists.
            return CoreJSON.string(["type": "accounts_unavailable"])
        }
    }
}
