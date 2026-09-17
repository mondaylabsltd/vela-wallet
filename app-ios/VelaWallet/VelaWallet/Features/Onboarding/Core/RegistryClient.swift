//
//  RegistryClient.swift
//  VelaWallet
//
//  The public-key registry, over HTTP.
//
//  Nothing here decides anything. The one judgement it makes is the `network`
//  bit on a failure — whether the request reached the server at all — because
//  that is the single fact only a shell can know, and the core needs it to tell
//  "the service said no" from "the service was not there".
//

import Foundation

/// A registry call that did not produce an answer.
///
/// `network` means the request never reached the server — a transport failure
/// or a timeout, as distinct from a refusal. Only a shell can tell those apart,
/// which is why this single bit of classification is delegated to it.
struct RegistryFailure: Error {
    let message: String
    let network: Bool
}

struct GroupChallenge {
    let groupChallenge: String
    /// Keyed by lowercase public key — the server echoes the case it was sent.
    let memberChallenges: [String: String]
}

struct RegisterAck {
    let id: String?
    let status: String
}

struct KeyStatus {
    let registered: Bool
    let unitIds: [UInt32]
}

struct UnitMember {
    let credentialIdHex: String
    let publicKeyHex: String
    let authenticatorAttachment: String
    let transports: String
}

struct UnitDetail {
    let metadataHex: String
    let members: [UnitMember]
}

/// One member of a publish, as the core hands it over.
struct PublishMember {
    let credentialIdHex: String
    let publicKeyHex: String
    let attestationHex: String
    let authenticatorAttachment: String
    let transports: String
    /// The proof collected AT CREATION. Absent on the login re-publish, whose
    /// executor signs the member live.
    let proof: [String: Any]?

    init(json: [String: Any]) {
        credentialIdHex = json["credential_id"] as? String ?? ""
        publicKeyHex = json["public_key_hex"] as? String ?? ""
        attestationHex = json["attestation_hex"] as? String ?? ""
        authenticatorAttachment = json["authenticator_attachment"] as? String ?? ""
        transports = json["transports"] as? String ?? ""
        proof = json["proof"] as? [String: Any]
    }
}

/// A member whose possession proof is in hand.
struct ProvenMember {
    let member: PublishMember
    let proof: [String: Any]
}

