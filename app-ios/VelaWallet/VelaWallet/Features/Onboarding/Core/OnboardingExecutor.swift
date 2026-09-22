//
//  OnboardingExecutor.swift
//  VelaWallet
//
//  The only place onboarding touches the outside world.
//
//  Each of the eighteen `ShellOperation`s the core declares maps to exactly one
//  call — a passkey ceremony, a storage read or write, a registry request, a
//  timer, a prompt. There is no branching on business meaning here: **if this
//  file ever grows an `if` that decides what happens next, that decision belongs
//  in the Rust machine instead.**
//
//  ## Failure contract
//
//  Nothing propagates outward. Every operation answers with the result variant
//  it owes, including for its failures, which is what lets the core own
//  classification instead of the shell pattern-matching error strings.
//
//  The mapping lives in `failure(for:error:)` rather than inline in each arm,
//  for the reason the web executor splits it out too: the classification for
//  several operations depends on WHAT THREW, not on which operation it was —
//  `sign_member_proof` can fail as a ceremony or as a registry call, and the
//  core branches differently on the two.
//
//  ## Exhaustiveness
//
//  The bridge is JSON, so the compiler cannot check this switch the way it
//  checks the desktop's `match`. `OnboardingExecutorTests` enumerates all
//  eighteen operation names against `operations` instead, and an operation this
//  file does not handle answers with a loud failure rather than silently
//  answering nothing — a silent answer would leave the core waiting forever with
//  a spinner on screen.
//

import Foundation
import VelaCore

/// The two operations whose outside world is the user interface itself.
protocol OnboardingExecutorDeps: AnyObject {
    /// Show a notice or ask a question. `confirmable` selects a two-button
    /// dialog whose answer is a business decision the core acts on; a dismissal
    /// is `false`.
    func prompt(kind: PromptKind, confirmable: Bool) async -> Bool

    /// Hand the wallet to the session machine and leave onboarding.
    func complete(mode: [String: Any]) async
}

@MainActor
final class OnboardingExecutor {

    /// Every operation this executor is required to handle (contract §1).
    static let operations = [
        "check_passkey_support",
        "register_passkey",
        "sign_proof",
        "generate_group_key",
        "sign_member_proof",
        "lookup_legacy_name",
        "authenticate_passkey",
        "load_accounts",
        "save_account",
        "save_pending_upload",
        "remove_pending_upload",
        "registry_publish",
        "registry_query_by_public_key",
        "registry_query_unit",
        "probe_index_health",
        "wait",
        "prompt",
        "complete_onboarding",
    ]

    private static let groupSeedBytes = 32
    private static let challengeBytes = 32

    private let passkey: PasskeyExecutor
    private let registry: RegistryClient
    private let store: AccountStore
    private weak var deps: OnboardingExecutorDeps?
    /// Spec 075: the fourth route. `nil` on a surface with no screen to open
    /// a page on (previews, the gallery), where choosing it fails closed.
    private let clearSigner: ClearSignerCeremonyPort?
    /// The wallet the page's card names.
    private let walletName: () -> String

    init(
        passkey: PasskeyExecutor,
        registry: RegistryClient,
        store: AccountStore,
        deps: OnboardingExecutorDeps,
        clearSigner: ClearSignerCeremonyPort? = nil,
        walletName: @escaping () -> String = { "" }
    ) {
        self.passkey = passkey
        self.registry = registry
        self.store = store
        self.deps = deps
        self.clearSigner = clearSigner
        self.walletName = walletName
    }

    /// Perform one operation and return the result JSON the core is waiting for.
    func perform(_ operation: [String: Any]) async -> String {
        do {
            return CoreJSON.string(try await run(operation))
        } catch {
            return CoreJSON.string(Self.failure(for: operation, error: error))
        }
    }

