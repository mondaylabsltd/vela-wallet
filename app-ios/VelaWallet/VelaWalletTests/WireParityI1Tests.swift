//
//  WireParityI1Tests.swift
//  VelaWalletTests
//
//  Native parity I1: every operation, result and field the core gained since
//  round 3 that iOS did not answer, answered — each branch on its own, and
//  each view the regenerated core writes decoded.
//
//  Hermetic: scripted ports, offline pools, a registry answered from memory,
//  and the REAL core where a view or an effect is the question.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - Fixtures

private func freshDefaults() -> UserDefaults {
    UserDefaults(suiteName: "vela.tests.i1.\(UUID().uuidString)")!
}

private func object(_ json: String) -> [String: Any] {
    (try? CoreJSON.object(json)) ?? [:]
}

/// A chain whose head and `getNonce` answer as scripted, remembering what it
/// was asked.
@MainActor
private final class NoncePort: RelayPort {
    var head: UInt64? = 0x1234
    var nonce: Any? = "0x" + String(repeating: "0", count: 63) + "7"
    private(set) var calls: [(method: String, params: [Any])] = []

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        calls.append((method, params))
        switch method {
        case "eth_blockNumber":
            guard let head else { return .failed(rateLimited: false) }
            return .ok("0x" + String(head, radix: 16))
        case "eth_call":
            guard let nonce else { return .failed(rateLimited: false) }
            return .ok(nonce)
        default:
            return .failed(rateLimited: false)
        }
    }

    func bundlerBase(chainId: Int) async -> String? { nil }
    func bestRpcUrl(chainId: Int) async -> String? { nil }
    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer { .failed }
}

/// A registry answering the group challenge and the register from memory,
/// and remembering the paths and bodies it was sent.
private final class PublishScript: @unchecked Sendable {
    private let lock = NSLock()
    private var seen: [(path: String, body: [String: Any])] = []
    let registerStatus: String

    init(registerStatus: String = "done") { self.registerStatus = registerStatus }

    var asked: [(path: String, body: [String: Any])] { lock.lock(); defer { lock.unlock() }; return seen }

    var transport: RegistryClient.Transport {
        { [self] request in
            guard let url = request.url,
                  let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)
            else { throw URLError(.badURL) }
            let body = request.httpBody.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] } ?? [:]
            lock.lock(); seen.append((url.path, body)); lock.unlock()
            let answer: String
            switch url.path {
            case "/api/challenge":
                answer = #"{"groupChallenge":{"challenge":"0x"# + String(repeating: "ab", count: 32)
                    + #""},"members":[]}"#
            case "/api/register":
                answer = #"{"status":"\#(registerStatus)","id":"t9"}"#
            default:
                answer = #"{"id":"t9","status":"done","txHash":"0xabc"}"#
            }
            return (Data(answer.utf8), response)
        }
    }
}

private let member: [String: Any] = [
    "credential_id": "c1", "public_key_hex": "04" + String(repeating: "11", count: 64),
    "attestation_hex": "", "authenticator_attachment": "platform", "transports": "internal",
    "proof": ["authenticatorData": "aa", "clientDataJSON": "bb", "signature": "cc"],
]

private func pending(members: [[String: Any]] = [member], page: Any = NSNull()) -> [String: Any] {
    [
        "metadata_hex": "",
        "members": members,
        "group_seed_hex": String(repeating: "22", count: 32),
        "group_public_key_hex": "04" + String(repeating: "33", count: 64),
        "page": page,
    ]
}

// MARK: - The tests

@MainActor
@Suite(.hangLimit)
struct WireParityI1Tests {

    // MARK: Clear — rpc_get_code (cs-13)

    @Test func clearAnswersGetCodeAsTheCodeProbe() async {
        let defaults = freshDefaults()
        let store = VelaStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: AccountStore(defaults: defaults), offline: true)
        let executor = ClearExecutor(dataBase: { "" }, pool: pool)
        #expect(ClearExecutor.operations.contains("rpc_get_code"))