actor RegistryClient {

    /// The v2 registry. Overridable so a self-hosted stack is a setting, not a fork.
    static let defaultURL = "https://p256-index-v2.getvela.app"

    /// The health identities this endpoint accepts — the legacy index and the
    /// v2 registry, so a wallet can point at either during the migration.
    private static let serviceIdentities: Set<String> = [
        "webauthn-p256-publickey-registry",
        "webauthn-p256-publickey-index",
    ]

    /// A vela wallet's founding set is capped at 7 keys; a larger group is not
    /// ours and must never be reconstructed into an account.
    private static let maxUnitMembers = 7

    private static let readTimeout: TimeInterval = 15
    private static let writeTimeout: TimeInterval = 30
    private static let pollTimeout: TimeInterval = 120
    private static let pollInterval: UInt64 = 2_000_000_000

    private var baseURL: String

    init(baseURL: String = RegistryClient.defaultURL) {
        self.baseURL = Self.normalize(baseURL)
    }

    func setBaseURL(_ url: String) {
        baseURL = Self.normalize(url)
    }

    static func normalize(_ url: String) -> String {
        let trimmed = url
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "\r", with: "")
            .replacingOccurrences(of: "\n", with: "")
        let stripped = trimmed.hasSuffix("/") ? String(trimmed.dropLast()) : trimmed
        return stripped.isEmpty ? defaultURL : stripped
    }

    /// MEMBER-mode challenge: one founding passkey confirming AT CREATION.
    /// Binds only (groupPublicKey, own attestation), so it exists before the
    /// rest of the set does — which is what makes the interleaved
    /// create-then-confirm flow work.
    func memberChallenge(
        groupPublicKey: String,
        publicKey: String,
        attestation: String,
        rpId: String
    ) async throws -> String {
        var body: [String: Any] = [
            "rpId": rpId,
            "groupPublicKey": groupPublicKey,
            "publicKey": publicKey,
        ]
        if !attestation.isEmpty { body["attestation"] = attestation }
        let answer = try await request("/api/challenge", body: body, timeout: Self.readTimeout, label: "Challenge")
        guard let challenge = answer["challenge"] as? String else {
            throw RegistryFailure(message: "Challenge failed: no challenge in the response", network: false)
        }
        return challenge
    }

    /// GROUP-mode challenge: closing the group at publish.
    func groupChallenge(
        metadataHex: String,
        groupPublicKey: String,
        members: [PublishMember],
        rpId: String
    ) async throws -> GroupChallenge {
        var body: [String: Any] = [
            "rpId": rpId,
            "groupPublicKey": groupPublicKey,
            "members": members.map { member -> [String: Any] in
                var entry: [String: Any] = ["publicKey": member.publicKeyHex]
                if !member.attestationHex.isEmpty { entry["attestation"] = member.attestationHex }
                return entry
            },
        ]
        if !metadataHex.isEmpty { body["metadata"] = metadataHex }

        let answer = try await request("/api/challenge", body: body, timeout: Self.readTimeout, label: "Challenge")
        guard let group = answer["groupChallenge"] as? [String: Any],
              let groupChallenge = group["challenge"] as? String
        else {
            throw RegistryFailure(message: "Challenge failed: no group challenge", network: false)
        }
        var perMember: [String: String] = [:]
        for entry in answer["members"] as? [[String: Any]] ?? [] {
            if let key = entry["publicKey"] as? String, let challenge = entry["challenge"] as? String {
                perMember[key.lowercased()] = challenge
            }
        }
        return GroupChallenge(groupChallenge: groupChallenge, memberChallenges: perMember)
    }

    /// Register the closed group.
    ///
    /// `members` is built here into the REGISTRY's camelCase shape. The core's
    /// wire type is snake_case because it is generated from Rust; sending
    /// `public_key_hex` where the server reads `publicKey` earns a
    /// `members[0]: publicKey is required` — after the person has already minted
    /// and confirmed every key. The two vocabularies meet in this one function.
    func registerGroup(
        metadataHex: String,
        groupPublicKey: String,
        groupProof: [String: Any],
        members: [ProvenMember],
        rpId: String
    ) async throws -> RegisterAck {
        var body: [String: Any] = [
            "rpId": rpId,
            "groupPublicKey": groupPublicKey,
            "groupProof": groupProof,
            "members": members.map { proven -> [String: Any] in
                var entry: [String: Any] = [
                    "publicKey": proven.member.publicKeyHex,
                    "credentialId": proven.member.credentialIdHex,
                    "proof": proven.proof,
                ]
                if !proven.member.attestationHex.isEmpty {
                    entry["attestation"] = proven.member.attestationHex
                }
                if !proven.member.authenticatorAttachment.isEmpty {
                    entry["authenticatorAttachment"] = proven.member.authenticatorAttachment
                }
                if !proven.member.transports.isEmpty {
                    entry["transports"] = proven.member.transports
                }
                return entry
            },
        ]
        if !metadataHex.isEmpty { body["metadata"] = metadataHex }

        let answer = try await request("/api/register", body: body, timeout: Self.writeTimeout, label: "Register")
        return RegisterAck(id: answer["id"] as? String, status: answer["status"] as? String ?? "")
    }

    /// Poll until terminal.
    ///
    /// A transient read failure is retried until the budget runs out: the task
    /// is already accepted, so giving up on one bad read would report a failure
    /// that did not happen — and the group may well be landing meanwhile.
    func awaitTask(id: String) async throws {
        let deadline = Date().addingTimeInterval(Self.pollTimeout)
        var lastError: String?
        while Date() < deadline {
            do {
                let task = try await request(
                    "/api/task/\(Self.escape(id))",
                    body: nil,
                    timeout: Self.readTimeout,
                    label: "Task status"
                )
                switch task["status"] as? String {
                case "done": return
                case "failed":
                    throw RegistryFailure(
                        message: "Register failed: \(task["error"] as? String ?? "unknown")",
                        network: false
                    )
                default: break
                }
            } catch let failure as RegistryFailure {
                if !failure.network { throw failure }
                lastError = failure.message
            }
            try? await Task.sleep(nanoseconds: Self.pollInterval)
        }
        throw RegistryFailure(
            message: "Register timed out after \(Int(Self.pollTimeout))s"
                + (lastError.map { ": \($0)" } ?? ""),
            network: true
        )
    }

    /// `/api/query?publicKey=` — is this key registered, and which groups does
    /// it found?
    func queryByPublicKey(_ publicKeyHex: String) async throws -> KeyStatus {
        let profile = try await request(
            "/api/query?publicKey=\(Self.escape(publicKeyHex))",
            body: nil,
            timeout: Self.readTimeout,
            label: "Query"
        )
        let raw = (profile["groups"] as? [String: Any])?["unitIds"] as? [Any] ?? []
        var unitIds: [UInt32] = []
        for value in raw {
            // The core speaks u32 unit ids because the wire is JSON. An id past
            // 2^32 would truncate into a DIFFERENT group, so this fails the
            // query instead of quietly fetching the wrong founding set.
            guard let number = value as? NSNumber,
                  number.int64Value >= 0,
                  number.int64Value < Int64(UInt32.max) + 1
            else {
                throw RegistryFailure(message: "Query failed: unit id out of u32 range", network: false)
            }
            unitIds.append(number.uint32Value)
        }
        let registered = !(profile["entry"] is NSNull) && profile["entry"] != nil
        return KeyStatus(registered: registered, unitIds: unitIds)
    }

    /// `/api/query?unitId=` — the group's frozen metadata and ALL its founding
    /// members in ascending order, which IS the canonical founding order the
    /// Safe address derivation pins.
    ///
    /// Both guards refuse rather than degrade: a group larger than a wallet's
    /// cap is not ours, and a partial page would rebuild the address from a
    /// SUBSET of the founding set — a different, wrong, fundable address.
    func queryUnit(_ unitId: UInt32) async throws -> UnitDetail {
        let detail = try await request(
            "/api/query?unitId=\(unitId)&pageSize=\(Self.maxUnitMembers)&order=asc",
            body: nil,
            timeout: Self.readTimeout,
            label: "Query"
        )
        let membersBox = detail["members"] as? [String: Any]
        let total = (membersBox?["total"] as? NSNumber)?.intValue ?? 0
        let items = membersBox?["items"] as? [[String: Any]] ?? []
        if total > Self.maxUnitMembers {
            throw RegistryFailure(
                message: "Query failed: unit \(unitId) has \(total) members (cap \(Self.maxUnitMembers))",
                network: false
            )
        }
        if items.count != total {
            throw RegistryFailure(
                message: "Query failed: unit \(unitId) page holds \(items.count) of \(total) members",
                network: false
            )
        }
        guard let unit = detail["unit"] as? [String: Any],
              let metadata = unit["metadata"] as? String
        else {
            throw RegistryFailure(message: "Query failed: unit \(unitId) has no metadata", network: false)
        }
        return UnitDetail(
            metadataHex: metadata,
            members: items.map { item in
                UnitMember(
                    credentialIdHex: item["credentialId"] as? String ?? "",
                    publicKeyHex: item["publicKey"] as? String ?? "",
                    authenticatorAttachment: item["authenticatorAttachment"] as? String ?? "",
                    transports: item["transports"] as? String ?? ""
                )
            }
        )
    }

    /// One health probe. Never throws: the core asked a yes/no question.
    func probeHealth() async -> Bool {
        do {
            let health = try await request(
                "/api/health?_t=\(Int(Date().timeIntervalSince1970 * 1000))",
                body: nil,
                timeout: Self.readTimeout,
                label: "Health"
            )
            return Self.serviceIdentities.contains(health["service"] as? String ?? "")
                && (health["status"] as? String) == "ok"
        } catch {
            return false
        }
    }

    /// One raw GET, for a caller that reads the body itself: the status and the
    /// text, or `nil` when the request never arrived.
    ///
    /// This is the transport half of "the name behind a wallet ADDRESS". The v2
    /// index cannot be asked that directly — `?walletRef=` answers 400, which
    /// is what `queryByWalletRef` here asked for two specs — so the name is
    /// reached through the chain, and the walk is the core's
    /// (`Core/RegistryNameLookup`, issue 191). Best-effort: nothing here throws.
    func rawGet(_ path: String) async -> (status: Int, body: String)? {
        guard let url = URL(string: baseURL + path) else { return nil }
        let request = URLRequest(url: url, timeoutInterval: Self.readTimeout)
        guard let (data, response) = try? await URLSession.shared.data(for: request),
              let http = response as? HTTPURLResponse
        else { return nil }
        return (http.statusCode, String(decoding: data, as: UTF8.self))
    }

    /// The v1 index's display name for a credential — the only place a v1-era
    /// wallet's name survives. Best-effort and read-only: a lost name degrades
    /// the label, never the flow.
    func legacyName(credentialIdHex: String) async -> String? {
        let record = try? await request(
            "/api/query?credentialId=\(Self.escape(credentialIdHex))",
            body: nil,
            timeout: Self.readTimeout,
            label: "Legacy name"
        )
        let name = (record?["name"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines)
        return (name?.isEmpty ?? true) ? nil : name
    }

    // MARK: - Transport

    private func request(
        _ path: String,
        body: [String: Any]?,
        timeout: TimeInterval,
        label: String
    ) async throws -> [String: Any] {
        guard let url = URL(string: baseURL + path) else {
            throw RegistryFailure(message: "\(label) failed: \(baseURL)\(path) is not a URL", network: false)
        }
        var urlRequest = URLRequest(url: url, timeoutInterval: timeout)
        if let body {
            urlRequest.httpMethod = "POST"
            urlRequest.setValue("application/json", forHTTPHeaderField: "content-type")
            urlRequest.httpBody = try? JSONSerialization.data(withJSONObject: body)
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await URLSession.shared.data(for: urlRequest)
        } catch {
            // The request never arrived. This is the ONE line that separates the
            // two failure worlds, and it is why the classification is delegated
            // to the shell at all.
            throw RegistryFailure(message: "\(label) failed: \(error.localizedDescription)", network: true)
        }

        guard let http = response as? HTTPURLResponse else {
            throw RegistryFailure(message: "\(label) failed: not an HTTP response", network: false)
        }
        guard (200..<300).contains(http.statusCode) else {
            // The server answered — a refusal is an answer.
            throw RegistryFailure(message: "\(label) failed: \(http.statusCode)", network: false)
        }
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            // Something that is not our protocol. That is an answer, not an
            // outage: `network = false` keeps the core from offering "check your
            // connection" for a broken proxy.
            throw RegistryFailure(message: "\(label) failed: malformed response", network: false)
        }
        return object
    }

    private static func escape(_ value: String) -> String {
        value.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? value
    }
}