    private func run(_ operation: [String: Any]) async throws -> [String: Any] {
        let type = operation["type"] as? String ?? ""
        switch type {
        case "check_passkey_support":
            return ["type": "passkey_support", "supported": passkey.supported()]

        case "register_passkey":
            if let answer = try await onTheClearSigner(operation) {
                return ["type": "passkey_registered", "registration": answer, "now_iso": Self.nowISO()]
            }
            let registration = try await passkey.register(
                name: operation["name"] as? String ?? "",
                excludeCredentialIds: operation["exclude_credential_ids"] as? [String] ?? [],
                method: KeyMethod(rawValue: operation["method"] as? String ?? "") ?? .platform
            )
            return [
                "type": "passkey_registered",
                "registration": [
                    "credential_id": registration.credentialIdHex,
                    "attestation_object_hex": registration.attestationObjectHex,
                    "client_data_json_hex": registration.clientDataJsonHex,
                    "authenticator_attachment": registration.authenticatorAttachment,
                    "transports": registration.transports,
                ],
                "now_iso": Self.nowISO(),
            ]

        case "sign_proof":
            if let answer = try await onTheClearSigner(operation) {
                return ["type": "proof_signed", "assertion": answer, "now_iso": Self.nowISO()]
            }
            let assertion = try await passkey.assert(
                challenge: Self.challenge(for: operation["purpose"] as? String ?? ""),
                credentialIdHex: operation["credential_id"] as? String,
                transports: operation["transports"] as? String ?? "",
                // The route the person signed in with. Recovery's second
                // signature over caBLE goes back to the same phone.
                method: KeyMethod(rawValue: operation["method"] as? String ?? "") ?? .platform
            )
            return ["type": "proof_signed", "assertion": assertion.wire, "now_iso": Self.nowISO()]

        case "generate_group_key":
            // The one-time software group key — the only randomness in the flow
            // that is not a challenge, and it stays in the shell. The core only
            // echoes it into the final publish.
            let seedHex = toHex(data: PasskeyExecutor.random(Self.groupSeedBytes), prefixed: false)
            return [
                "type": "group_key_generated",
                "seed_hex": seedHex,
                "group_public_key_hex": try registryGroupPublicKeyFromSeed(seedHex: seedHex),
            ]

        case "sign_member_proof":
            // Creation-time membership confirmation: fetch the member-mode
            // challenge (it binds only groupPublicKey + own attestation, so it
            // exists before the rest of the set does), sign against exactly this
            // credential, assemble the proof in the core. The publish later
            // replays it without another prompt.
            let challenge = try await registry.memberChallenge(
                groupPublicKey: operation["group_public_key_hex"] as? String ?? "",
                publicKey: operation["public_key_hex"] as? String ?? "",
                attestation: operation["attestation_hex"] as? String ?? "",
                rpId: passkey.relyingPartyId
            )
            // Spec 075: the page fetches its own challenge for the same
            // inputs and the core demands the two be EQUAL — which is why
            // the bytes the wallet fetched ride along.
            if let answer = try await onTheClearSigner(
                operation, memberChallenge: try fromHex(s: Self.stripHex(challenge))
            ) {
                return ["type": "member_proof_signed", "proof": try Self.memberProof(answer)]
            }
            let assertion = try await passkey.assert(
                challenge: try fromHex(s: Self.stripHex(challenge)),
                credentialIdHex: operation["credential_id"] as? String,
                transports: operation["transports"] as? String ?? "",
                // The route that minted this key confirms it — a key created on
                // a phone over caBLE is confirmed on that phone.
                method: KeyMethod(rawValue: operation["method"] as? String ?? "") ?? .platform
            )
            return ["type": "member_proof_signed", "proof": try Self.memberProof(assertion)]

        case "lookup_legacy_name":
            let name = await registry.legacyName(
                credentialIdHex: operation["credential_id"] as? String ?? ""
            )
            return ["type": "legacy_name", "name": name ?? NSNull()]

        case "authenticate_passkey":
            if let answer = try await onTheClearSigner(operation) {
                return ["type": "passkey_authenticated", "assertion": answer, "now_iso": Self.nowISO()]
            }
            let assertion = try await passkey.assert(
                challenge: PasskeyExecutor.random(Self.challengeBytes),
                credentialIdHex: nil,
                method: KeyMethod(rawValue: operation["method"] as? String ?? "") ?? .platform
            )
            return [
                "type": "passkey_authenticated",
                "assertion": assertion.wire,
                "now_iso": Self.nowISO(),
            ]

        case "load_accounts":
            return ["type": "accounts_loaded", "accounts": await store.loadAccounts()]

        case "save_account":
            await store.saveAccount(operation["account"] as? [String: Any] ?? [:])
            return ["type": "account_saved"]

        case "save_pending_upload":
            await store.savePendingUpload(operation["record"] as? [String: Any] ?? [:])
            return ["type": "pending_upload_saved"]

        case "remove_pending_upload":
            await store.removePendingUpload(
                credentialIdHex: operation["credential_id"] as? String ?? ""
            )
            return ["type": "pending_upload_removed"]

        case "registry_publish":
            try await publish(operation)
            return ["type": "registry_published"]

        case "registry_query_by_public_key":
            let status = try await registry.queryByPublicKey(
                operation["public_key_hex"] as? String ?? ""
            )
            return [
                "type": "registry_key_status",
                "registered": status.registered,
                "unit_ids": status.unitIds.map { NSNumber(value: $0) },
            ]

        case "registry_query_unit":
            let unitId = (operation["unit_id"] as? NSNumber)?.uint32Value ?? 0
            let unit = try await registry.queryUnit(unitId)
            return [
                "type": "registry_unit",
                "metadata_hex": unit.metadataHex,
                "members": unit.members.map { member in
                    [
                        "credential_id": member.credentialIdHex,
                        "public_key_hex": member.publicKeyHex,
                        "authenticator_attachment": member.authenticatorAttachment,
                        "transports": member.transports,
                    ]
                },
            ]

        case "probe_index_health":
            return ["type": "index_health", "ok": await registry.probeHealth()]

        case "wait":
            // `wait` is the core's only clock. `Task.sleep` is cancellable, so
            // an abandoned timer stops rather than firing into a machine that
            // moved on — and the driver drops its answer either way.
            let ms = (operation["ms"] as? NSNumber)?.uint64Value ?? 0
            try? await Task.sleep(nanoseconds: ms * 1_000_000)
            return ["type": "waited"]

        case "prompt":
            guard let deps else { return ["type": "prompt_answered", "accepted": false] }
            let accepted = await deps.prompt(
                kind: PromptKind(json: operation["kind"] as? [String: Any] ?? [:]),
                confirmable: operation["confirmable"] as? Bool ?? false
            )
            return ["type": "prompt_answered", "accepted": accepted]

        case "complete_onboarding":
            await deps?.complete(mode: operation["mode"] as? [String: Any] ?? [:])
            return ["type": "onboarding_completed"]

        default:
            throw UnhandledOperation(type: type)
        }
    }

