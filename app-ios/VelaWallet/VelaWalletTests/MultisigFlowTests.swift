//
//  MultisigFlowTests.swift
//  VelaWalletTests
//
//  The multisig scenarios as FLOWS, through the real core machines over
//  uniffi (`MultisigAccountsCore`, `MultisigProposalsCore`,
//  `MultisigMembersCore`, each on `MultisigMachine` and this app's
//  `MultisigExecutor`) — answered by a simulated world, not by stubs that
//  say what a test wants to hear:
//
//    - the genesis service: `/api/health`, `/api/import` (recording on the
//      index chain), `/api/genesis/…`, and the proposals mailbox
//      (`POST /api/proposals` merging approvals by safeTxHash — the hash the
//      core computes — and the per-chain and cross-chain reads);
//    - chains holding Safe's own v1.4.1 code at its own addresses (the core's
//      vectors, `rust/crates/vela-core/tests/vectors/safe-code`), deployed
//      Safes answering `getOwners`/`getThreshold`/`nonce`/
//      `getModulesPaginated`/`approvedHashes` and their guard, handler and
//      singleton slots;
//    - passkeys signed by the parallel space's fixture keys
//      (`fixtureAssert`): a genuine P-256 WebAuthn assertion the core
//      verifies.
//
//  A subset of the core's own world (`rust/crates/vela-core/tests/
//  multisig_world/mod.rs`), and the flows are the core's
//  (`app_multisig_accounts.rs` created_and_recorded_on_the_index_chain,
//  `app_multisig_outside.rs` rejecting_proposes_the_same_number_cancellation…,
//  `app_multisig_members.rs` removes_a_member_with_one_proposal_per_deployed…,
//  the_approvals_stepper_changes_the_rule…) — driven here through the iOS
//  shell, with the verbs the iOS screens send (`MultisigHub.propose`,
//  `MultisigProposalSheet.reject`, `MultisigSettingsSheet.propose`).
//
//  Waits are on state, ended early by the machine's idleness (`Waits.swift`).
//

#if DEBUG
import CoreGraphics
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - The simulated world

/// One deployed Safe on one chain.
struct SimSafe {
    var owners: [String]
    var threshold: Int
    var nonce = 0
    var modules: [String] = []
    var guardAddress: String?
    var handler = MsSimWorld.handler141
    var singleton = MsSimWorld.safeL2141
    /// `approvedHashes[owner][hash]` set on chain: "owner:hash", lowercase.
    var approved: Set<String> = []
}

/// The multisig machines' world: service, mailbox, chains, store, clock,
/// passkeys. Every port answers from here.
@MainActor
final class MsSimWorld: MultisigPorts {
    static let service = "http://service.test"
    static let t0: Double = 1_793_000_000_000

    static let factory = "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67"
    static let safe141 = "0x41675C099F32341bf84BFc5382aF534df5C7461a"
    static let safeL2141 = "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762"
    static let handler141 = "0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99"
    static let toL2Setup141 = "0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54"
    static let genesisRegistry = "0xF412388bF908E510066d3Be5c704bd9A327f945d"
    static let webauthnSigner = "0x94a4F6affBd8975951142c3999aEAB7ecee555c2"
    static let guardSlot = "0x4a204f620c8c5ccdca3fd54d003badd85ba500436a431f0cbda4f558c93c34c8"
    static let handlerSlot = "0x6c9a6c4a39284e37ed1cf53d337577d14212a4870fb976a4366c693b939918d5"
    static let singletonSlot = "0x0"

