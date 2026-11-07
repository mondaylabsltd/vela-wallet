//
//  SessionExecutor.swift
//  VelaWallet
//
//  The session machine's twelve operations.
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
import VelaCore

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
        "publish_pending_upload",
    ]

    private let store: AccountStore
    /// How the landing watch reaches the registry — the app's session, or a
    /// test's answers from memory.
    private let registryTransport: RegistryClient.Transport
    /// The wallet's own relying party — a replayed unit filed under no page
    /// is this domain's, as the create's was.
    private let relyingPartyId: String

    init(
        store: AccountStore,
        registryTransport: @escaping RegistryClient.Transport = RegistryClient.urlSession,
        relyingPartyId: String = PasskeyExecutor.relyingParty
    ) {
        self.store = store
        self.registryTransport = registryTransport
        self.relyingPartyId = relyingPartyId
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
                return CoreJSON.string(Self.landingUnconfirmed(error))
            }

        // Only ever after `registry_landed` — the core's rule. Best effort: a
        // record that survives is confirmed again next launch.
        case "remove_pending_upload":
            await store.removePendingUpload(credentialIdHex: operation["credential_id"] as? String ?? "")
            return CoreJSON.string(["type": "pending_upload_removed"])

        // ob-2: a wallet's publish that never landed, replayed as the create
        // would have sent it. Every member carries its creation-time proof,
        // so no authenticator is asked; it waits for the landing on the
        // registry's own budget, like the wait above. Never answered at
        // acceptance — the record goes only once the group landed.
        case "publish_pending_upload":
            let registry = RegistryClient(
                baseURL: await store.loadRegistryURL() ?? RegistryClient.defaultURL,
                transport: registryTransport
            )
            do {
                try await Self.replay(
                    operation["publish"] as? [String: Any] ?? [:],
                    registry: registry,
                    walletRpId: relyingPartyId
                )
                return CoreJSON.string(["type": "registry_landed"])
            } catch {
                return CoreJSON.string(Self.landingUnconfirmed(error))
            }

        default:
            // Fail closed: an unknown session operation must not silently
            // succeed. `accounts_unavailable` is the variant that leaves the app
            // in onboarding rather than in a wallet it cannot prove exists.
            return CoreJSON.string(["type": "accounts_unavailable"])
        }
    }

    /// The replayed publish (ob-2), under the relying party the create would
    /// have used: the domain of the page the unit is filed under, else the
    /// wallet's own. A member without its creation proof cannot be replayed
    /// without a prompt, so it is refused here rather than signed live — that
    /// is a sign-in's publish, not this one.
    /// The landing not confirmed, and — r18-8 (§28) — whether this test
    /// build refused the publish before it left (the session then never asks
    /// again by itself).
    static func landingUnconfirmed(_ error: Error) -> [String: Any] {
        let failure = error as? RegistryFailure
        return [
            "type": "registry_landing_unconfirmed",
            "message": failure?.message ?? error.localizedDescription,
            "refused": failure?.refused ?? TestBuildRefusal.matches(error),
        ]
    }

    static func replay(
        _ pending: [String: Any],
        registry: RegistryClient,
        walletRpId: String
    ) async throws {
        let members = (pending["members"] as? [[String: Any]] ?? []).map(PublishMember.init(json:))
        guard !members.isEmpty else {
            throw RegistryFailure(message: "the pending publish has no member", network: false)
        }
        let proven = try members.map { member -> ProvenMember in
            guard let proof = member.proof else {
                throw RegistryFailure(message: "a pending member carries no creation proof", network: false)
            }
            return ProvenMember(member: member, proof: proof)
        }
        let seedHex = pending["group_seed_hex"] as? String ?? ""
        let groupPublicKey = pending["group_public_key_hex"] as? String ?? ""
        guard !seedHex.isEmpty, !groupPublicKey.isEmpty else {
            throw RegistryFailure(message: "the pending publish carries no group key", network: false)
        }
        let metadataHex = pending["metadata_hex"] as? String ?? ""
        let page = (pending["page"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        let rpId = trustedSignerRegistryRpId(signerOrigin: page) ?? walletRpId

        let challenge = try await registry.groupChallenge(
            metadataHex: metadataHex, groupPublicKey: groupPublicKey, members: members, rpId: rpId
        )
        let groupJSON = try registryBuildGroupProof(
            seedHex: seedHex, rpId: rpId, challengeHex: challenge.groupChallenge
        )
        guard let group = try? CoreJSON.object(groupJSON),
              let groupProof = group["proof"] as? [String: Any]
        else {
            throw RegistryFailure(message: "the group proof did not parse", network: false)
        }
        let ack = try await registry.registerGroup(
            metadataHex: metadataHex,
            groupPublicKey: groupPublicKey,
            groupProof: groupProof,
            members: proven,
            rpId: rpId
        )
        switch try OnboardingExecutor.afterRegister(ack, answerWhenAccepted: false) {
        case .landed, .accepted:
            return
        case .awaitLanding(let taskId):
            try await registry.awaitTask(id: taskId)
        }
    }
}