    /// Possession-proven publish of the founding key set as one registry group.
    ///
    /// With a group seed in hand the members already carry creation-time proofs
    /// and **no prompt is raised**; that is the whole point of the interleaved
    /// create-then-confirm flow. The empty-seed path is the login re-publish: a
    /// fresh group key, and one live assertion per member that has no proof.
    private func publish(_ operation: [String: Any]) async throws {
        let members = (operation["members"] as? [[String: Any]] ?? []).map(PublishMember.init(json:))
        guard !members.isEmpty else {
            throw RegistryFailure(message: "registry publish needs at least one member", network: false)
        }
        // The route a member with no replayable proof signs its live possession
        // proof over — recovery's third signature. A caBLE-recovered wallet
        // signs it on the same phone; a create replays and never reaches here.
        let method = KeyMethod(rawValue: operation["method"] as? String ?? "") ?? .platform

        var seedHex = operation["group_seed_hex"] as? String ?? ""
        var groupPublicKey = operation["group_public_key_hex"] as? String ?? ""
        if seedHex.isEmpty || groupPublicKey.isEmpty {
            seedHex = toHex(data: PasskeyExecutor.random(Self.groupSeedBytes), prefixed: false)
            groupPublicKey = try registryGroupPublicKeyFromSeed(seedHex: seedHex)
        }

        let metadataHex = operation["metadata_hex"] as? String ?? ""
        let challenge = try await registry.groupChallenge(
            metadataHex: metadataHex,
            groupPublicKey: groupPublicKey,
            members: members,
            rpId: passkey.relyingPartyId
        )

        var proven: [ProvenMember] = []
        for member in members {
            if let existing = member.proof {
                proven.append(ProvenMember(member: member, proof: existing))
                continue
            }
            guard let memberChallenge = challenge.memberChallenges[member.publicKeyHex.lowercased()]
            else {
                throw RegistryFailure(
                    message: "registry challenge is missing member \(member.publicKeyHex)",
                    network: false
                )
            }
            let assertion = try await passkey.assert(
                challenge: try fromHex(s: Self.stripHex(memberChallenge)),
                credentialIdHex: member.credentialIdHex,
                method: method
            )
            proven.append(ProvenMember(member: member, proof: try Self.memberProof(assertion)))
        }

        // The group key silently closes over the content hash.
        let groupJSON = try registryBuildGroupProof(
            seedHex: seedHex,
            rpId: passkey.relyingPartyId,
            challengeHex: challenge.groupChallenge
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
            rpId: passkey.relyingPartyId
        )

        // `done` up front means the identical group was already on-chain —
        // idempotent by content hash, and just as landed as a fresh one.
        if ack.status == "done" { return }
        guard let id = ack.id else {
            throw RegistryFailure(message: "register was accepted without a task id", network: false)
        }
        try await registry.awaitTask(id: id)
    }