    /// Safe's own v1.4.1 code at its own addresses, from the core's vectors.
    static func canonicalCodes() throws -> [String: String] {
        let dir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("rust/crates/vela-core/tests/vectors/safe-code")
        let named = [
            (factory, "safe-proxy-factory-1.4.1"), (safe141, "safe-1.4.1"), (safeL2141, "safe-l2-1.4.1"),
            (handler141, "compatibility-fallback-handler-1.4.1"), (toL2Setup141, "safe-to-l2-setup-1.4.1"),
        ]
        var out: [String: String] = [:]
        for (address, name) in named {
            let text = try String(contentsOf: dir.appendingPathComponent("\(name).hex"), encoding: .utf8)
            out[address.lowercased()] = text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return out
    }

    let codes: [String: String]
    let started = Date()

    // The person.
    var locals: [FixtureAccountRecord] = []
    var passkeys: [(owner: String, challenge: String)] = []
    /// The person's next passkeys answered otherwise (closed, timed out),
    /// in order; none left: the fixture key signs.
    var passkeyAnswers: [[String: Any]] = []
    var notices: [[String: Any]] = []
    var store: [String: String] = [:]
    private var randomByte: UInt8 = 7

    // The genesis service.
    var served = [100]
    var records: [String: [String: Any]] = [:]
    var imports: [[String: Any]] = []
    var mailbox: [[String: Any]] = []
    var submits: [[String: Any]] = []
    var httpLog: [String] = []
    /// R15: the multisigs (lower-cased) whose record lookup is held — a slow
    /// service for one link while another is opened.
    var holdGenesis: Set<String> = []

    // The chains.
    var safes: [String: SimSafe] = [:]
    var rpcLog: [String] = []

    let peer: MultisigPeerTransport = FakePeer()

    init() throws {
        codes = try Self.canonicalCodes()
    }

    // MARK: Setup

    /// The parallel space's fixture accounts `indices`, on this device.
    static func fixtures(_ indices: [Int]) throws -> [FixtureAccountRecord] {
        let all = try fixtureAccounts()
        return try indices.map { index in try #require(all.first { $0.index == UInt32(index) }) }
    }

    /// `locals` as the machines read them (`MultisigLocalAccount`).
    var localsJson: [[String: Any]] {
        locals.map { account in
            [
                "address": account.address, "name": account.name,
                "keys": [["credential_id": account.credentialIdHex, "public_key_hex": account.publicKeyHex]],
            ]
        }
    }

    func deploy(_ chain: Int, _ address: String, _ safe: SimSafe) {
        safes["\(chain):\(address.lowercased())"] = safe
    }

    func storedAccounts() -> [[String: Any]] {
        guard let text = store["vela.multisigAccounts"],
              let rows = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [[String: Any]]
        else { return [] }
        return rows
    }

    // MARK: Ports

    func http(method: String, url: String, body: String?, timeoutMs: Int) async throws
        -> (status: Int, retryAfter: String?, body: String) {
        httpLog.append("\(method) \(url)")
        if url.hasPrefix("https://api.safe.global/tx-service/") { return (404, nil, "{}") }
        guard url.hasPrefix(Self.service) else { throw URLError(.cannotFindHost) }
        let path = String(url.dropFirst(Self.service.count))
        func answer(_ status: Int, _ object: Any) -> (Int, String?, String) { (status, nil, Self.text(object)) }
        if path == "/api/health" {
            return answer(200, [
                "service": "safe-genesis-registry", "ok": true, "homeChainId": 100,
                "chains": served.map { ["chainId": $0, "registry": $0 == 100] },
            ])
        }
        if path.hasPrefix("/api/genesis/") {
            let safe = String(path.dropFirst("/api/genesis/".count)).lowercased()
            // R15: a slow service for this one multisig, until let go.
            while holdGenesis.contains(safe), !Task.isCancelled { try? await Task.sleep(nanoseconds: 5_000_000) }
            if let record = records[safe] { return answer(200, record) }
            return answer(404, ["found": false])
        }
        if path == "/api/import", method == "POST" {
            let request = Self.object(body)
            imports.append(request)
            if let safe = request["safe"] as? String, let genesis = request["genesis"] {
                records[safe.lowercased()] = [
                    "found": true, "pending": false, "genesis": genesis, "recordedOn": [100], "readFrom": 100,
                ]
            }
            return answer(200, ["status": "recorded", "recordedOn": [100]])
        }
        if path == "/api/proposals", method == "POST" {
            let request = Self.object(body)
            submits.append(request)
            return answer(201, take(request))
        }
        if path.hasPrefix("/api/proposals/") {
            let rest = String(path.dropFirst("/api/proposals/".count))
            let parts = rest.split(separator: "?", maxSplits: 1).map(String.init)
            let safe = parts[0]
            let chain = parts.count > 1
                ? parts[1].split(separator: "&").compactMap { $0.hasPrefix("chainId=") ? Int($0.dropFirst(8)) : nil }.first
                : nil
            let mine = mailbox.filter {
                ($0["safe"] as? String)?.caseInsensitiveCompare(safe) == .orderedSame
                    && (chain == nil || Self.int($0["chainId"]) == chain)
            }
            if chain != nil {
                return answer(200, [
                    "members": ["deployed": false, "owners": [], "threshold": "1", "nonce": "0"],
                    "proposals": mine,
                ])
            }
            var counts: [Int: Int] = [:]
            for p in mine { counts[Self.int(p["chainId"]) ?? 0, default: 0] += 1 }
            return answer(200, [
                "total": mine.count,
                "chains": counts.keys.sorted().map { ["chainId": $0, "count": counts[$0]!] },
                "proposals": mine,
            ])
        }
        if path.hasPrefix("/api/task/") { return answer(200, ["status": "done"]) }
        return answer(404, ["error": "no route"])
    }

    /// The mailbox merges an approval into its proposal (by the safeTxHash
    /// the core computes), or opens one.
    private func take(_ request: [String: Any]) -> [String: Any] {
        let chain = Self.int(request["chainId"]) ?? 0
        let safe = request["safe"] as? String ?? ""
        let tx = request["safeTx"] ?? [:]
        let hash = Self.safeTxHash(chain: chain, safe: safe, tx: tx) ?? ""
        var approval = request["approval"] as? [String: Any] ?? [:]
        approval["createdMs"] = Int(nowMs())
        let owner = (approval["owner"] as? String ?? "").lowercased()
        let same: ([String: Any]) -> Bool = {
            Self.int($0["chainId"]) == chain && ($0["safeTxHash"] as? String)?.lowercased() == hash.lowercased()
        }
        let status: String
        if let i = mailbox.firstIndex(where: same) {
            var proposal = mailbox[i]
            var approvals = proposal["approvals"] as? [[String: Any]] ?? []
            if approvals.contains(where: { ($0["owner"] as? String)?.lowercased() == owner }) {
                status = "unchanged"
            } else {
                approvals.append(approval)
                status = "approved"
            }
            proposal["approvals"] = approvals
            proposal["updatedMs"] = Int(nowMs())
            mailbox[i] = proposal
        } else {
            mailbox.append([
                "chainId": chain, "safe": safe, "safeTxHash": hash, "safeTx": tx,
                "proposer": approval["owner"] ?? "", "approvals": [approval],
                "createdMs": Int(nowMs()), "updatedMs": Int(nowMs()),
            ])
            status = "created"
        }
        return ["status": status, "proposal": mailbox.first(where: same) ?? [:]]
    }

    func rpc(chainId: Int, method: String, params: Any) async -> MultisigRpcReply {
        rpcLog.append("\(chainId) \(method)")
        let list = params as? [Any] ?? []
        switch method {
        case "eth_getCode":
            let address = (list.first as? String ?? "").lowercased()
            if safes["\(chainId):\(address)"] != nil { return .result("0x6080") }
            return .result(codes[address] ?? "0x")
        case "eth_getTransactionCount":
            return .result("0x0")
        case "eth_getStorageAt":
            let address = (list.first as? String ?? "").lowercased()
            let slot = (list.count > 1 ? list[1] as? String : nil)?.lowercased() ?? ""
            guard let safe = safes["\(chainId):\(address)"] else { return .result("0x" + String(repeating: "0", count: 64)) }
            let value: String? = switch slot {
            case Self.guardSlot: safe.guardAddress
            case Self.handlerSlot: safe.handler
            case Self.singletonSlot: safe.singleton
            default: nil
            }
            return .result("0x" + Self.wordAddress(value ?? "0x0"))
        case "eth_call":
            return ethCall(chainId, list.first as? [String: Any] ?? [:])
        case "eth_estimateGas":
            return .error(code: -32000, message: "gas required exceeds allowance")
        case "eth_getBlockByNumber":
            return .result(["number": "0x64", "timestamp": "0x" + String(Int(nowMs() / 1000), radix: 16)])
        case "eth_getLogs":
            return .result([Any]())
        case "eth_getTransactionReceipt":
            return .result(nil)
        case "eth_getBalance":
            return .result("0x56bc75e2d63100000")  // 100 coins
        default:
            return .error(code: -32601, message: "\(method) not here")
        }
    }

    private func ethCall(_ chain: Int, _ call: [String: Any]) -> MultisigRpcReply {
        let to = (call["to"] as? String ?? "").lowercased()
        let data = (call["data"] as? String ?? "0x").lowercased()
        if to == Self.genesisRegistry.lowercased() {
            return .error(code: 3, message: "execution reverted")
        }
        // `getConfiguration(address)` on Vela's shared passkey signer: nothing configured.
        if to == Self.webauthnSigner.lowercased(), data.hasPrefix("0xc44b11f7") {
            return .result("0x" + String(repeating: "0", count: 192))
        }
        guard let safe = safes["\(chain):\(to)"] else { return .result("0x") }
        let chars = Array(data)
        func slice(_ from: Int, _ to: Int) -> String { chars.count >= to ? String(chars[from..<to]) : "" }
        switch slice(0, 10) {
        case "0xa0e67e2b":  // getOwners()
            return .result("0x" + Self.wordU(32) + Self.wordU(safe.owners.count) + safe.owners.map(Self.wordAddress).joined())
        case "0xe75235b8":  // getThreshold()
            return .result("0x" + Self.wordU(safe.threshold))
        case "0xaffed0e0":  // nonce()
            return .result("0x" + Self.wordU(safe.nonce))
        case "0xcc2f8452":  // getModulesPaginated(address,uint256)
            return .result("0x" + Self.wordU(64) + Self.wordAddress("0x0000000000000000000000000000000000000001")
                + Self.wordU(safe.modules.count) + safe.modules.map(Self.wordAddress).joined())
        case "0x7d832974":  // approvedHashes(address,bytes32)
            let owner = "0x" + slice(10 + 24, 10 + 64)
            let hash = "0x" + slice(10 + 64, 10 + 128)
            return .result("0x" + Self.wordU(safe.approved.contains("\(owner):\(hash)") ? 1 : 0))
        default:
            return .result("0x")
        }
    }

    func storeRead(_ key: String) -> String?? { .some(store[key]) }
    func storeWrite(_ key: String, _ value: String?) -> Bool { store[key] = value; return true }
    /// A clock that runs: the world's start plus the time since.
    func nowMs() -> Double { Self.t0 + Date().timeIntervalSince(started) * 1000 + skewMs }
    /// R15: the clock moved on (a look grows stale without waiting for it).
    var skewMs = 0.0
    func random(_ bytes: Int) -> Data {
        randomByte &+= 1
        return Data(repeating: randomByte, count: bytes)
    }

    /// The owner's passkey: the parallel space's fixture key signs the
    /// challenge (a genuine WebAuthn assertion), as the app's spine does.
    func passkey(_ op: [String: Any]) async -> [String: Any] {
        let owner = op["owner"] as? String ?? ""
        let challenge = op["challenge"] as? String ?? ""
        passkeys.append((owner, challenge))
        if !passkeyAnswers.isEmpty { return passkeyAnswers.removeFirst() }
        guard let local = locals.first(where: { $0.address.caseInsensitiveCompare(owner) == .orderedSame }),
              let bytes = MultisigLive.bytes(challenge)
        else { return ["type": "passkey_failed", "detail": "not an account on this device"] }
        do {
            let signed = try fixtureAssert(challenge: bytes, allowCredentialIds: [local.credentialIdHex], preferred: nil)
            return [
                "type": "asserted",
                "credential_id": signed.credentialIdHex,
                "authenticator_data": MultisigLive.hex0x(signed.authenticatorDataHex),
                "client_data_json": MultisigLive.hex0x(signed.clientDataJsonHex),
                "signature": MultisigLive.hex0x(signed.signatureDerHex),
            ]
        } catch {
            return ["type": "passkey_failed", "detail": "\(error)"]
        }
    }

    func askRisk(_ op: [String: Any]) async -> Bool { true }
    func sendCalls(_ op: [String: Any]) async -> [String: Any] {
        sentFees.append((op["fee"] as? [String: Any])?["amount"] as? String ?? "")
        return ["type": "calls_sent", "user_op_hash": "0x" + String(repeating: "ab", count: 32), "tx_hash": NSNull()]
    }
    /// R15: what each execution's operation carried as its fee (`send_calls.fee.amount`).
    var sentFees: [String] = []
    /// R15: the fee the relay quotes now (the gas price moves).
    var quoteAmount = "0x1"
    func quoteCalls(_ op: [String: Any]) async -> [String: Any] {
        ["type": "quoted", "quote": ["kind": "quoted", "fee": ["token": NSNull(), "amount": quoteAmount], "totalGas": "0x1", "executorDeployed": true]]
    }
    /// §12: while set, a review's simulation and its recipients' facts are
    /// not answered — the review stays `loading`, as on a slow network.
    var holdReviews = false
    private func held() async {
        while holdReviews, !Task.isCancelled { try? await Task.sleep(nanoseconds: 5_000_000) }
    }
    func simulate(_ op: [String: Any]) async -> String? {
        await held()
        return nil
    }
    func recipientFacts(_ op: [String: Any]) async -> (isContract: Bool?, firstTime: Bool?) {
        await held()
        return (false, false)
    }
    func userOpStatus(_ op: [String: Any]) async -> (status: String?, txHash: String?)? { ("pending", nil) }
    func wallet(method: String, params: Any) async throws -> Any {
        throw MultisigWalletRefusal(code: 4200, message: "no wallet in this world")
    }
    func notice(_ notice: [String: Any]) async { notices.append(notice) }

    // MARK: Helpers

    static func wordAddress(_ address: String) -> String {
        let body = address.lowercased().hasPrefix("0x") ? String(address.lowercased().dropFirst(2)) : address.lowercased()
        return String(repeating: "0", count: max(0, 64 - body.count)) + body
    }

    static func wordU(_ n: Int) -> String {
        let body = String(n, radix: 16)
        return String(repeating: "0", count: 64 - body.count) + body
    }

    static func int(_ value: Any?) -> Int? { MultisigExecutor.int(value) }

    static func text(_ object: Any) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.fragmentsAllowed]) else { return "null" }
        return String(decoding: data, as: UTF8.self)
    }

    static func object(_ body: String?) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data((body ?? "{}").utf8)) as? [String: Any]) ?? [:]
    }

    /// One of the core's multisig doors (`multisig::call_json`), its answer read.
    static func door(_ method: String, _ args: [String: Any]) throws -> Any {
        let answer = try multisigCall(method: method, argsJson: CoreJSON.string(args))
        return try JSONSerialization.jsonObject(with: Data(answer.utf8), options: [.fragmentsAllowed])
    }

    static func safeTxHash(chain: Int, safe: String, tx: Any) -> String? {
        try? door("safeTxHash", ["chainId": chain, "safe": safe, "tx": tx]) as? String
    }
}