        let answer = object(await executor.perform([
            "type": "rpc_get_code", "chain_id": 100, "address": "0xabc",
        ]))
        #expect(answer["type"] as? String == "rpc_answer")
        #expect(answer["probe"] as? String == "code")
        #expect(answer["to"] as? String == "0xabc")
        #expect(answer["chain_id"] as? Int == 100)
        // Nobody answered: unknown, never a revert.
        #expect(answer["result"] is NSNull)
        #expect(answer["rpc_error"] as? Bool == false)
    }

    @Test func clearsNeutralGetCodeIsUnknown() {
        let answer = object(ClearExecutor.neutralAnswer([
            "type": "rpc_get_code", "chain_id": 8453, "address": "0xdef",
        ]))
        #expect(answer["probe"] as? String == "code")
        #expect(answer["to"] as? String == "0xdef")
        #expect(answer["result"] is NSNull)
        #expect(answer["rpc_error"] as? Bool == false)
    }

    // MARK: Tracker — read_nonce (send-5)

    @Test func theNonceIsReadAtTheHeadItNames() async {
        let port = NoncePort()
        let executor = TrackerExecutor(
            store: VelaStore(defaults: freshDefaults()),
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        )
        #expect(TrackerExecutor.operations.contains("read_nonce"))
        let sender = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        let answer = object(await executor.perform([
            "type": "read_nonce", "chain_id": 100, "user_op_hash": "0xop",
            "entry_point": "0x0000000071727De22E5E9d8BAf0edAc6f37da032", "sender": sender,
        ]))
        #expect(answer["type"] as? String == "nonce_read")
        #expect(answer["user_op_hash"] as? String == "0xop")
        #expect(answer["head_block"] as? Int == 0x1234)
        #expect(answer["nonce"] as? String == "0x" + String(repeating: "0", count: 63) + "7")
        #expect(answer["now_ms"] is NSNumber)

        // `getNonce(sender, 0)` at that block, to the EntryPoint.
        let call = port.calls.last
        #expect(call?.method == "eth_call")
        let tx = call?.params.first as? [String: Any]
        #expect(tx?["to"] as? String == "0x0000000071727De22E5E9d8BAf0edAc6f37da032")
        #expect(tx?["data"] as? String == RelayClient.getNonceCalldata(sender: sender))
        #expect(call?.params.last as? String == "0x1234")
    }

    @Test func noHeadIsNoAnswerAtAll() async {
        let port = NoncePort()
        port.head = nil
        let executor = TrackerExecutor(
            store: VelaStore(defaults: freshDefaults()),
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        )
        let answer = object(await executor.perform([
            "type": "read_nonce", "chain_id": 100, "user_op_hash": "0xop",
            "entry_point": "0x0000000071727De22E5E9d8BAf0edAc6f37da032",
            "sender": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        ]))
        #expect(answer["nonce"] is NSNull)
        #expect(answer["head_block"] is NSNull)
        #expect(!port.calls.contains { $0.method == "eth_call" }, "no head, no nonce read")
    }

    @Test func theNonceCalldataIsGetNonceOfTheSenderAndKeyZero() {
        let data = RelayClient.getNonceCalldata(sender: "0x88cCA0EeDbF2C4426110bbFc998F048689266894")
        #expect(data == "0x35567e1a" + String(repeating: "0", count: 24)
            + "88cca0eedbf2c4426110bbfc998f048689266894" + String(repeating: "0", count: 64))
        #expect(RelayClient.getNonceCalldata(sender: "0x1234") == nil)
        let neutral = object(TrackerExecutor.neutralAnswer(["type": "read_nonce", "user_op_hash": "0xop"]))
        #expect(neutral["type"] as? String == "nonce_read")
        #expect(neutral["nonce"] is NSNull)
    }

    // MARK: Contacts — write_failed / store_unreadable (set-1, set-22)

    @Test func aDamagedShelfIsUnreadableNotEmpty() async {
        let defaults = freshDefaults()
        defaults.set("{not json", forKey: VelaStore.Key.contacts)
        let executor = ContactsExecutor(store: VelaStore(defaults: defaults))
        let answer = object(await executor.perform(["type": "read_store"]))
        #expect(answer["type"] as? String == "store_unreadable")

        // An object where a list belongs is as damaged.
        let other = freshDefaults()
        other.set(#"{"a":1}"#, forKey: VelaStore.Key.contactGroups)
        #expect(object(await ContactsExecutor(store: VelaStore(defaults: other))
            .perform(["type": "read_store"]))["type"] as? String == "store_unreadable")
    }

    @Test func anAbsentShelfIsAnEmptyBook() async {
        let executor = ContactsExecutor(store: VelaStore(defaults: freshDefaults()))
        let answer = object(await executor.perform(["type": "read_store"]))
        #expect(answer["type"] as? String == "store_loaded")
        #expect((answer["contacts"] as? [Any])?.isEmpty == true)
    }

    @Test func aWriteSaysWhetherItWasKept() async {
        let executor = ContactsExecutor(store: VelaStore(defaults: freshDefaults()))
        let kept = object(await executor.perform(["type": "write_groups", "groups": [[String: Any]]()]))
        #expect(kept["type"] as? String == "written")
        // The executor's own failure twin: what a refused or unanswerable
        // write is told as — never a silent `written`.
        for type in ["write_contacts", "write_dismissed", "write_groups"] {
            #expect(object(ContactsExecutor.neutralAnswer(["type": type]))["type"] as? String == "write_failed")
        }
        #expect(object(ContactsExecutor.neutralAnswer(["type": "read_store"]))["type"] as? String == "store_unreadable")
        #expect(object(ContactsExecutor.neutralAnswer(["type": "load_send_history"]))["type"] as? String == "history_failed")
    }

    @Test func theStoreReportsAWriteItCouldNotMake() {
        let store = VelaStore(defaults: freshDefaults())
        #expect(store.writeList("vela.test", [["ok": 1]]))
        // NaN is not JSON: nothing is written, and the caller is told.
        #expect(!store.writeList("vela.test", [["bad": Double.nan]]))
        #expect(store.readList("vela.test").first?["ok"] as? Int == 1, "yesterday's good bytes are kept")
    }

    // MARK: Session — publish_pending_upload (ob-2)

    @Test func aPendingPublishIsReplayedWithItsOwnProofsAndLands() async {
        let registry = PublishScript()
        let executor = SessionExecutor(
            store: AccountStore(defaults: freshDefaults()), registryTransport: registry.transport
        )
        #expect(SessionExecutor.operations.contains("publish_pending_upload"))
        let answer = object(await executor.perform(["type": "publish_pending_upload", "publish": pending()]))
        #expect(answer["type"] as? String == "registry_landed", "\(answer)")
        let paths = registry.asked.map(\.path)
        #expect(paths == ["/api/challenge", "/api/register"])
        let register = registry.asked.last?.body ?? [:]
        #expect(register["rpId"] as? String == PasskeyExecutor.relyingParty, "no page: the wallet's own party")
        let sent = (register["members"] as? [[String: Any]])?.first
        #expect((sent?["proof"] as? [String: Any])?["signature"] as? String == "cc", "the creation proof, replayed")
    }

    @Test func anAcceptedReplayWaitsForTheLanding() async {
        let registry = PublishScript(registerStatus: "pending")
        let executor = SessionExecutor(
            store: AccountStore(defaults: freshDefaults()), registryTransport: registry.transport
        )
        let answer = object(await executor.perform(["type": "publish_pending_upload", "publish": pending()]))
        #expect(answer["type"] as? String == "registry_landed")
        #expect(registry.asked.map(\.path).last == "/api/task/t9", "never answered at acceptance")
    }

    @Test func aMemberWithoutItsProofIsNotReplayed() async {
        let registry = PublishScript()
        let executor = SessionExecutor(
            store: AccountStore(defaults: freshDefaults()), registryTransport: registry.transport
        )
        var bare = member
        bare.removeValue(forKey: "proof")
        let answer = object(await executor.perform([
            "type": "publish_pending_upload", "publish": pending(members: [bare]),
        ]))
        #expect(answer["type"] as? String == "registry_landing_unconfirmed")
        #expect(!(answer["message"] as? String ?? "").isEmpty)
        #expect(registry.asked.isEmpty, "nothing is sent for a publish that would need a prompt")
    }

    /// The session asks to sync again while a wallet is owed and retrying.
    @Test func theSessionViewCarriesTheUnsyncedWallets() throws {
        let view = try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": "0xa", "active_index": 0,
            "accounts": [], "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts_unreadable": false,
            "unsynced": [["name": "Ann", "address": "0xa", "retrying": true]],
        ])
        #expect(view.unsynced?.first?.name == "Ann")
        #expect(view.retryingUnsynced)
        let older = try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": false, "address": "", "active_index": 0,
            "accounts": [], "allowed_route": "onboarding", "sign_out": NSNull(),
        ])
        #expect(!older.retryingUnsynced, "an older core's view: nothing owed")
        #expect(SessionController.syncRetryMs.first == 15_000)
    }

    /// The REAL session machine takes `sync_pending` (no fault, an answer).
    @Test func theRealSessionMachineTakesSyncPending() throws {
        let core = SessionCore()
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "sync_pending"])))
        #expect(result["view"] is [String: Any])
        _ = try CoreJSON.decode(SessionView.self, from: result["view"] as? [String: Any] ?? [:])
    }

    // MARK: Send — propose_safe_tx, recipient risk (r4a-18, round 3)

    @Test func aProposalIsNeverPretendedOnThisShell() {
        let answer = object(SendExecutor.proposeFailed)
        #expect(answer["type"] as? String == "propose_failed")
        #expect(answer["cancelled"] as? Bool == false)
        #expect(answer["reason"] is NSNull)
        #expect(SendExecutor.operations.contains("propose_safe_tx"))
        #expect(object(SendExecutor.neutralAnswer(["type": "propose_safe_tx"]))["type"] as? String == "propose_failed")
    }

    private let mine = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let stranger = "0x1111111111111111111111111111111111111111"

    @Test func anOwnAccountIsNeverAFirstTransfer() {
        let risk = SendExecutor.recipientRisk(
            address: mine.lowercased(), isContract: false, records: [],
            accounts: [["address": mine, "name": " Savings "]], contacts: []
        )
        #expect(risk["own"] as? Bool == true)
        #expect(risk["known_name"] as? String == "Savings")
        // The history says never; `own` is what the core reads first.
        #expect(risk["first_time"] as? Bool == true)
        #expect(risk["history_unread"] as? Bool == false)
    }

    @Test func aContactIsNamedByThePersonsOwnWordOnly() {
        let named = SendExecutor.recipientRisk(
            address: stranger, isContract: nil, records: [], accounts: [],
            contacts: [["address": stranger, "name": "Bob", "resolvedName": "bob.eth"]]
        )
        #expect(named["known_name"] as? String == "Bob")
        #expect(named["own"] as? Bool == false)
        let resolvedOnly = SendExecutor.recipientRisk(
            address: stranger, isContract: nil, records: [], accounts: [],
            contacts: [["address": stranger, "resolvedName": "bob.eth"]]
        )
        #expect(resolvedOnly["known_name"] is NSNull, "a resolved name is a service's word")
    }

    /// vis-24: an account's name outranks a contact's, and blank is none.
    @Test func theKnownNameIsTheAccountsThenTheContacts() {
        let accounts: [[String: Any]] = [["address": mine, "name": "  "]]
        let contacts: [[String: Any]] = [["address": mine.lowercased(), "name": "Me, saved"]]
        #expect(SendExecutor.knownName(mine, accounts: accounts, contacts: contacts) == "Me, saved")
        #expect(SendExecutor.knownName(mine, accounts: [["address": mine, "name": "Savings"]], contacts: contacts) == "Savings")
        #expect(SendExecutor.knownName(stranger, accounts: accounts, contacts: contacts) == nil)
    }

    @Test func anUnreadHistoryIsNeverNeverSentHere() {
        let risk = SendExecutor.recipientRisk(
            address: stranger, isContract: true, records: nil, accounts: [], contacts: []
        )
        #expect(risk["first_time"] is NSNull)
        #expect(risk["history_unread"] as? Bool == true)
        #expect(risk["is_contract"] as? Bool == true)
        let sent = SendExecutor.recipientRisk(
            address: stranger, isContract: nil, records: [["to": stranger, "type": "send"]],
            accounts: [], contacts: []
        )
        #expect(sent["first_time"] as? Bool == false)
        let notAnAddress = SendExecutor.recipientRisk(
            address: "bob", isContract: nil, records: nil, accounts: [], contacts: []
        )
        #expect(notAnAddress["first_time"] as? Bool == false)
        #expect(notAnAddress["history_unread"] as? Bool == false)
    }

    @Test func theRiskDecodesWithItsNewFields() throws {
        let risk = try CoreJSON.decode(SendRecipientRiskWire.self, from: [
            "is_contract": false, "first_time": NSNull(), "history_unread": true,
            "known_name": "Savings", "own": true,
        ])
        #expect(risk.own == true)
        #expect(risk.knownName == "Savings")
        #expect(risk.historyUnread == true)
        let older = try CoreJSON.decode(SendRecipientRiskWire.self, from: ["is_contract": NSNull(), "first_time": true])
        #expect(older.own == nil)
    }

    /// The real send machine reads our risk answer — `own` included — with no fault.
    @Test func theRealSendMachineTakesTheRiskAnswerAndTheRetry() throws {
        let core = SendCore()
        let view = try CoreJSON.decode(SendViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(view.riskHistoryUnread != true)
        let retry = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "retry_risk_check"])))
        #expect(retry["view"] is [String: Any], "`retry_risk_check` is an event the core knows")
    }

    @Test func theSendViewDecodesRoundThreeToSixFields() throws {
        var raw = try CoreJSON.object(SendCore().view())
        raw["submit_slow"] = ["seconds": 9, "busy": true]
        raw["max_moved"] = ["from": "1.2", "to": "1.1"]
        raw["passkey_note"] = "timed_out"
        raw["recipient_warning"] = "own_address"
        raw["risk_history_unread"] = true
        raw["queued_behind_tab"] = true
        raw["multisig"] = ["threshold": 2, "owners": 3]
        raw["proposed"] = ["chain_id": 100, "safe_tx_hash": "0xs"]
        let view = try CoreJSON.decode(SendViewWire.self, from: raw)
        #expect(view.submitSlow == SendSubmitSlowWire(seconds: 9, busy: true))
        #expect(view.maxMoved?.to == "1.1")
        #expect(view.passkeyNote == "timed_out")
        #expect(view.recipientWarning == "own_address")
        #expect(view.riskHistoryUnread == true)
        #expect(view.queuedBehindTab == true)
        #expect(view.multisig?.threshold == 2)
        #expect(view.proposed?.safeTxHash == "0xs")
    }

    // MARK: Rpc — pool_config own_only; Balance — own-node / fallback

    @Test func onlyUseMyNodeIsReadFromTheChainsConfig() {
        let store = VelaStore(defaults: freshDefaults())
        store.writeList(VelaStore.Key.networkConfig, [
            ["chainId": 1, "rpcURL": "https://mine.test", "onlyMine": true],
            ["chainId": 10, "rpcURL": "https://other.test"],
        ])
        #expect(RpcEndpoints.ownNodeOnly(chainId: 1, store: store))
        #expect(!RpcEndpoints.ownNodeOnly(chainId: 10, store: store))
        #expect(!RpcEndpoints.ownNodeOnly(chainId: 137, store: store))
    }

    @Test func thePoolConfigCarriesOwnOnly() async {
        let defaults = freshDefaults()
        let store = VelaStore(defaults: defaults)
        store.writeList(VelaStore.Key.networkConfig, [["chainId": 100, "rpcURL": "https://mine.test", "onlyMine": true]])
        let pool = RpcPool(store: store, accounts: AccountStore(defaults: defaults), offline: true)
        let answer = object(await pool.perform(["type": "load_pool_config", "chain_id": 100]))
        #expect(answer["type"] as? String == "pool_config")
        #expect(answer["own_only"] as? Bool == true)
        let sources = (answer["rpc_endpoints"] as? [[String: Any]] ?? []).compactMap { $0["source"] as? String }
        #expect(sources.first == "user")
    }

    @Test func thePoolViewCarriesTheOwnNodeLists() throws {
        var raw = try CoreJSON.object(RpcPoolCore().view())
        raw["fallback_chains"] = [1]
        raw["own_only_chains"] = [1, 10]
        raw["own_node_unreached_chains"] = [10]
        let view = try CoreJSON.decode(RpcPoolViewWire.self, from: raw)
        #expect(view.fallbackChains == [1])
        #expect(view.ownOnlyChains == [1, 10])
        #expect(view.ownNodeUnreachedChains == [10])
    }

    @Test func ownNodeChainsAreTheFailedOnesTheOwnNodeAlonesServes() {
        let view = SettingsLive.OwnNodes(fallback: [1], unreached: [10, 8453]).viewJson
        #expect(BalanceExecutor.ownNodeChains(failed: [1, 10, 137], poolViewJson: view) == [10])
        #expect(BalanceExecutor.ownNodeChains(failed: [], poolViewJson: view).isEmpty)
        // No view yet: nothing is said about anybody's node.
        #expect(BalanceExecutor.ownNodeChains(failed: [10], poolViewJson: nil).isEmpty)
    }

    // MARK: Net — the regenerated view

    @Test func theNetViewDecodesWithoutProvidersAndWithHiddenNetworks() throws {
        var raw = try CoreJSON.object(NetworkAdminCore().view())
        raw.removeValue(forKey: "providers")
        raw["hidden_vela_chain_ids"] = [988]
        let view = try CoreJSON.decode(NetViewWire.self, from: raw)
        #expect(view.hiddenVelaChainIds == [988])
        let row = try CoreJSON.decode(NetNetworkRowWire.self, from: [
            "id": "1", "chain_id": 1, "display_name": "Ethereum", "native_symbol": "ETH",
            "is_custom": false, "rpc_url": "", "explorer_url": "", "bundler_url": "",
            "rpc_save_deferred": false, "is_testnet": false,
            "optional": true, "own_rpc": true, "only_mine": true,
        ])
        #expect(row.optIn && row.ownRpc && row.onlyMine)
    }

    /// The real networks machine, through its whole first load with this
    /// executor's answers: every effect it asks for is one this build
    /// handles, and the view it lands on decodes — `shown_networks` and
    /// `multisig_service` included.
    @Test func theRealNetworksMachineLoadsOnThisExecutorsAnswers() async throws {
        let defaults = freshDefaults()
        let store = VelaStore(defaults: defaults)
        store.writeObject(VelaStore.Key.rpcProviders, ["alchemy": "k"])
        let executor = NetworkAdminExecutor(store: store, accounts: AccountStore(defaults: defaults))
        let core = NetworkAdminCore()
        var pending = (try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "started"]))))["effects"]
            as? [[String: Any]] ?? []
        var asked: [String] = []
        var rounds = 0
        while let effect = pending.first, rounds < 40 {
            pending.removeFirst()
            rounds += 1
            let operation = effect["operation"] as? [String: Any] ?? [:]
            let tag = operation["type"] as? String ?? ""
            asked.append(tag)
            #expect(NetworkAdminExecutor.operations.contains(tag), "the core asks for `\(tag)`")
            // Only the storage answers: a probe here would reach the network.
            guard ["read_store", "write_shown_networks", "retire_provider_keys", "write_network_configs",
                   "write_custom_networks", "invalidate_pools"].contains(tag)
            else { continue }
            let answer = await executor.perform(operation)
            let id = (effect["id"] as? NSNumber)?.uint64Value ?? 0
            let next = try CoreJSON.object(try core.resolveEffect(effectId: id, resultJson: answer))
            pending += next["effects"] as? [[String: Any]] ?? []
        }
        #expect(asked.first == "read_store")
        let view = try CoreJSON.decode(NetViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(view.loaded)
        // The provider key was retired into an override and the shelf emptied.
        if asked.contains("retire_provider_keys") {
            #expect(defaults.object(forKey: VelaStore.Key.rpcProviders) == nil)
        }
    }

    // MARK: The transport's one HTTPS exception (the core's `is_localhost_http`)

    @Test func aNodeOnThisComputerIsTheOnlyPlainHttpTheTransportSends() {
        for url in ["http://localhost", "http://localhost:8545", "http://127.0.0.1:9045/",
                    "http://[::1]:8545/rpc", "HTTP://LOCALHOST:1"] {
            #expect(CoreHTTP.isLocalhostHttp(url), "\(url)")
        }
        for url in ["http://127.0.0.1.evil.com", "http://localhost.evil.com:80", "http://localhost:",
                    "http://localhostx", "https://localhost", "http://10.0.0.2:8545", "ws://localhost:1"] {
            #expect(!CoreHTTP.isLocalhostHttp(url), "\(url)")
        }
    }

    // MARK: F0.5 — the debug launch inputs

    @Test func serviceEndpointsAreMergedIntoTheStoredBlob() throws {
        let defaults = freshDefaults()
        defaults.set(#"{"passkeyIndexURL":"https://idx.test","fiatRatesURL":"https://fx.test"}"#,
                     forKey: VelaStore.Key.serviceEndpoints)
        try DevLaunchInputs.apply(environment: [
            "VELA_SERVICE_ENDPOINTS":
                #"{"bundlerServiceURL":"http://127.0.0.1:9090","genesis_service_url":"http://127.0.0.1:9087","fiatRatesURL":null}"#,
        ], defaults: defaults)
        let stored = try JSONSerialization.jsonObject(
            with: Data((defaults.string(forKey: VelaStore.Key.serviceEndpoints) ?? "").utf8)
        ) as? [String: Any]
        #expect(stored?["bundlerServiceURL"] as? String == "http://127.0.0.1:9090")
        #expect(stored?["genesisServiceURL"] as? String == "http://127.0.0.1:9087", "a wire name is stored by its stored name")
        #expect(stored?["passkeyIndexURL"] as? String == "https://idx.test", "what it does not name is kept")
        #expect(stored?["fiatRatesURL"] == nil, "null removes")
    }

    @Test func rpcOverridesAreUpsertedAsThePersonsOwnNode() throws {
        let defaults = freshDefaults()
        let store = VelaStore(defaults: defaults)
        store.writeList(VelaStore.Key.networkConfig, [
            ["chainId": 100, "rpcURL": "https://old.test", "explorerURL": "https://scan.test", "bundlerURL": ""],
            ["chainId": 137, "rpcURL": "https://poly.test", "explorerURL": "", "bundlerURL": ""],
        ])
        try DevLaunchInputs.apply(environment: [
            "VELA_RPC_OVERRIDES":
                #"{"100":"http://127.0.0.1:9045","8453":{"rpcURL":"http://127.0.0.1:9046","onlyMine":true},"137":null}"#,
        ], defaults: defaults)
        let configs = store.readList(VelaStore.Key.networkConfig)
        let gnosis = configs.first { ($0["chainId"] as? Int) == 100 }
        #expect(gnosis?["rpcURL"] as? String == "http://127.0.0.1:9045")
        #expect(gnosis?["explorerURL"] as? String == "https://scan.test", "the rest of the config is kept")
        let base = configs.first { ($0["chainId"] as? Int) == 8453 }
        #expect(base?["onlyMine"] as? Bool == true)
        #expect(!configs.contains { ($0["chainId"] as? Int) == 137 }, "null removes the override")
        #expect(RpcEndpoints.ownNodeOnly(chainId: 8453, store: store))
    }

    @Test func thePeerConnectRelayIsKeptWhereTheWebKeepsIt() throws {
        let defaults = freshDefaults()
        try DevLaunchInputs.apply(environment: ["VELA_PEERCONNECT_RELAY": "ws://127.0.0.1:9092"], defaults: defaults)
        #expect(defaults.string(forKey: "vela.peerconnectRelay") == "ws://127.0.0.1:9092")
    }

    @Test func malformedInputsAreRefusedNotGuessed() {
        let defaults = freshDefaults()
        #expect(throws: DevLaunchInputs.InputError.self) {
            try DevLaunchInputs.apply(environment: ["VELA_SERVICE_ENDPOINTS": "not json"], defaults: defaults)
        }
        #expect(throws: DevLaunchInputs.InputError.self) {
            try DevLaunchInputs.apply(environment: ["VELA_SERVICE_ENDPOINTS": #"{"madeUpURL":"https://x"}"#], defaults: defaults)
        }
        #expect(throws: DevLaunchInputs.InputError.self) {
            try DevLaunchInputs.apply(environment: ["VELA_RPC_OVERRIDES": #"{"gnosis":"http://x"}"#], defaults: defaults)
        }
        #expect(throws: DevLaunchInputs.InputError.self) {
            try DevLaunchInputs.apply(environment: ["VELA_PEERCONNECT_RELAY": "nowhere"], defaults: defaults)
        }
        // Nothing set: nothing written.
        let untouched = freshDefaults()
        #expect(throws: Never.self) { try DevLaunchInputs.apply(environment: [:], defaults: untouched) }
        #expect(untouched.string(forKey: VelaStore.Key.serviceEndpoints) == nil)
    }
}