    // MARK: - Spec 075: the Clear Signer as a passkey route

    /// Runs `operation` on the Clear Signer when that is the route the person
    /// chose, and answers the core's own `Registration` / `Assertion` as the
    /// object the shell result carries. `nil` means "not this route" and the
    /// caller goes on to the OS sheet.
    ///
    /// The whole operation is handed over verbatim: `clearSignerCeremonyRequest`
    /// reads the fields it needs out of the same wire JSON the machine sent,
    /// so no field is copied here and none can be copied wrongly.
    private func onTheClearSigner(
        _ operation: [String: Any], memberChallenge: Data? = nil
    ) async throws -> [String: Any]? {
        guard KeyMethod(rawValue: operation["method"] as? String ?? "") == .clearSigner else {
            return nil
        }
        guard let clearSigner else {
            // Chosen where no page can be opened: fail closed rather than
            // silently signing with something else.
            throw PasskeyFailure(kind: .notSupported, message: "The Clear Signer cannot be opened here.")
        }
        let step = await clearSigner.ceremony(
            operationJson: CoreJSON.string(operation),
            walletName: walletName(),
            registry: await registry.base(),
            expectedMemberChallenge: memberChallenge,
            // A key that already exists names the page it lives behind; a
            // key being created has none yet and opens the one from Settings.
            page: operation["signer_origin"] as? String
        )
        switch step {
        case .registered(let json), .asserted(let json):
            return try CoreJSON.object(json)
        case .failed(let kind, let message):
            throw PasskeyFailure(kind: kind, message: message ?? "")
        }
    }

    /// The core's `Assertion` JSON, as the registry proof the machine wants.
    private static func memberProof(_ assertion: [String: Any]) throws -> [String: Any] {
        try CoreJSON.object(
            try registryBuildMemberProof(
                authenticatorDataHex: assertion["authenticator_data_hex"] as? String ?? "",
                clientDataJsonHex: assertion["client_data_json_hex"] as? String ?? "",
                signatureDerHex: assertion["signature_der_hex"] as? String ?? ""
            )
        )
    }

    private static func memberProof(_ assertion: Assertion) throws -> [String: Any] {
        try CoreJSON.object(
            try registryBuildMemberProof(
                authenticatorDataHex: assertion.authenticatorDataHex,
                clientDataJsonHex: assertion.clientDataJsonHex,
                signatureDerHex: assertion.signatureDerHex
            )
        )
    }

    /// The challenge a proof purpose signs over.
    ///
    /// The label strings are preserved verbatim from the other clients — they
    /// are part of the wire, not decoration. The two recovery purposes share a
    /// label on purpose: what must differ between the two signatures is the
    /// challenge BYTES, and the millisecond tail supplies that. The invariant is
    /// not trusted to the shell either way — `recover_public_key_from_assertions`
    /// returns nothing unless the two assertions pin down exactly one key, so a
    /// repeated challenge fails closed in the core.
    static func challenge(for purpose: String) -> Data {
        let label = purpose == "verify" ? "vela-verify-" : "vela-recover-"
        let millis = Int64(Date().timeIntervalSince1970 * 1000)
        return Data("\(label)\(millis)".utf8)
    }