// MARK: - The flows

@MainActor
@Suite(.hangLimit)
struct MultisigFlowTests {

    /// Not a member here: an address nobody on this device holds.
    static let bo = "0x00000000000000000000000000000000000000bb"

    /// A 2-of-3 of the three fixture accounts, as the store keeps it; used on `chains`.
    private static func twoOfThree(_ salt: String, chains: [Int]) throws -> (record: [String: Any], address: String, owners: [String]) {
        let owners = try MsSimWorld.fixtures([0, 1, 2]).map(\.address)
        let g = try #require(try MsSimWorld.door("genesis", [
            "owners": owners, "threshold": 2, "name": "Quiet Harbor", "saltNonce": salt,
        ]) as? [String: Any])
        let address = try #require(g["address"] as? String)
        let record: [String: Any] = [
            "address": address, "genesis": g["genesis"] ?? [:], "owners": g["owners"] ?? owners,
            "threshold": 2, "recorded": true, "chainIds": chains, "addedMs": 1,
        ]
        return (record, address, owners)
    }

    /// The proposals machine started on this device's networks, following `record`.
    private static func following(_ world: MsSimWorld, _ record: [String: Any], networks: [Int]) async -> MultisigMachine {
        let machine = MultisigMachine(core: MultisigProposalsCore(), ports: world, name: "proposals")
        machine.dispatch([
            "type": "start", "service_url": MsSimWorld.service,
            "accounts": world.localsJson, "networks": networks, "known": [],
        ])
        machine.dispatch(["type": "follow", "account": record])
        await Wait.until({ machine.proposalsView.readAt != nil && !machine.proposalsView.loading }, orIdle: { machine.isIdle })
        return machine
    }

    /// `MultisigHub.propose`: the proposal drawn with its review, then
    /// "Propose and approve" as `member` — one passkey. Answers how it ended.
    private static func propose(
        _ machine: MultisigMachine, chainId: Int, _ proposal: [String: Any], as member: String
    ) async -> MsTagged? {
        func state() -> MsTagged? { machine.proposalsView.actions["propose"] }
        machine.dispatch(["type": "draft", "chain_id": chainId, "proposal": proposal])
        await Wait.until({ machine.proposalsView.draft != nil || (state().map { $0.kind != "working" } ?? false) },
                         orIdle: { machine.isIdle })
        guard machine.proposalsView.draft != nil else { return state() }
        await reviewRead { machine.proposalsView }
        machine.dispatch(["type": "confirm_draft", "as_member": member])
        await Wait.until({ state().map { $0.kind != "working" } ?? false }, orIdle: { machine.isIdle })
        return state()
    }

    /// §12: the person presses "Propose and approve" only once the draft's
    /// review is read — the sheet's door is shut until then (and the core
    /// refuses `signing_unchecked` before it) — so a test waits for it as
    /// they do: until it is `ready` or `failed` (the core bounds it).
    static func reviewRead(_ view: () -> MsProposalsView) async {
        await Wait.until {
            guard let draft = view().draft else { return true }
            guard let kind = draft.reviewState?.kind else { return false }
            return kind != "loading"
        }
    }

    private static func lower(_ value: Any?) -> String { (value as? String ?? "").lowercased() }

    // MARK: create

    /// Create a 2-of-2 from the create sheet's verbs: drafted, created with ONE
    /// passkey, its record imported on the index chain (100, not the network
    /// it is first used on) with the creator's owner proof, and kept here as
    /// recorded, shared and shown — the address the review showed.
    @Test func aCreatedMultisigIsRecordedOnTheIndexChainWithOnePasskey() async throws {
        let world = try MsSimWorld()
        world.locals = try MsSimWorld.fixtures([0])
        let me = world.locals[0].address
        let machine = MultisigMachine(core: MultisigAccountsCore(), ports: world, name: "accounts")
        defer { machine.dispose() }
        machine.dispatch([
            "type": "start", "service_url": MsSimWorld.service, "accounts": world.localsJson, "networks": [100, 8453],
        ])
        await Wait.until({ machine.accountsView.loaded }, orIdle: { machine.isIdle })
        #expect(machine.accountsView.loaded)

        machine.dispatch(["type": "new_draft"])
        machine.dispatch(["type": "draft", "owners": [me, Self.bo], "threshold": 2, "name": "Quiet Harbor"])
        await Wait.until({ machine.accountsView.create.draftAddress != nil }, orIdle: { machine.isIdle })
        let drafted = try #require(machine.accountsView.create.draftAddress, "the review shows no address")
        #expect(world.passkeys.isEmpty, "a draft asks for no passkey")

        machine.dispatch(["type": "create", "choice": [
            "owners": [me, Self.bo], "threshold": 2, "name": "Quiet Harbor", "chain_id": 8453, "creator": me,
            "members": [Self.bo: ["name": "Bo", "kind": "wallet"]],
        ]])
        await Wait.until({ [.done, .failed, .cancelled].contains(machine.accountsView.create.phase) }, orIdle: { machine.isIdle })
        let view = machine.accountsView
        #expect(view.create.phase == .done, "create ended \(view.create)")
        #expect(view.create.refused == nil && view.create.error == nil)
        #expect(world.passkeys.count == 1, "one passkey prompt")
        #expect(Self.lower(world.passkeys.first?.owner) == me.lowercased(), "the creator signs")
        let address = try #require(view.create.address)
        #expect(address == drafted, "the address the review showed")
        #expect(view.active?.lowercased() == address.lowercased(), "shown at once")
        #expect(world.notices.contains { $0["kind"] as? String == "activated" && Self.lower($0["address"]) == address.lowercased() })

        // Imported on the index chain, named, with the creator's proof.
        #expect(world.imports.count == 1)
        let imported = try #require(world.imports.first)
        #expect(MsSimWorld.int(imported["chainId"]) == 100, "the index chain, not the first network")
        #expect(Self.lower(imported["safe"]) == address.lowercased())
        #expect((imported["genesis"] as? [String: Any])?["name"] as? String == "Quiet Harbor", "sent named")
        let proof = try #require(imported["ownerProof"] as? [String: Any])
        #expect(Self.lower(proof["owner"]) == me.lowercased())
        #expect((proof["clientDataJSON"] as? String)?.hasPrefix(#"{"type":"webauthn.get""#) == true)
        #expect((proof["signature"] as? String)?.hasPrefix("0x") == true)
        #expect(world.records[address.lowercased()] != nil, "the index holds it")

        // Kept here: recorded, shared, on its first network, its names kept.
        let rows = world.storedAccounts()
        #expect(rows.count == 1)
        let row = try #require(rows.first)
        #expect(row["recorded"] as? Bool == true)
        #expect(row["shared"] as? Bool == true)
        #expect((row["chainIds"] as? [Any])?.compactMap(MsSimWorld.int) == [8453])
        #expect(row["firstRun"] as? Bool == true)
        #expect(((row["members"] as? [String: Any])?[Self.bo] as? [String: Any])?["name"] as? String == "Bo")
        #expect(world.store["vela.multisigPendingProofs"] == "{}", "no proof left waiting")
        #expect(world.store["vela.activeMultisig"]?.lowercased() == address.lowercased())
        #expect(view.accounts.first?.recordWaiting == false)
        #expect(view.accounts.first?.account.address.lowercased() == address.lowercased())
    }

    // MARK: reject

    /// A transfer proposed by one member; another member rejects it (the
    /// cancellation at the SAME number, proposed as that member — drawn with
    /// its review first, below): the mailbox holds both at one nonce, the
    /// cancellation approved by the rejecting member only, and the transfer
    /// is said voided by it.
    @Test func rejectingProposesTheSameNumberCancellationApprovedByThatMember() async throws {
        let world = try MsSimWorld()
        world.locals = try MsSimWorld.fixtures([0, 1])
        let (record, safe, owners) = try Self.twoOfThree("0x31", chains: [100])
        world.deploy(100, safe, SimSafe(owners: owners, threshold: 2))
        let machine = await Self.following(world, record, networks: [100, 8453])
        defer { machine.dispose() }

        // Proposed: one passkey, the mailbox has it.
        let proposed = await Self.propose(machine, chainId: 100, [
            "kind": "transfers", "transfers": [["token": NSNull(), "to": Self.bo, "amount": "1"]],
        ], as: owners[0])
        #expect(proposed?.kind == "done", "proposing ended \(String(describing: proposed?.raw))")
        let transferHash = try #require(proposed?["safe_tx_hash"]?.string)
        #expect(world.mailbox.count == 1)
        await Wait.until({ machine.proposalsView.items.contains { $0.safeTxHash == transferHash } }, orIdle: { machine.isIdle })
        let transfer = try #require(machine.proposalsView.items.first { $0.safeTxHash == transferHash })
        #expect(transfer.approved == 1 && transfer.threshold == 2)
        #expect(transfer.canApprove.contains { $0.caseInsensitiveCompare(owners[1]) == .orderedSame },
                "the other member here can still approve — or reject")

        // Rejected by the other member: a cancellation at the same number.
        let rejected = await Self.propose(machine, chainId: 100, ["kind": "cancel", "nonce": transfer.nonce], as: owners[1])
        #expect(rejected?.kind == "done", "rejecting ended \(String(describing: rejected?.raw))")
        let cancelHash = try #require(rejected?["safe_tx_hash"]?.string)
        #expect(cancelHash != transferHash)
        #expect(world.passkeys.map { $0.owner.lowercased() } == [owners[0].lowercased(), owners[1].lowercased()],
                "one passkey each: the proposer, then the member who rejects")

        // The mailbox: both, at one nonce; the cancellation is to the
        // multisig itself, empty, approved by the rejecting member.
        #expect(world.submits.count == 2)
        let cancelTx = try #require(world.submits.last?["safeTx"] as? [String: Any])
        #expect(Self.lower(cancelTx["to"]) == safe.lowercased(), "to the multisig itself")
        #expect(Self.lower(cancelTx["data"]) == "0x", "it calls nothing")
        #expect(["0x0", "0"].contains(Self.lower(cancelTx["value"])), "nothing moves: \(cancelTx["value"] ?? "")")
        #expect(Self.lower((world.submits.last?["approval"] as? [String: Any])?["owner"]) == owners[1].lowercased())
        #expect(world.mailbox.count == 2)
        let nonces = Set(world.mailbox.compactMap { ($0["safeTx"] as? [String: Any])?["nonce"] }.map { "\($0)" })
        #expect(nonces.count == 1, "both at one nonce: \(nonces)")
        let cancelStored = try #require(world.mailbox.first { Self.lower($0["safeTxHash"]) == cancelHash.lowercased() })
        #expect((cancelStored["approvals"] as? [[String: Any]])?.map { Self.lower($0["owner"]) } == [owners[1].lowercased()])

        // Read back: the transfer knows the cancellation that voids it, and
        // the cancellation is counted to the member who rejected.
        machine.dispatch(["type": "refresh", "fresh": true])
        await Wait.until({
            machine.proposalsView.items.first { $0.safeTxHash == transferHash }?.cancelledBy == cancelHash
        }, orIdle: { machine.isIdle })
        let items = machine.proposalsView.items
        let voided = try #require(items.first { $0.safeTxHash == transferHash })
        let cancel = try #require(items.first { $0.safeTxHash == cancelHash })
        #expect(voided.cancelledBy == cancelHash)
        #expect(voided.sameNonce.contains(cancelHash))
        #expect(cancel.nonce == transfer.nonce)
        #expect(cancel.action?.kind == "cancel")
        #expect(cancel.approvals.map { $0.owner.lowercased() } == [owners[1].lowercased()])
        #expect(cancel.approvals.first?.counts == true, "a member's verified approval counts")
        #expect(cancel.approved == 1)
    }

    // MARK: reject — the cancellation's review first (r9-13)

    /// The app's hub over the world (`MultisigHub(ports:)`), its proposals
    /// machine following a 2-of-3 deployed on Gnosis, two of whose members
    /// are here.
    private static func hubFollowing(
        _ world: MsSimWorld, salt: String
    ) async throws -> (hub: MultisigHub, record: [String: Any], safe: String, owners: [String]) {
        world.locals = try MsSimWorld.fixtures([0, 1])
        let (record, safe, owners) = try Self.twoOfThree(salt, chains: [100])
        world.deploy(100, safe, SimSafe(owners: owners, threshold: 2))
        let shelf = VelaStore(defaults: UserDefaults(suiteName: "vela.tests.msreject.\(UUID().uuidString)")!)
        let localsJson = world.localsJson
        let hub = MultisigHub(
            ports: world, loc: Loc(overrideTag: "en", preferredLanguages: []), shelf: shelf, tokens: nil,
            facts: { .init(serviceUrl: MsSimWorld.service, accounts: localsJson, networks: [100, 8453], space: [], known: []) }
        )
        hub.proposals.dispatch([
            "type": "start", "service_url": MsSimWorld.service,
            "accounts": world.localsJson, "networks": [100, 8453], "known": [],
        ])
        hub.proposals.dispatch(["type": "follow", "account": record])
        await Wait.until({ hub.proposalsView.readAt != nil && !hub.proposalsView.loading }, orIdle: { hub.proposals.isIdle })
        return (hub, record, safe, owners)
    }

    /// `hubFollowing`; a transfer proposed by `owners[0]` and read back; the
    /// transfer's own sheet open, as when Reject is pressed.
    private static func rejecting(
        _ world: MsSimWorld, salt: String
    ) async throws -> (hub: MultisigHub, safe: String, owners: [String], transfer: MsProposalItem) {
        let (hub, _, safe, owners) = try await Self.hubFollowing(world, salt: salt)
        let proposed = await Self.propose(hub.proposals, chainId: 100, [
            "kind": "transfers", "transfers": [["token": NSNull(), "to": Self.bo, "amount": "1"]],
        ], as: owners[0])
        #expect(proposed?.kind == "done", "proposing ended \(String(describing: proposed?.raw))")
        let hash = try #require(proposed?["safe_tx_hash"]?.string)
        await Wait.until({ hub.proposalsView.items.contains { $0.safeTxHash == hash } }, orIdle: { hub.proposals.isIdle })
        let transfer = try #require(hub.proposalsView.items.first { $0.safeTxHash == hash })
        hub.sheet = .proposal(chainId: 100, safeTxHash: hash)
        return (hub, safe, owners, transfer)
    }

    /// Reject as `member` (`MultisigProposalSheet.reject` → `MultisigHub.reject`),
    /// waited on until its review is up — or until it ended without one.
    private static func pressReject(
        _ hub: MultisigHub, _ transfer: MsProposalItem, as member: String
    ) async -> Task<MsTagged?, Never> {
        let settled = Flag()
        let pressed = Task { @MainActor in
            defer { settled.set() }
            return await hub.reject(chainId: 100, safeTxHash: transfer.safeTxHash, nonce: transfer.nonce, as: member)
        }
        // The press reaching the point the person holds it at (its review),
        // or ending without one — not the machine's idleness, which holds
        // before the press has asked anything.
        await Wait.until { (hub.sheet == .review(chainId: 100) && hub.proposalsView.draft != nil) || settled.isSet }
        return pressed
    }

    /// r9-13: Reject is a proposal like any other. The question answered,
    /// the cancellation at the transfer's number is DRAWN for its review —
    /// the core's review of it, the member who rejects offered first — and
    /// no passkey runs and nothing reaches the mailbox until the person
    /// proposes it there. Proposed: one passkey, as that member; the
    /// transfer's sheet is back; nothing is said.
    @Test func rejectingShowsTheCancellationsReviewBeforeItsPasskey() async throws {
        let world = try MsSimWorld()
        let (hub, safe, owners, transfer) = try await Self.rejecting(world, salt: "0x41")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        #expect(world.passkeys.count == 1 && world.submits.count == 1, "the transfer, proposed")

        let pressed = await Self.pressReject(hub, transfer, as: owners[1])

        // Its review, before any signature.
        #expect(hub.sheet == .review(chainId: 100), "the review sheet stands in for the transfer's")
        // The core draws the draft first and its review once gathered (the
        // chain read, the simulation): waited for, as the person waits for it.
        await Wait.until({ hub.proposalsView.draft.map { !$0.review.isNull } ?? false }, orIdle: { hub.proposals.isIdle })
        let draft = try #require(hub.proposalsView.draft, "the cancellation is drawn for its review")
        #expect(draft.item.action?.kind == "cancel")
        #expect(draft.item.nonce == transfer.nonce, "at the transfer's number")
        #expect(draft.item.tx["to"]?.string?.lowercased() == safe.lowercased(), "to the multisig itself")
        #expect(!draft.review.isNull, "the core's review of it is drawn")
        #expect(draft.review["sign"]?["allowed"]?.bool != false, "and it may be signed")
        #expect(world.passkeys.count == 1, "no passkey before the person proposes it on its review")
        #expect(world.submits.count == 1, "nothing sent before it is proposed")
        // The member who rejects is the one offered first (not the machine's default).
        let first = MultisigDraftReviewSheet.firstMember(draft, acting: hub.reviewAs)
        #expect(first?.lowercased() == owners[1].lowercased(), "offered first: \(first ?? "nobody")")
        #expect(hub.proposalsView.action("propose")?.kind != "working", "waiting on the person, not working")

        // Proposed on its review, as that member: one passkey, the cancellation sent.
        await Self.reviewRead { hub.proposalsView }
        hub.confirmDraft(as: first)
        let settled = await pressed.value
        #expect(settled?.kind == "done", "rejecting ended \(String(describing: settled?.raw))")
        #expect(world.passkeys.map { $0.owner.lowercased() } == [owners[0].lowercased(), owners[1].lowercased()])
        #expect(world.submits.count == 2)
        let cancelTx = try #require(world.submits.last?["safeTx"] as? [String: Any])
        #expect(Self.lower(cancelTx["to"]) == safe.lowercased())
        #expect(Self.lower(cancelTx["data"]) == "0x")
        #expect("\(cancelTx["nonce"] ?? "")" == "\((world.submits.first?["safeTx"] as? [String: Any])?["nonce"] ?? "")")
        #expect(Self.lower((world.submits.last?["approval"] as? [String: Any])?["owner"]) == owners[1].lowercased())
        #expect(hub.sheet == .proposal(chainId: 100, safeTxHash: transfer.safeTxHash), "the transfer's sheet is back")
        #expect(hub.reviewAs == nil)
        #expect(hub.rejectFailure(chainId: 100, safeTxHash: transfer.safeTxHash) == nil, "nothing to say")
    }

    /// r9-13: withdrawn on the cancellation's review, nothing is signed,
    /// nothing is sent, and nothing is said — the transfer's sheet is back
    /// with Reject on offer again, the transfer voided by nothing.
    @Test func aRejectionWithdrawnOnItsReviewSignsNothingAndSaysNothing() async throws {
        let world = try MsSimWorld()
        let (hub, _, owners, transfer) = try await Self.rejecting(world, salt: "0x42")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }

        let pressed = await Self.pressReject(hub, transfer, as: owners[1])
        #expect(hub.proposalsView.draft?.item.action?.kind == "cancel", "its review is up")

        hub.withdrawDraft()  // "Don't propose" / the review sheet's close
        let settled = await pressed.value
        #expect(settled?.kind == "cancelled", "\(String(describing: settled?.raw))")
        #expect(world.passkeys.count == 1, "no passkey: only the transfer's")
        #expect(world.submits.count == 1 && world.mailbox.count == 1, "nothing sent")
        #expect(hub.proposalsView.draft == nil)
        #expect(hub.sheet == .proposal(chainId: 100, safeTxHash: transfer.safeTxHash), "the transfer's sheet is back")
        #expect(hub.rejectFailure(chainId: 100, safeTxHash: transfer.safeTxHash) == nil)

        // The transfer's sheet: no failure said, nothing voids it, Reject still on offer.
        let item = try #require(hub.proposalsView.items.first { $0.safeTxHash == transfer.safeTxHash })
        let model = MsProposalModel(
            item: item, view: hub.proposalsView, words: MsWords(loc: hub.loc, hub: nil), account: nil,
            rejectState: hub.rejectFailure(chainId: 100, safeTxHash: transfer.safeTxHash)
        )
        #expect(!model.notes.contains { $0.tone == .danger }, "said: \(model.notes.map(\.text))")
        #expect(item.cancelledBy == nil)
        #expect(item.canApprove.contains { $0.caseInsensitiveCompare(owners[1]) == .orderedSame })
    }

    /// Proposed on its review, the passkey closed by the person: their word,
    /// nothing said. Timed out: that is said on the transfer's sheet (the
    /// review stood in for it meanwhile) — until Reject is pressed again.
    @Test func aRejectionsPasskeyClosedSaysNothingButOneTimedOutIsSaid() async throws {
        let world = try MsSimWorld()
        let (hub, _, owners, transfer) = try await Self.rejecting(world, salt: "0x43")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        let words = MsWords(loc: hub.loc, hub: nil)
        // What the sheet says: its notes, and (r16-5) the line beside Reject.
        func notes() throws -> [(text: String, tone: MsNote.Tone, id: String)] {
            let item = try #require(hub.proposalsView.items.first { $0.safeTxHash == transfer.safeTxHash })
            let model = MsProposalModel(
                item: item, view: hub.proposalsView, words: words, account: nil,
                rejectState: hub.rejectFailure(chainId: 100, safeTxHash: transfer.safeTxHash)
            )
            return model.notes + (model.rejectLine.map { [($0.text, $0.tone, "rejectFailed")] } ?? [])
        }

        // Closed.
        world.passkeyAnswers = [["type": "passkey_cancelled"]]
        var pressed = await Self.pressReject(hub, transfer, as: owners[1])
        await Self.reviewRead { hub.proposalsView }
        hub.confirmDraft(as: owners[1])
        var settled = await pressed.value
        #expect(settled?.kind != "done")
        #expect(world.passkeys.count == 2, "the passkey was asked, after its review")
        #expect(world.submits.count == 1, "nothing sent")
        #expect(hub.rejectFailure(chainId: 100, safeTxHash: transfer.safeTxHash) == nil, "closed: \(String(describing: settled?.raw))")
        #expect(try !notes().contains { $0.id == "rejectFailed" })

        // Timed out.
        world.passkeyAnswers = [[
            "type": "passkey_failed",
            "detail": MultisigShellFailure.passkey(failure: "cancelled", timedOut: true, message: nil).detail,
        ]]
        pressed = await Self.pressReject(hub, transfer, as: owners[1])
        #expect(hub.rejectFailure(chainId: 100, safeTxHash: transfer.safeTxHash) == nil, "a new press starts clean")
        await Self.reviewRead { hub.proposalsView }
        hub.confirmDraft(as: owners[1])
        settled = await pressed.value
        #expect(settled?.kind == "failed", "\(String(describing: settled?.raw))")
        #expect(world.submits.count == 1, "nothing sent")
        let said = try notes().first { $0.id == "rejectFailed" }
        #expect(said?.tone == .danger)
        #expect(said?.text == words.t("proposal.reason.passkeyTimedOut"), "said: \(said?.text ?? "nothing")")
        #expect(hub.sheet == .proposal(chainId: 100, safeTxHash: transfer.safeTxHash))
    }

    // MARK: §12 — every review has a state; nothing is signed before it is read

    /// The iOS lane's bug: the core draws the draft first and its review a
    /// moment later, and "Propose and approve" was pressable in between
    /// (`?? true`). Now: while it is read the sheet's door (the core's
    /// `review.door`) is shut and says "Reading the transaction…"; pressed
    /// anyway, the core's own backstop signs nothing (`signing_unchecked`);
    /// read, it opens; "Try again" reads it again (shut meanwhile); then
    /// proposed, with one passkey.
    @Test func proposeIsShutWhileTheDraftsReviewIsReadAndOpensOnceReady() async throws {
        let world = try MsSimWorld()
        let (hub, _, _, owners) = try await Self.hubFollowing(world, salt: "0x61")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        let words = MsWords(loc: hub.loc, hub: nil)

        world.holdReviews = true
        hub.proposals.dispatch(["type": "draft", "chain_id": 100, "proposal": [
            "kind": "transfers", "transfers": [["token": NSNull(), "to": Self.bo, "amount": "1"]],
        ]])
        await Wait.until { hub.proposalsView.draft != nil }
        let loading = try #require(hub.proposalsView.draft)
        #expect(loading.reviewState?.kind == "loading", "drawn before its review: \(String(describing: loading.reviewState))")
        let shut = MultisigDraftReviewSheet.door(loading)
        #expect(shut.shows == .loading)
        #expect(!shut.enabled, "Propose and approve before its review is blind signing")
        #expect(words.t("review.state.loading") == "Reading the transaction…")

        // A shell that let it be pressed: the core signs nothing.
        hub.confirmDraft(as: owners[0])
        await Wait.until({ hub.proposalsView.action("propose").map { $0.kind != "working" } ?? false },
                         orIdle: { hub.proposals.isIdle })
        #expect(hub.proposalsView.action("propose")?.kind == "signingUnchecked",
                "\(String(describing: hub.proposalsView.action("propose")?.raw))")
        #expect(world.passkeys.isEmpty && world.submits.isEmpty, "nothing signed, nothing sent")
        #expect(hub.proposalsView.draft != nil, "the draft is still there to review")

        // Read: the door opens on the review's own rule.
        world.holdReviews = false
        await Self.reviewRead { hub.proposalsView }
        let ready = try #require(hub.proposalsView.draft)
        #expect(ready.reviewState?.kind == "ready")
        #expect(!ready.review.isNull)
        #expect(MultisigDraftReviewSheet.door(ready).enabled)

        // "Try again" (`review_again`, no fields: the draft's): read again, shut meanwhile.
        world.holdReviews = true
        hub.sendProposal(["type": "review_again"])
        await Wait.until { hub.proposalsView.draft?.reviewState?.kind == "loading" }
        let again = try #require(hub.proposalsView.draft)
        #expect(!MultisigDraftReviewSheet.door(again).enabled, "shut while it is read again")
        world.holdReviews = false
        await Self.reviewRead { hub.proposalsView }
        let reread = try #require(hub.proposalsView.draft)
        #expect(reread.reviewState?.kind == "ready")
        #expect(MultisigDraftReviewSheet.door(reread).enabled)

        hub.confirmDraft(as: owners[0])
        await Wait.until({ hub.proposalsView.action("propose").map { $0.kind != "working" } ?? false },
                         orIdle: { hub.proposals.isIdle })
        #expect(hub.proposalsView.action("propose")?.kind == "done",
                "\(String(describing: hub.proposalsView.action("propose")?.raw))")
        #expect(world.passkeys.count == 1, "one passkey, after its review")
    }

    /// A proposal's review: one not in the list is `failed {gone}` — said,
    /// shut, with "Try again"; one read again (`review_again` with its chain
    /// and hash) is `loading` — Approve shut — until it is `ready` again.
    @Test func approveIsShutWhileAProposalsReviewIsReadAndAFailedOneOffersTryAgain() async throws {
        let world = try MsSimWorld()
        let (hub, _, owners, transfer) = try await Self.rejecting(world, salt: "0x62")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        let words = MsWords(loc: hub.loc, hub: nil)
        func model() throws -> MsProposalModel {
            let item = try #require(hub.proposalsView.items.first { $0.safeTxHash == transfer.safeTxHash })
            return MsProposalModel(item: item, view: hub.proposalsView, words: words, account: nil)
        }
        func kind() -> String? {
            hub.proposalsView.lookup(hub.proposalsView.reviewStates, chainId: 100, hash: transfer.safeTxHash)?.kind
        }

        // Proposed from its review: that review is its own, read.
        hub.sendProposal(["type": "review", "chain_id": 100, "safe_tx_hash": transfer.safeTxHash, "blind": false])
        await Wait.until({ kind() == "ready" }, orIdle: { hub.proposals.isIdle })
        #expect(try model().signable, "owners[1] may approve it once it is read")
        #expect(try model().canApprove.contains { $0.caseInsensitiveCompare(owners[1]) == .orderedSame })

        // "Try again" on it: loading at once, Approve shut, then ready again.
        world.holdReviews = true
        hub.sendProposal(["type": "review_again", "chain_id": 100, "safe_tx_hash": transfer.safeTxHash])
        await Wait.until { kind() == "loading" }
        #expect(try model().reviewState == .loading)
        #expect(try !model().signable, "Approve while its review is read again is blind")
        #expect(try model().door.shows == .loading)
        world.holdReviews = false
        await Wait.until { kind() == "ready" }
        #expect(try model().signable)

        // One no longer in the list: failed, with its reason and Try again.
        let gone = "0x" + String(repeating: "de", count: 32)
        hub.sendProposal(["type": "review", "chain_id": 100, "safe_tx_hash": gone, "blind": false])
        await Wait.until {
            hub.proposalsView.lookup(hub.proposalsView.reviewStates, chainId: 100, hash: gone)?.kind == "failed"
        }
        let state = hub.proposalsView.lookup(hub.proposalsView.reviewStates, chainId: 100, hash: gone)
        #expect(state?["reason_key"]?.string == "home.multisig.review.state.gone")
        let door = MsRules.reviewDoor(state: state, review: nil)
        #expect(door.shows == .failed && !door.enabled && door.retry)
        #expect(MsReviewPending.failedLine(door, words: words)
                == "\(words.t("review.state.gone")) \(words.t("review.state.blocked"))")
    }

    // MARK: r15-5 — the fee-moved stop, said where it was pressed

    /// iOS R15: the first press after the fee moved "signed nothing and said
    /// nothing". Through the hub over the world: the one press on a look
    /// grown stale whose new quote differs signs nothing and fails
    /// `fee_moved` — said by the sheet beside that press, in the core's words
    /// with one "That didn't go through:" — and the next press signs exactly
    /// the new figure.
    @Test func aFeeThatMovedStopsThePressSaidBesideItAndTheNextPressSignsTheNewFigure() async throws {
        let world = try MsSimWorld()
        let (hub, _, owners, transfer) = try await Self.rejecting(world, salt: "0x6f")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        let hash = transfer.safeTxHash
        let words = MsWords(loc: hub.loc, hub: nil)
        func item() -> MsProposalItem? { hub.proposalsView.items.first { $0.safeTxHash == hash } }
        func execute() -> MsTagged? { hub.proposalsView.action("execute:\(hash)") }

        hub.sendProposal(["type": "review", "chain_id": 100, "safe_tx_hash": hash, "blind": false])
        await Wait.until({
            ["ready", "unchecked"].contains(hub.proposalsView.lookup(hub.proposalsView.reviewStates, chainId: 100, hash: hash)?.kind ?? "")
        }, orIdle: { hub.proposals.isIdle })
        hub.sendProposal(["type": "preview", "chain_id": 100, "safe_tx_hash": hash, "executor": owners[1]])
        await Wait.until({ item().map { !hub.proposalsView.approveExecute($0).isEmpty } ?? false }, orIdle: { hub.proposals.isIdle })
        #expect(item().map { hub.proposalsView.approveExecute($0) } == [owners[1]], "the one press is offered")

        // The look on screen grows stale and the relay's price moves.
        world.quoteAmount = "0x2"
        world.skewMs = 20_000
        hub.sendProposal(["type": "approve_execute", "chain_id": 100, "safe_tx_hash": hash, "executor": owners[1]])
        await Wait.until({ execute().map { $0.kind != "working" } ?? false }, orIdle: { hub.proposals.isIdle })
        #expect(execute()?.kind == "failed", "\(String(describing: execute()?.raw))")
        #expect(world.sentFees.isEmpty, "nothing signed at a fee nobody was shown")

        let model = MsProposalModel(item: try #require(item()), view: hub.proposalsView, words: words, account: nil)
        let said = model.pressFailures(at: .approve)
        #expect(said.map(\.id) == ["executeFailed"], "said beside the one press: \(said)")
        let line = try #require(said.first?.text)
        let prefix = words.t("proposal.failed", ["reason": ""]).trimmingCharacters(in: .whitespaces)
        #expect(line.hasPrefix(prefix) && line.components(separatedBy: prefix).count == 2, "\(line)")
        #expect(!model.notes.contains { $0.id == "executeFailed" }, "not (only) at the head of the sheet")

        // The sheet as drawn, scrolled down to the press.
        let screen = HostedScreen(MultisigProposalSheet(hub: hub, chainId: 100, safeTxHash: hash, words: words, onClose: {}))
        defer { screen.close() }
        let seen = try await screen.read { $0.element("multisig.proposal.note.executeFailed") != nil }
        let note = try #require(seen.element("multisig.proposal.note.executeFailed"))
        // Said where the one press is: right by "Approve and execute".
        if let press = seen.element("multisig.proposal.approveExecute") {
            #expect(abs(note.frame.minY - press.frame.maxY) < 200, "\(note.frame) vs \(press.frame)")
        }
        try await screen.scroll(by: max(0, note.frame.minY - 420))
        try await screen.save("i-r15-5-feemoved-after")

        // Pressed again on the figure now shown: signed at exactly it.
        hub.sendProposal(["type": "approve_execute", "chain_id": 100, "safe_tx_hash": hash, "executor": owners[1]])
        await Wait.until({ !world.sentFees.isEmpty }, orIdle: { hub.proposals.isIdle })
        #expect(world.sentFees == ["0x2"])
    }

    // MARK: §24 natives carry the fee shown

    /// The sheet painted `executor`'s look priced at 0x1; the relay's price
    /// then moved and a new look (0x2, fresh) landed before the press —
    /// never drawn. The model as painted.
    private static func aQuoteLandsAfterThePaint(
        _ world: MsSimWorld, _ hub: MultisigHub, hash: String, executor: String, words: MsWords
    ) async throws -> MsProposalModel {
        func item() -> MsProposalItem? { hub.proposalsView.items.first { $0.safeTxHash == hash } }
        func look() -> MsJSON? { item().flatMap { hub.proposalsView.preview($0, executor: executor) } }
        hub.sendProposal(["type": "preview", "chain_id": 100, "safe_tx_hash": hash, "executor": executor])
        await Wait.until({ look()?.kind == "priced" }, orIdle: { hub.proposals.isIdle })
        let painted = MsProposalModel(item: try #require(item()), view: hub.proposalsView, words: words, account: nil)
        #expect(painted.shownFee(for: executor) == "0x1", "the figure drawn")
        // The look grows stale, the price moves, and a new look lands — fresh.
        world.quoteAmount = "0x2"
        world.skewMs = 20_000
        hub.sendProposal(["type": "preview", "chain_id": 100, "safe_tx_hash": hash, "executor": executor])
        await Wait.until({ look()?["fee"]?["amount"]?.string == "0x2" }, orIdle: { hub.proposals.isIdle })
        #expect(look()?["fee"]?["amount"]?.string == "0x2", "the new look, not drawn")
        return painted
    }

    /// §24: the one press "Approve and execute" carries the figure drawn
    /// under it (`shown_fee`): a quote that landed between the paint and the
    /// press signs nothing (`fee_moved`), said beside the one press once in
    /// the core's words; the next press signs the new figure.
    @Test func theOnePressCarriesTheFeeDrawnSoAQuoteLandedAfterThePaintSignsNothing() async throws {
        let world = try MsSimWorld()
        let (hub, _, owners, transfer) = try await Self.rejecting(world, salt: "0x70")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        let hash = transfer.safeTxHash
        let words = MsWords(loc: hub.loc, hub: nil)
        func item() -> MsProposalItem? { hub.proposalsView.items.first { $0.safeTxHash == hash } }
        func execute() -> MsTagged? { hub.proposalsView.action("execute:\(hash)") }

        hub.sendProposal(["type": "review", "chain_id": 100, "safe_tx_hash": hash, "blind": false])
        await Wait.until({
            ["ready", "unchecked"].contains(hub.proposalsView.lookup(hub.proposalsView.reviewStates, chainId: 100, hash: hash)?.kind ?? "")
        }, orIdle: { hub.proposals.isIdle })
        let painted = try await Self.aQuoteLandsAfterThePaint(world, hub, hash: hash, executor: owners[1], words: words)
        await Wait.until({ item().map { !hub.proposalsView.approveExecute($0).isEmpty } ?? false }, orIdle: { hub.proposals.isIdle })
        #expect(item().map { hub.proposalsView.approveExecute($0) } == [owners[1]], "the one press is offered")

        let press = painted.executeEvent(executor: owners[1], oneTouch: true)
        #expect(press["type"] as? String == "approve_execute")
        #expect(press["shown_fee"] as? String == "0x1")
        hub.sendProposal(press)
        await Wait.until({ execute().map { $0.kind != "working" } ?? false }, orIdle: { hub.proposals.isIdle })
        #expect(execute()?.kind == "failed", "\(String(describing: execute()?.raw))")
        #expect(world.sentFees.isEmpty, "nothing signed at a fee nobody was shown")

        let now = MsProposalModel(item: try #require(item()), view: hub.proposalsView, words: words, account: nil)
        let said = now.pressFailures(at: .approve)
        #expect(said.map(\.id) == ["executeFailed"], "said beside the one press: \(said)")
        let line = try #require(said.first?.text)
        let prefix = words.t("proposal.failed", ["reason": ""]).trimmingCharacters(in: .whitespaces)
        #expect(line.hasPrefix(prefix) && line.components(separatedBy: prefix).count == 2, "\(line)")
        #expect(!now.notes.contains { $0.id == "executeFailed" }, "not at the head of the sheet too")
        #expect(now.shownFee(for: owners[1]) == "0x2", "the next press carries the figure now drawn")

        hub.sendProposal(now.executeEvent(executor: owners[1], oneTouch: true))
        await Wait.until({ !world.sentFees.isEmpty }, orIdle: { hub.proposals.isIdle })
        #expect(world.sentFees == ["0x2"])
    }

    /// §24: Execute carries the figure drawn above it, the same way; no
    /// figure drawn, none carried.
    @Test func executeCarriesTheFeeDrawnSoAQuoteLandedAfterThePaintSignsNothing() async throws {
        let world = try MsSimWorld()
        let (hub, _, owners, transfer) = try await Self.rejecting(world, salt: "0x71")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        let hash = transfer.safeTxHash
        let words = MsWords(loc: hub.loc, hub: nil)
        func item() -> MsProposalItem? { hub.proposalsView.items.first { $0.safeTxHash == hash } }
        func execute() -> MsTagged? { hub.proposalsView.action("execute:\(hash)") }

        hub.sendProposal(["type": "review", "chain_id": 100, "safe_tx_hash": hash, "blind": false])
        await Wait.until({
            ["ready", "unchecked"].contains(hub.proposalsView.lookup(hub.proposalsView.reviewStates, chainId: 100, hash: hash)?.kind ?? "")
        }, orIdle: { hub.proposals.isIdle })
        hub.sendProposal(["type": "approve", "chain_id": 100, "safe_tx_hash": hash, "owner": owners[1]])
        await Wait.until({ item()?.status == "ready" }, orIdle: { hub.proposals.isIdle })
        #expect(item()?.status == "ready")

        // Not looked at yet: "Checking the fee…" — no figure drawn, none carried.
        let unpriced = MsProposalModel(item: try #require(item()), view: hub.proposalsView, words: words, account: nil)
        let bare = unpriced.executeEvent(executor: "0x0000000000000000000000000000000000000001", oneTouch: false)
        #expect(bare["shown_fee"] == nil)
        #expect(Set(bare.keys) == ["type", "chain_id", "safe_tx_hash", "executor"])

        let painted = try await Self.aQuoteLandsAfterThePaint(world, hub, hash: hash, executor: owners[0], words: words)
        let press = painted.executeEvent(executor: owners[0], oneTouch: false)
        #expect(press["type"] as? String == "execute")
        #expect(press["shown_fee"] as? String == "0x1")
        hub.sendProposal(press)
        await Wait.until({ execute().map { $0.kind != "working" } ?? false }, orIdle: { hub.proposals.isIdle })
        #expect(execute()?.kind == "failed", "\(String(describing: execute()?.raw))")
        #expect(world.sentFees.isEmpty, "nothing signed at a fee nobody was shown")

        let now = MsProposalModel(item: try #require(item()), view: hub.proposalsView, words: words, account: nil)
        let said = now.pressFailures(at: .execute)
        #expect(said.map(\.id) == ["executeFailed"], "said beside Execute: \(said)")
        #expect(now.pressFailures(at: .approve).isEmpty && now.pressFailures(at: .top).isEmpty, "said once")
        #expect(now.shownFee(for: owners[0]) == "0x2")

        hub.sendProposal(now.executeEvent(executor: owners[0], oneTouch: false))
        await Wait.until({ !world.sentFees.isEmpty }, orIdle: { hub.proposals.isIdle })
        #expect(world.sentFees == ["0x2"])
    }

    // MARK: a review swiped away — withdrawn at once

    /// The send's own review swiped down (`MultisigHub.sheetDismissed`, the
    /// root's sheet binding set to nothing): withdrawn exactly as "Don't
    /// propose" — the send told `cancelled` at once (not at the wait's
    /// 600 s limit), no passkey, nothing sent, no sheet left. Proposed again,
    /// it goes: the person can always come back.
    @Test func aSendsReviewSwipedAwayIsWithdrawnAtOnceAndCanBeProposedAgain() async throws {
        let world = try MsSimWorld()
        let (hub, _, safe, owners) = try await Self.hubFollowing(world, salt: "0x51")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        let op: [String: Any] = [
            "chain_id": 100, "multisig": safe,
            "transfers": [["token": NSNull(), "to": Self.bo, "amount": "1"]],
        ]
        func send() async -> Task<[String: Any], Never> {
            let answered = Flag()
            let sending = Task { @MainActor in
                defer { answered.set() }
                return await hub.proposeFromSend(op)
            }
            await Wait.until { (hub.sheet == .review(chainId: 100) && hub.proposalsView.draft != nil) || answered.isSet }
            return sending
        }

        var sending = await send()
        #expect(hub.sheet == .review(chainId: 100), "its review is up")
        #expect(world.passkeys.isEmpty && world.submits.isEmpty)

        let swiped = ContinuousClock.now
        hub.sheetDismissed()
        #expect(hub.sheet == nil, "nothing was under it: no sheet left")
        let answer = await sending.value
        #expect(ContinuousClock.now - swiped < .seconds(5), "answered at once, not at the wait's limit")
        #expect(answer["type"] as? String == "propose_failed")
        #expect(answer["cancelled"] as? Bool == true, "the person's word, not a failure: \(answer)")
        #expect(hub.proposalsView.draft == nil, "withdrawn")
        #expect(hub.proposalsView.action("propose")?.kind == "cancelled")
        #expect(world.passkeys.isEmpty, "nothing signed")
        #expect(world.submits.isEmpty && world.mailbox.isEmpty, "nothing sent")

        // Again, and proposed on its review this time.
        sending = await send()
        #expect(hub.sheet == .review(chainId: 100), "its review is up again")
        await Self.reviewRead { hub.proposalsView }
        hub.confirmDraft(as: owners[0])
        let proposed = await sending.value
        #expect(proposed["type"] as? String == "proposed", "\(proposed)")
        #expect(world.passkeys.count == 1 && world.mailbox.count == 1)
    }

    /// Reject's review (r9-13) swiped down: withdrawn at once, as "Don't
    /// propose" — nothing signed or said, and the transaction's sheet it
    /// covered is shown again.
    @Test func aRejectionsReviewSwipedAwayIsWithdrawnAndItsTransactionShownAgain() async throws {
        let world = try MsSimWorld()
        let (hub, _, owners, transfer) = try await Self.rejecting(world, salt: "0x52")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }

        let pressed = await Self.pressReject(hub, transfer, as: owners[1])
        #expect(hub.sheet == .review(chainId: 100))
        let swiped = ContinuousClock.now
        hub.sheetDismissed()
        #expect(hub.sheet == .proposal(chainId: 100, safeTxHash: transfer.safeTxHash), "the transaction's sheet, at once")
        let settled = await pressed.value
        #expect(ContinuousClock.now - swiped < .seconds(5), "answered at once, not at the wait's limit")
        #expect(settled?.kind == "cancelled", "\(String(describing: settled?.raw))")
        #expect(world.passkeys.count == 1 && world.submits.count == 1, "only the transfer's")
        #expect(hub.proposalsView.draft == nil)
        #expect(hub.rejectFailure(chainId: 100, safeTxHash: transfer.safeTxHash) == nil, "nothing said")
        #expect(hub.sheet == .proposal(chainId: 100, safeTxHash: transfer.safeTxHash), "and it stays")
    }

    /// A site's request on its review (M5), swiped down: left as its close
    /// leaves it (`dapp_leave`) — the site answered at once with the
    /// person's refusal, nothing signed or sent, no sheet left.
    @Test func aSitesRequestSwipedAwayOnItsReviewIsLeftAndTheSiteAnsweredAtOnce() async throws {
        let world = try MsSimWorld()
        let (hub, record, safe, _) = try await Self.hubFollowing(world, salt: "0x53")
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        world.store["vela.multisigAccounts"] = MsSimWorld.text([record])
        hub.accounts.dispatch([
            "type": "start", "service_url": MsSimWorld.service, "accounts": world.localsJson, "networks": [100, 8453],
        ])
        await Wait.until({ hub.isMultisig(safe) }, orIdle: { hub.accounts.isIdle })
        try #require(hub.isMultisig(safe), "the multisig is not on this device")

        var answers: [[String: Any]] = []
        let params = MsSimWorld.text([["from": safe, "to": Self.bo, "value": "0x1", "data": "0x"]])
        hub.dappRequest(
            id: "site-1", address: safe, method: "eth_sendTransaction", paramsJson: params,
            origin: "https://app.example", chainId: 100, answer: { answers.append($0) }
        )
        await Wait.until({ hub.proposalsView.draft != nil || !answers.isEmpty }, orIdle: { hub.proposals.isIdle })
        #expect(hub.sheet == .dapp(id: "site-1"))
        #expect(hub.proposalsView.draft != nil, "the request is drawn for its review: \(answers)")
        #expect(answers.isEmpty)

        let swiped = ContinuousClock.now
        hub.sheetDismissed()
        #expect(hub.sheet == nil)
        await Wait.until({ !answers.isEmpty }, orIdle: { hub.proposals.isIdle })
        #expect(ContinuousClock.now - swiped < .seconds(5), "answered at once")
        #expect(answers.count == 1, "answered once: \(answers)")
        let error = answers.first?["error"] as? [String: Any]
        #expect((error?["code"] as? NSNumber)?.intValue == 4001, "the person's refusal: \(answers)")
        #expect(hub.proposalsView.draft == nil, "withdrawn")
        #expect(world.passkeys.isEmpty && world.submits.isEmpty, "nothing signed, nothing sent")
    }

    // MARK: member change

    /// The members machine on Settings (`MultisigHub.showMembers`): a 2-of-3
    /// deployed on Gnosis and Base, not on Optimism. Remove a member: the
    /// plan is 2 of 3 → 2 of 2 on each deployed network, none where it is
    /// not deployed — and proposed (`MultisigSettingsSheet.propose`), it is
    /// ONE proposal per deployed network, each a `removeOwner` of that
    /// member, one passkey each.
    @Test func removingAMemberProposesOnceOnEachDeployedNetworkAndNowhereElse() async throws {
        let world = try MsSimWorld()
        world.locals = try MsSimWorld.fixtures([0, 1])
        let (record, safe, owners) = try Self.twoOfThree("0x01", chains: [100, 8453, 10])
        world.deploy(100, safe, SimSafe(owners: owners, threshold: 2))
        world.deploy(8453, safe, SimSafe(owners: owners, threshold: 2))
        let members = MultisigMachine(core: MultisigMembersCore(), ports: world, name: "members")
        defer { members.dispose() }
        members.dispatch(["type": "show", "account": record, "chain_ids": [100, 8453, 10], "local": world.localsJson, "known": []])
        await Wait.until({ (members.json["rows"]?.array.count ?? 0) > 0 }, orIdle: { members.isIdle })
        #expect(members.json["rows"]?.array.count == 3, "every member, with Replace and Remove")

        let leaving = owners[2]
        members.dispatch(["type": "compose", "compose": ["kind": "remove", "owner": leaving]])
        await Wait.until({ members.json["plan"]?.isNull == false && members.json["plan"] != nil }, orIdle: { members.isIdle })
        let plan = try #require(members.json["plan"])
        #expect(plan["ready"]?.array.compactMap(\.int) == [100, 8453])
        #expect(plan["undeployed"]?.array.compactMap(\.int) == [10])
        for network in plan["networks"]?.array ?? [] {
            let verdict = network["verdict"]
            #expect(verdict?.kind == "ready", "refused on \(network["chain_id"]?.int ?? 0): \(String(describing: verdict))")
            #expect(verdict?["before"]?["threshold"]?.int == 2 && verdict?["before"]?["owners"]?.array.count == 3)
            #expect(verdict?["after"]?["threshold"]?.int == 2 && verdict?["after"]?["owners"]?.array.count == 2)
        }
        #expect(members.json["can_propose"]?.bool == true)

        // Proposed: each network's proposal through the proposals machine.
        let proposals = await Self.following(world, record, networks: [100, 8453, 10])
        defer { proposals.dispose() }
        members.dispatch(["type": "proposing"])
        var hashes: [Int: String] = [:]
        for network in plan["networks"]?.array ?? [] {
            guard let chainId = network["chain_id"]?.int, let proposal = network["proposal"]?.any as? [String: Any] else { continue }
            #expect(proposal["kind"] as? String == "member_change")
            let settled = await Self.propose(proposals, chainId: chainId, proposal, as: owners[0])
            #expect(settled?.kind == "done", "on \(chainId): \(String(describing: settled?.raw))")
            hashes[chainId] = settled?["safe_tx_hash"]?.string
        }
        #expect(Set(hashes.keys) == [100, 8453], "one proposal per deployed network")
        #expect(world.passkeys.count == 2, "one passkey per network")
        #expect(world.submits.count == 2)
        #expect(!world.submits.contains { MsSimWorld.int($0["chainId"]) == 10 }, "nothing where it is not deployed")
        for submit in world.submits {
            let tx = try #require(submit["safeTx"] as? [String: Any])
            #expect(Self.lower(tx["to"]) == safe.lowercased(), "a call of the multisig on itself")
            // removeOwner(prevOwner, owner, threshold): the member leaving, 2 still needed.
            let data = Self.lower(tx["data"])
            #expect(data.hasPrefix("0xf8dc5dd9"), "removeOwner: \(data.prefix(10))")
            #expect(data.contains(MsSimWorld.wordAddress(leaving)))
            #expect(data.hasSuffix(MsSimWorld.wordU(2)))
        }

        // Read back per network: the action the members machine composed.
        proposals.dispatch(["type": "refresh", "fresh": true])
        await Wait.until({ Set(proposals.proposalsView.items.map(\.chainId)) == [100, 8453] }, orIdle: { proposals.isIdle })
        for (chainId, hash) in hashes {
            let item = try #require(proposals.proposalsView.items.first { $0.safeTxHash == hash }, "nothing read on \(chainId)")
            #expect(item.chainId == chainId)
            #expect(item.action?.kind == "removeOwner")
            #expect(item.action?["owner"]?.string?.lowercased() == leaving.lowercased())
            #expect(item.action?["threshold"]?.string == "2")
        }
    }

    /// The approvals stepper: 2 of 3 → 3 of 3 on every deployed network (and
    /// no further than the members); proposed, each network gets its
    /// `changeThreshold(3)`. A network with a member change already waiting
    /// (the proposals machine's list, told to the members machine as the hub
    /// tells it) refuses the next one and says so; the others still propose.
    @Test func theStepperChangesTheRuleAndANetworkWithAChangeWaitingRefuses() async throws {
        let world = try MsSimWorld()
        world.locals = try MsSimWorld.fixtures([0, 1])
        let (record, safe, owners) = try Self.twoOfThree("0x02", chains: [100, 8453])
        world.deploy(100, safe, SimSafe(owners: owners, threshold: 2))
        world.deploy(8453, safe, SimSafe(owners: owners, threshold: 2))
        let members = MultisigMachine(core: MultisigMembersCore(), ports: world, name: "members")
        defer { members.dispose() }
        members.dispatch(["type": "show", "account": record, "chain_ids": [100, 8453], "local": world.localsJson, "known": []])
        await Wait.until({ (members.json["rows"]?.array.count ?? 0) > 0 }, orIdle: { members.isIdle })
        #expect(members.json["recovery"]?.kind == "replaceable", "\(String(describing: members.json["recovery"]))")

        members.dispatch(["type": "step", "by": 1])
        await Wait.until({ members.json["stepper"]?["value"]?.int == 3 }, orIdle: { members.isIdle })
        #expect(members.json["compose"]?.kind == "threshold")
        #expect(members.json["stepper"]?["value"]?.int == 3)
        #expect(members.json["stepper"]?["can_more"]?.bool == false, "no further than the members")
        // A step is the machine's answer to the event itself (no read).
        members.dispatch(["type": "step", "by": 1])
        #expect(members.json["change"]?.kind == "threshold" && members.json["change"]?["threshold"]?.int == 3, "past the members: still 3")
        let plan = try #require(members.json["plan"])
        #expect(plan["ready"]?.array.compactMap(\.int) == [100, 8453])
        for network in plan["networks"]?.array ?? [] {
            #expect(network["verdict"]?["after"]?["threshold"]?.int == 3)
        }

        // Proposed on Base only first: a change now waits there.
        let proposals = await Self.following(world, record, networks: [100, 8453])
        defer { proposals.dispose() }
        let base = try #require(plan["networks"]?.array.first { $0["chain_id"]?.int == 8453 }?["proposal"]?.any as? [String: Any])
        let settled = await Self.propose(proposals, chainId: 8453, base, as: owners[0])
        #expect(settled?.kind == "done", "\(String(describing: settled?.raw))")
        let data = Self.lower((world.submits.first?["safeTx"] as? [String: Any])?["data"])
        #expect(data == "0x694e80c3" + MsSimWorld.wordU(3), "changeThreshold(3): \(data)")
        await Wait.until({ proposals.proposalsView.items.contains { $0.chainId == 8453 } }, orIdle: { proposals.isIdle })
        let waiting = proposals.proposalsView.items.map { item -> [String: Any] in
            [
                "chain_id": item.chainId, "safe_tx_hash": item.safeTxHash, "nonce": item.nonce,
                "action": item.action?.any ?? NSNull(), "approved": item.approved,
                "threshold": item.threshold, "status": item.status,
            ]
        }
        #expect(waiting.count == 1)
        #expect((waiting.first?["action"] as? [String: Any])?["kind"] as? String == "changeThreshold")

        // Told to the members machine (`MultisigHub.proposalsMoved`): Base
        // refuses the next change, Gnosis still proposes it.
        members.dispatch(["type": "waiting", "proposals": waiting])
        members.dispatch(["type": "compose", "compose": ["kind": "remove", "owner": owners[2]]])
        await Wait.until({ members.json["plan"]?["ready"]?.array.compactMap(\.int) == [100] }, orIdle: { members.isIdle })
        let next = try #require(members.json["plan"])
        #expect(next["ready"]?.array.compactMap(\.int) == [100])
        let refused = next["networks"]?.array.first { $0["chain_id"]?.int == 8453 }
        #expect(refused?["verdict"]?.kind == "refused")
        #expect(refused?["verdict"]?["refusal"]?.kind == "changeWaiting")
        #expect(refused?["proposal"]?.isNull ?? true, "nothing to propose where it is refused")
    }
}
#endif