    /// UTC, always: a stored `created_at_iso` carrying a local offset means
    /// something different the moment the phone changes time zone.
    static func nowISO() -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(identifier: "UTC")
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'"
        return formatter.string(from: Date())
    }

    private static func stripHex(_ value: String) -> String {
        value.hasPrefix("0x") ? String(value.dropFirst(2)) : value
    }

    // MARK: - The failure contract

    struct UnhandledOperation: Error { let type: String }

    /// The result variant an operation owes when its execution threw.
    ///
    /// This is the whole failure contract: every rejection lands here, and the
    /// core sees a described outcome rather than an error. An operation missing
    /// from this map would leave the core waiting forever, so the fallthrough is
    /// deliberate and loud.
    static func failure(for operation: [String: Any], error: Error) -> [String: Any] {
        switch operation["type"] as? String ?? "" {
        case "check_passkey_support":
            return ["type": "passkey_support", "supported": false]

        case "register_passkey", "sign_proof", "authenticate_passkey":
            return passkeyFailure(error)

        // Mixed: the ceremony and the challenge fetch can each fail, and the
        // core branches differently on the two. Classify by what actually threw
        // rather than by which operation it was.
        case "sign_member_proof":
            return error is RegistryFailure ? indexFailure(error) : passkeyFailure(error)

        case "generate_group_key", "load_accounts", "save_account",
             "save_pending_upload", "remove_pending_upload":
            return ["type": "storage_failed", "message": describe(error)]

        case "registry_publish", "registry_query_by_public_key", "registry_query_unit":
            return indexFailure(error)

        // Best-effort and read-only: a lost name degrades the label, never the flow.
        case "lookup_legacy_name":
            return ["type": "legacy_name", "name": NSNull()]

        case "probe_index_health":
            return ["type": "index_health", "ok": false]

        case "wait":
            return ["type": "waited"]

        // A dismissed dialog is a refusal, not an error.
        case "prompt":
            return ["type": "prompt_answered", "accepted": false]

        // The hand-over already happened as far as the core is concerned; a
        // failure here is the app's to survive, not the machine's.
        case "complete_onboarding":
            return ["type": "onboarding_completed"]

        default:
            // An operation nobody handled. The core must still be unblocked, or
            // the flow stops with a spinner that never resolves — but it says
            // loudly what happened rather than pretending to succeed.
            return [
                "type": "storage_failed",
                "message": "unhandled shell operation: \(operation["type"] as? String ?? "?")",
            ]
        }
    }

    private static func passkeyFailure(_ error: Error) -> [String: Any] {
        let failure = (error as? PasskeyFailure) ?? PasskeyExecutor.classify(error)
        return [
            "type": "passkey_failed",
            "kind": failure.kind.rawValue,
            // A classified failure's copy comes from the classification; only
            // `other` and `not_supported` carry the platform's own words,
            // because those go into the bug report and must not be prettified.
            "message": failure.kind == .cancelled ? NSNull() : failure.message,
        ]
    }

    private static func indexFailure(_ error: Error) -> [String: Any] {
        [
            "type": "index_failed",
            "message": describe(error),
            // The one bit of classification only a shell can supply: a request
            // that never arrived is not the same as one the server refused.
            "network": (error as? RegistryFailure)?.network ?? true,
        ]
    }

    private static func describe(_ error: Error) -> String {
        if let failure = error as? RegistryFailure { return failure.message }
        if let failure = error as? PasskeyFailure { return failure.message }
        if let unhandled = error as? UnhandledOperation {
            return "unhandled shell operation: \(unhandled.type)"
        }
        return error.localizedDescription
    }
}

private extension Assertion {
    var wire: [String: Any] {
        [
            "credential_id": credentialIdHex,
            "signature_der_hex": signatureDerHex,
            "authenticator_data_hex": authenticatorDataHex,
            "client_data_json_hex": clientDataJsonHex,
            "user_id_hex": userIdHex ?? NSNull(),
            "authenticator_attachment": authenticatorAttachment,
        ]
    }
}
