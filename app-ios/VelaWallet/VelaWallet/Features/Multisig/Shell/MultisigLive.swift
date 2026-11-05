//
//  MultisigLive.swift
//  VelaWallet
//
//  The app's own pipelines behind the multisig shell (`MultisigPorts`) — the
//  web's `shell/live.ts`: the RPC pool, the key-value store, the passkey (in
//  Vela, or on the member's trusted signing page), the high-risk question,
//  the send and fee pipelines, the signing sheet's simulation, recipient
//  facts, the relay's word on a user operation, the PeerConnect rooms.
//
//  Each answers in the vocabulary and never decides: a failure is reported as
//  what it was (`MultisigShellFailure`, carried as the `detail` the core
//  hands back in its views) so a sheet can say it in the corpus's words.
//

import Foundation
import Security
import VelaCore

/// Why a passkey or a send did not happen, as the shell saw it — JSON in a
/// result's `detail`, the web's `ShellFailure` (`shell/failure.ts`), so the
/// three shells hand the core, and their sheets, the same words.
enum MultisigShellFailure: Equatable {
    case venueBlocked(VenueBlockWire)
    case pageDeclined
    case pageSignFailed(page: String, message: String?)
    case passkey(failure: String, timedOut: Bool, message: String?)
    case notSent
    case cannotPay(need: String, have: String)
    case unknown(String)

    var object: [String: Any] {
        switch self {
        case .venueBlocked(let block): ["kind": "venue_blocked", "block": block.wire]
        case .pageDeclined: ["kind": "page_declined"]
        case .pageSignFailed(let page, let message):
            ["kind": "page_sign_failed", "page": page, "message": message ?? NSNull()]
        case .passkey(let failure, let timedOut, let message):
            ["kind": "passkey", "failure": failure, "timedOut": timedOut, "message": message ?? NSNull()]
        case .notSent: ["kind": "not_sent"]
        case .cannotPay(let need, let have): ["kind": "cannot_pay", "need": need, "have": have]
        case .unknown(let message): ["kind": "unknown", "message": message]
        }
    }

    var detail: String { CoreJSON.string(object) }

    /// A `detail` the shell wrote, or `nil` for one it did not (the core's
    /// own words) — the web's `readShellFailure`.
    static func read(_ detail: String?) -> [String: Any]? {
        guard let detail, detail.hasPrefix("{"),
              let object = try? CoreJSON.object(detail),
              object["kind"] is String
        else { return nil }
        return object
    }
}

/// What the app hands the multisig shell. Built once, in `AppGraph`.
@MainActor
final class MultisigLive: MultisigPorts {
    struct Deps {
        let pool: RpcPool
        let relay: RelayClient
        let spine: UserOpSpine
        let accounts: AccountStore
        let accountPort: SendAccountPort
        let shelf: VelaStore
        let fees: FeeStore
        let trustedSigner: TrustedSignerPort & AnyObject
    }

    private let deps: Deps
    let peer: MultisigPeerTransport
    /// Where a notice goes (the hub's router), set once the hub exists.
    var onNotice: (@MainActor (_ notice: [String: Any]) async -> Void)?
    /// The high-risk question (§3.5): the hub's sheet. `nil`: nobody can be
    /// asked, so the answer is "do not sign".
    var onAskRisk: (@MainActor (_ op: [String: Any]) async -> Bool)?
    /// A first-party transaction through the signing sheet (a multisig's
    /// record saved to Ethereum): the root view's. `nil`: answered failed.
    var onFirstParty: (@MainActor (_ op: [String: Any]) async -> [String: Any])?
    /// The approvals pages answered, by member, chain and hash: the same
    /// bytes approved twice are one approval (the web's `answered`).
    private var pageAnswers: [String: String] = [:]

    init(deps: Deps, peer: MultisigPeerTransport) {
        self.deps = deps
        self.peer = peer
    }

    // MARK: - HTTP

    func http(method: String, url: String, body: String?, timeoutMs: Int) async throws
        -> (status: Int, retryAfter: String?, body: String) {
        guard let target = URL(string: url) else {
            throw MultisigWalletRefusal(code: nil, message: "not a URL: \(url)")
        }
        // Real-device S3: the RPC's rule, not a looser one — https, or plain
        // http to this device only. A LAN address over plain http is refused
        // here as it is for a node (answered `http_unreachable`).
        if let refusal = Self.transportRefusal(url) { throw refusal }
        var request = URLRequest(url: target, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData)
        request.httpMethod = method
        request.timeoutInterval = max(1, Double(timeoutMs) / 1000)
        if let body {
            request.httpBody = Data(body.utf8)
            request.setValue("application/json", forHTTPHeaderField: "content-type")
        }
        let (data, response) = try await URLSession.shared.data(for: request)
        let http = response as? HTTPURLResponse
        return (
            status: http?.statusCode ?? 0,
            retryAfter: http?.value(forHTTPHeaderField: "retry-after"),
            body: String(decoding: data, as: UTF8.self)
        )
    }

    /// The app's one transport rule (`CoreHTTP.allowsTransport`), as the
    /// refusal the multisig service's `http` answers with; `nil` when the
    /// URL may be sent to.
    nonisolated static func transportRefusal(_ url: String) -> MultisigWalletRefusal? {
        CoreHTTP.allowsTransport(url) ? nil : MultisigWalletRefusal(code: nil, message: "not https: \(url)")
    }

    // MARK: - RPC

    func rpc(chainId: Int, method: String, params: Any) async -> MultisigRpcReply {
        let list = params as? [Any] ?? [params]
        let routed = await deps.pool.callDetailed(chainId: chainId, method: method, params: list)
        switch routed.outcome {
        case .ok(let value): return .result(value)
        case .rpcError(let code, let message):
            // r8-4: the error's `data` as the node sent it (the held error).
            return .error(code: code, message: message, data: Self.errorData(routed.heldErrorJson))
        case .failed(let rateLimited):
            return .unanswered(rateLimited ? "every node of \(chainId) is rate limiting" : "no node of \(chainId) answered")
        case .rangeCap: return .unanswered("the node capped the range")
        }
    }

    /// A JSON-RPC error object's `data`, as the node sent it; `nil` when it
    /// carried none.
    static func errorData(_ errorJson: String?) -> Any? {
        guard let errorJson,
              let object = try? JSONSerialization.jsonObject(with: Data(errorJson.utf8)) as? [String: Any],
              let data = object["data"], !(data is NSNull)
        else { return nil }
        return data
    }

    // MARK: - Store

    func storeRead(_ key: String) -> String?? {
        .some(deps.shelf.readString(key))
    }

    func storeWrite(_ key: String, _ value: String?) -> Bool {
        deps.shelf.writeString(key, value)
        return true
    }

    func nowMs() -> Double { Date().timeIntervalSince1970 * 1000 }

    func random(_ bytes: Int) -> Data {
        var data = Data(count: bytes)
        let status = data.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, bytes, $0.baseAddress!) }
        return status == errSecSuccess ? data : Data((0..<bytes).map { _ in UInt8.random(in: 0...255) })
    }

    // MARK: - Passkey

    /// `owner` signs `challenge`: in Vela, or on its trusted page for an
    /// approval routed there (the web's `passkey`).
    func passkey(_ op: [String: Any]) async -> [String: Any] {
        let owner = op["owner"] as? String ?? ""
        let chainId = MultisigExecutor.int(op["chain_id"]) ?? 0
        let purpose = op["purpose"] as? [String: Any] ?? [:]
        guard let record = await deps.accounts.loadAccounts().first(where: {
            ($0["address"] as? String)?.caseInsensitiveCompare(owner) == .orderedSame
        }), let accountJson = await deps.accountPort.accountJson(of: owner) else {
            return Self.passkeyFailed(.unknown("not an account on this device"))
        }
        let keys = await deps.accountPort.keys(of: owner)
        if purpose["kind"] as? String == "approve" {
            switch SigningRouteWire.of(accountJson: accountJson, method: "vela_multisigApprove") {
            case .blocked(let block):
                return Self.passkeyFailed(.venueBlocked(block))
            case .page(let page):
                return await approveOnPage(
                    owner: owner, name: record["name"] as? String, chainId: chainId,
                    purpose: purpose, keys: keys, accountJson: accountJson, page: page
                )
            case .inVela, nil:
                break
            }
        } else if case .page = SigningRouteWire.of(accountJson: accountJson, method: "vela_multisigCreate") {
            // A wallet on its own domain proves membership on its page — a
            // ceremony this app runs only for onboarding today. Said, never
            // signed behind its back.
            return Self.passkeyFailed(.pageSignFailed(page: "not_opened", message: nil))
        }
        guard let challenge = Self.bytes(op["challenge"] as? String) else {
            return Self.passkeyFailed(.unknown("the challenge does not read"))
        }
        do {
            let assertion = try await deps.spine.assertInVela(account: owner, challenge: challenge)
            return [
                "type": "asserted",
                "credential_id": assertion.credentialIdHex,
                "authenticator_data": Self.hex0x(assertion.authenticatorDataHex),
                "client_data_json": Self.hex0x(assertion.clientDataJsonHex),
                "signature": Self.hex0x(assertion.signatureDerHex),
            ]
        } catch let refused as UserOpSpine.Refused {
            return Self.passkeyAnswer(refused.failure)
        } catch {
            // Real-device S3: the core's words for the platform's error.
            return Self.passkeyFailed(Self.passkeyError(error))
        }
    }

    /// The member's approval on its trusted page: the core builds the request
    /// and accepts the answer (`trustedSignerApprovalRequest` / `…Accept`).
    private func approveOnPage(
        owner: String, name: String?, chainId: Int, purpose: [String: Any],
        keys: [WalletKeyRecord], accountJson: String, page: String
    ) async -> [String: Any] {
        let hash = purpose["safe_tx_hash"] as? String ?? ""
        let answerKey = "\(owner.lowercased()):\(chainId):\(hash.lowercased())"
        if let kept = pageAnswers[answerKey], let approval = try? JSONSerialization.jsonObject(with: Data(kept.utf8)) {
            return ["type": "page_approved", "approval": approval]
        }
        let plan = SigningPlanWire.of(accountJson: accountJson)
        let chain = ChainCatalog.meta(chainId)
        let txJson = (purpose["tx"]).flatMap { try? JSONSerialization.data(withJSONObject: $0) }
            .map { String(decoding: $0, as: UTF8.self) } ?? "{}"
        let input = TrustedSignerApprovalInput(
            chainId: UInt64(chainId),
            chainName: chain?.displayName,
            nativeSymbol: chain?.nativeSymbol,
            multisig: purpose["multisig"] as? String ?? "",
            multisigName: purpose["name"] as? String,
            txJson: txJson,
            member: owner,
            memberName: name,
            credentialIdsHex: keys.map(\.credentialId),
            keyRouteJson: plan?.key?.json,
            // r9-10: the review asks for "… anyway" — the page says
            // "Approve anyway" in the danger style, as the app does.
            anyway: purpose["anyway"] as? Bool ?? false,
            // r10: the members now and the rule, so a member change reads
            // before → after on the page.
            owners: purpose["owners"] as? [String],
            threshold: (purpose["threshold"] as? NSNumber).map { UInt64(truncating: $0) }
        )
        guard let request = try? trustedSignerApprovalRequest(input: input) else {
            return Self.passkeyFailed(.pageSignFailed(page: "not_opened", message: nil))
        }
        let ending = await deps.trustedSigner.approveMultisig(
            requestJson: request, chainId: UInt64(chainId), member: owner, safeTxHash: hash,
            keys: keys, page: page, keyLabel: plan?.keyLabel
        )
        switch ending {
        case .approval(.accepted(let approvalJson)):
            guard let approval = try? JSONSerialization.jsonObject(with: Data(approvalJson.utf8)) else {
                return Self.passkeyFailed(.pageSignFailed(page: "mismatch", message: nil))
            }
            pageAnswers[answerKey] = approvalJson
            return ["type": "page_approved", "approval": approval]
        case let other:
            let notice = TrustedSignerNotice(ending: other) ?? .mismatch
            if notice == .closed { return ["type": "passkey_cancelled"] }
            return Self.passkeyFailed(.pageSignFailed(page: notice.pageFailure ?? "refused", message: nil))
        }
    }

    /// A passkey ceremony's error as the core reads it (`passkey_error_words`):
    /// its failure, and the corpus key the sheet says — never the system's
    /// "domain code: description".
    static func passkeyError(_ error: Error) -> MultisigShellFailure {
        let words = PasskeyErrorFacts.of(error).words
        return .passkey(failure: words.failure, timedOut: words.kind == .timedOut, message: words.key)
    }

    static func passkeyFailed(_ failure: MultisigShellFailure) -> [String: Any] {
        ["type": "passkey_failed", "detail": failure.detail]
    }

    /// A spine refusal, as the passkey operation answers it.
    static func passkeyAnswer(_ failure: UserOpSpine.Failure) -> [String: Any] {
        switch failure {
        case .passkeyCancelled: return ["type": "passkey_cancelled"]
        default: return passkeyFailed(shellFailure(failure))
        }
    }

    /// A spine refusal in the shell's failure words.
    static func shellFailure(_ failure: UserOpSpine.Failure) -> MultisigShellFailure {
        switch failure {
        case .passkeyCancelled: return .passkey(failure: "cancelled", timedOut: false, message: nil)
        case .passkeyTimedOut: return .passkey(failure: "cancelled", timedOut: true, message: nil)
        case .signer(let kind, let message): return .passkey(failure: kind.rawValue, timedOut: false, message: message)
        case .trustedSigner(let notice):
            return notice == .closed ? .pageDeclined : .pageSignFailed(page: notice.pageFailure ?? "refused", message: nil)
        case .venueBlocked(let block): return .venueBlocked(block)
        case .notSent: return .notSent
        case .relayerUnavailable: return .unknown("The relay is unavailable right now.")
        case .bundlerUnderfunded: return .unknown("The relay cannot pay for transactions right now.")
        case .other(let message): return .unknown(message ?? "")
        case .askerGone: return .unknown("cancelled")
        case .rejected(let message): return .unknown(message ?? "The network refused this transaction.")
        case .previousPending: return .unknown("Waiting for the last transaction of this account on this network.")
        case .nonceUsed: return .unknown("Another transaction of this account went first.")
        }
    }

    // MARK: - Risk

    func askRisk(_ op: [String: Any]) async -> Bool {
        guard let onAskRisk else { return false }
        return await onAskRisk(op)
    }

    // MARK: - Send

    /// `from` sends `calls` as one user operation, by what it is for. An
    /// execution: the multisig path's own — ONE signature, the executor's own
    /// operation, its page handed what it runs; the coin checked before the
    /// passkey. A record (Ethereum backup): the signing sheet.
    func sendCalls(_ op: [String: Any]) async -> [String: Any] {
        let purpose = op["purpose"] as? [String: Any]
        let isRecord = purpose.map { $0["kind"] as? String == "record" } ?? (op["fee"] == nil || op["fee"] is NSNull)
        if isRecord {
            guard let onFirstParty else {
                return ["type": "calls_failed", "detail": MultisigShellFailure.unknown("nothing to sign it on").detail, "maybe_sent": false]
            }
            return await onFirstParty(op)
        }
        let from = op["from"] as? String ?? ""
        let chainId = MultisigExecutor.int(op["chain_id"]) ?? 0
        guard await deps.accountPort.accountJson(of: from) != nil else {
            return [
                "type": "calls_failed",
                "detail": MultisigShellFailure.unknown("the executor is not on this device").detail,
                "maybe_sent": false,
            ]
        }
        let calls = (op["calls"] as? [[String: Any]] ?? []).map { call in
            UserOpCall(
                to: call["to"] as? String ?? "",
                value: Self.decimal(call["value"]),
                data: call["data"] as? String ?? "0x"
            )
        }
        let fee = op["fee"] as? [String: Any]
        let quoted = fee.flatMap { fee -> UserOpSpine.Quoted? in
            guard let recipient = fee["recipient"] as? String, let amount = fee["amount"] else { return nil }
            return UserOpSpine.Quoted(amount: Self.decimal(amount), recipient: recipient, tier: fee["tier"] as? String)
        }
        // The coin, before any prompt: a fee in the chain's own coin the
        // executor cannot pay is said, never signed.
        if let quoted, (fee?["token"] == nil || fee?["token"] is NSNull) {
            let value = calls.reduce(UInt128Text.zero) { $0.adding($1.value) }
            if case .ok(let raw as String) = await deps.pool.call(chainId: chainId, method: "eth_getBalance", params: [from, "latest"]),
               let have = UInt128Text(hex: raw) {
                let need = value.adding(quoted.amount)
                if have.less(than: need) {
                    return [
                        "type": "calls_failed",
                        "detail": MultisigShellFailure.cannotPay(need: need.decimal, have: have.decimal).detail,
                        "maybe_sent": false,
                    ]
                }
            }
        }
        let asked = UserOpSpine.Asked(
            method: "vela_multisigExecute",
            paramsJson: CoreJSON.string(Self.executeParams(purpose)),
            origin: ""
        )
        do {
            let submitted = try await deps.spine.submit(
                chainId: chainId,
                account: from,
                calls: calls,
                gasFeeToken: nil,
                quotedFee: quoted,
                asked: asked,
                // The proposals machine follows the execution itself (its
                // `executions`, by this hash): nothing waits on a tracker.
                writeAhead: { _, _ in true }
            )
            if submitted.maybeSent {
                return ["type": "calls_failed", "detail": submitted.userOpHash, "maybe_sent": true]
            }
            return ["type": "calls_sent", "user_op_hash": submitted.userOpHash, "tx_hash": NSNull()]
        } catch let refused as UserOpSpine.Refused {
            if refused.failure == .passkeyCancelled { return ["type": "calls_cancelled"] }
            if case .trustedSigner(.closed) = refused.failure { return ["type": "calls_cancelled"] }
            return ["type": "calls_failed", "detail": Self.shellFailure(refused.failure).detail, "maybe_sent": false]
        } catch {
            return ["type": "calls_failed", "detail": MultisigShellFailure.unknown(MultisigExecutor.describe(error)).detail, "maybe_sent": false]
        }
    }

    /// What the executor's trusted page is shown beside the operation (r7-1):
    /// the transaction, the multisig, its name and how many approvals it
    /// needs (the web's `executeRequest`).
    static func executeParams(_ purpose: [String: Any]?) -> [String: Any] {
        guard let purpose, purpose["kind"] as? String == "execute" else { return [:] }
        return [
            "tx": purpose["tx"] ?? NSNull(),
            "multisig": purpose["multisig"] ?? NSNull(),
            "name": purpose["name"] ?? NSNull(),
            "threshold": purpose["threshold"] ?? NSNull(),
            // r10: its members now, for a member change's before → after.
            "owners": purpose["owners"] ?? [],
            // D1: the one review asks for "… anyway" — the page says "Execute anyway".
            "anyway": purpose["anyway"] as? Bool ?? false,
        ]
    }

    // MARK: - Quote

    /// Price the executor's calls the way every other send is priced: one
    /// fee-policy quote of the real operation, at the relay's price (the
    /// web's `quoteExecutionFee`). Its own fee session, so a quote here never
    /// supersedes the send screen's.
    func quoteCalls(_ op: [String: Any]) async -> [String: Any] {
        let from = op["from"] as? String ?? ""
        let chainId = MultisigExecutor.int(op["chain_id"]) ?? 0
        let calls = (op["calls"] as? [[String: Any]] ?? []).map { call -> [String: Any] in
            ["to": call["to"] as? String ?? "", "value": Self.decimal(call["value"]), "data": (call["data"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "0x"]
        }
        let keys = await deps.accountPort.keys(of: from)
        guard let view = await deps.fees.quote(
            chainId: chainId, account: from, deployed: false,
            publicKeyAvailable: !keys.isEmpty, calls: calls, feeToken: nil, readDeployment: true
        ) else {
            return ["type": "quoted", "quote": ["kind": "failed"]]
        }
        if let estimate = view.fee, view.failed == nil, let recipient = estimate.feeRecipient {
            let amount: String
            switch estimate.feeAsset {
            case .erc20(_, _, let erc20Amount, _): amount = erc20Amount
            case .native: amount = estimate.totalWei
            }
            if (Decimal(string: amount) ?? 0) > 0 {
                var fee: [String: Any] = ["amount": amount, "recipient": recipient]
                if estimate.tier != "rapid" { fee["tier"] = estimate.tier }
                return ["type": "quoted", "quote": [
                    "kind": "ok", "fee": fee,
                    "totalGas": estimate.totalGas, "executorDeployed": estimate.deployed,
                ]]
            }
        }
        if view.failed == "would_fail" {
            return ["type": "quoted", "quote": ["kind": "reverts", "reason": NSNull()]]
        }
        return ["type": "quoted", "quote": ["kind": "failed"]]
    }

    // MARK: - Reads

    func simulate(_ op: [String: Any]) async -> String? {
        let from = op["from"] as? String ?? ""
        let chainId = MultisigExecutor.int(op["chain_id"]) ?? 0
        let legs = (op["calls"] as? [[String: Any]] ?? []).map {
            SimDeltas.Call(to: $0["to"] as? String ?? "", value: Self.decimal($0["value"]), data: $0["data"] as? String ?? "0x")
        }
        guard let payload = Self.simulatePayload(from: from, calls: legs, stateOverrides: op["state_overrides"]) else { return nil }
        let answer = await deps.pool.call(chainId: chainId, method: "eth_simulateV1", params: payload, kind: "rpc")
        return SigningController.simReply(answer)
    }

    /// `eth_simulateV1`'s params, with the core's `state_overrides` put AS
    /// IS into the first block's `stateOverrides` (real-device S2: an
    /// undeployed multisig placed as its deployment leaves it, so its first
    /// transaction is not "expected to fail").
    static func simulatePayload(from: String, calls: [SimDeltas.Call], stateOverrides: Any?) -> [Any]? {
        guard var body = SimDeltas.body(from: from, calls: calls) else { return nil }
        if let overrides = stateOverrides as? [String: Any], !overrides.isEmpty,
           var blocks = body["blockStateCalls"] as? [[String: Any]], !blocks.isEmpty {
            blocks[0]["stateOverrides"] = overrides
            body["blockStateCalls"] = blocks
        }
        return [body, "latest"]
    }

    func recipientFacts(_ op: [String: Any]) async -> (isContract: Bool?, firstTime: Bool?) {
        let chainId = MultisigExecutor.int(op["chain_id"]) ?? 0
        let address = op["address"] as? String ?? ""
        let contract = await deps.relay.isContract(chainId: chainId, address: address)
        let unreadable = deps.shelf.isUnreadable(VelaStore.Key.transactionHistory, expecting: .list)
        let first: Bool? = unreadable ? nil : SendExecutor.firstTime(address, records: TxRecords.load(store: deps.shelf))
        return (contract, first)
    }

    func userOpStatus(_ op: [String: Any]) async -> (status: String?, txHash: String?)? {
        let chainId = MultisigExecutor.int(op["chain_id"]) ?? 0
        let hash = op["user_op_hash"] as? String ?? ""
        guard let answer = await deps.relay.userOpStatus(chainId: chainId, userOpHash: hash) else { return nil }
        return (answer.status, answer.txHash)
    }

    // MARK: - Wallet, notices

    /// iOS has no injected wallet: the outside page runs in a browser.
    func wallet(method: String, params: Any) async throws -> Any {
        throw MultisigWalletRefusal(code: 4900, message: "no wallet connected")
    }

    func notice(_ notice: [String: Any]) async {
        await onNotice?(notice)
    }

    // MARK: - Helpers

    static func bytes(_ hex: String?) -> Data? {
        guard var text = hex else { return nil }
        if text.hasPrefix("0x") || text.hasPrefix("0X") { text = String(text.dropFirst(2)) }
        guard text.count % 2 == 0 else { return nil }
        var out = Data(capacity: text.count / 2)
        var index = text.startIndex
        while index < text.endIndex {
            let next = text.index(index, offsetBy: 2)
            guard let byte = UInt8(text[index..<next], radix: 16) else { return nil }
            out.append(byte)
            index = next
        }
        return out
    }

    static func hex0x(_ text: String) -> String { text.hasPrefix("0x") ? text : "0x" + text }

    /// `0x…` or decimal → decimal text, as a call's value travels.
    static func decimal(_ value: Any?) -> String {
        switch value {
        case let n as NSNumber: return n.stringValue
        case let s as String:
            if s.hasPrefix("0x") || s.hasPrefix("0X") { return UInt128Text(hex: s)?.decimal ?? "0" }
            return s.isEmpty ? "0" : s
        default: return "0"
        }
    }
}

extension TrustedSignerPort {
    /// The multisig's door on the one trusted page (`TrustedSigner.approve`);
    /// a port without one (a test's) answers unavailable.
    func approveMultisig(
        requestJson: String, chainId: UInt64, member: String, safeTxHash: String,
        keys: [WalletKeyRecord], page: String, keyLabel: KeyLabelWire?
    ) async -> TrustedSignerChannel.Ending {
        guard let signer = self as? TrustedSigner else { return .unavailable }
        return await signer.approve(
            requestJson: requestJson, chainId: chainId, member: member, safeTxHash: safeTxHash,
            keys: keys, page: page, keyLabel: keyLabel
        )
    }
}

/// A non-negative integer up to 2^128 as decimal digits: what a balance and
/// a fee are compared in before a prompt. Never a price, never displayed.
struct UInt128Text: Equatable {
    /// Little-endian 32-bit limbs.
    private var limbs: [UInt32]

    static let zero = UInt128Text(limbs: [0])

    private init(limbs: [UInt32]) { self.limbs = limbs }

    init?(hex: String) {
        var text = Substring(hex)
        if text.hasPrefix("0x") || text.hasPrefix("0X") { text = text.dropFirst(2) }
        guard !text.isEmpty, text.allSatisfy(\.isHexDigit) else { return nil }
        var value = UInt128Text.zero
        for char in text {
            value = value.times(16).plus(UInt32(char.hexDigitValue!))
        }
        self = value
    }

    init?(decimal: String) {
        guard !decimal.isEmpty, decimal.allSatisfy(\.isNumber) else { return nil }
        var value = UInt128Text.zero
        for char in decimal { value = value.times(10).plus(UInt32(char.wholeNumberValue!)) }
        self = value
    }

    func adding(_ decimal: String) -> UInt128Text {
        guard let other = UInt128Text(decimal: decimal) ?? UInt128Text(hex: decimal) else { return self }
        return adding(other)
    }

    func adding(_ other: UInt128Text) -> UInt128Text {
        var out: [UInt32] = []
        var carry: UInt64 = 0
        for i in 0..<max(limbs.count, other.limbs.count) {
            let sum = UInt64(i < limbs.count ? limbs[i] : 0) + UInt64(i < other.limbs.count ? other.limbs[i] : 0) + carry
            out.append(UInt32(truncatingIfNeeded: sum))
            carry = sum >> 32
        }
        if carry > 0 { out.append(UInt32(carry)) }
        return UInt128Text(limbs: out).trimmed()
    }

    func less(than other: UInt128Text) -> Bool {
        let a = trimmed().limbs, b = other.trimmed().limbs
        if a.count != b.count { return a.count < b.count }
        for i in stride(from: a.count - 1, through: 0, by: -1) where a[i] != b[i] {
            return a[i] < b[i]
        }
        return false
    }

    var decimal: String {
        var digits: [Character] = []
        var value = trimmed()
        while !(value.limbs.count == 1 && value.limbs[0] == 0) {
            let (quotient, remainder) = value.divided(by: 10)
            digits.append(Character(String(remainder)))
            value = quotient
        }
        return digits.isEmpty ? "0" : String(digits.reversed())
    }

    private func times(_ factor: UInt32) -> UInt128Text {
        var out: [UInt32] = []
        var carry: UInt64 = 0
        for limb in limbs {
            let product = UInt64(limb) * UInt64(factor) + carry
            out.append(UInt32(truncatingIfNeeded: product))
            carry = product >> 32
        }
        if carry > 0 { out.append(UInt32(carry)) }
        return UInt128Text(limbs: out)
    }

    private func plus(_ small: UInt32) -> UInt128Text {
        adding(UInt128Text(limbs: [small]))
    }

    private func divided(by divisor: UInt32) -> (UInt128Text, UInt32) {
        var out = [UInt32](repeating: 0, count: limbs.count)
        var remainder: UInt64 = 0
        for i in stride(from: limbs.count - 1, through: 0, by: -1) {
            let current = (remainder << 32) | UInt64(limbs[i])
            out[i] = UInt32(current / UInt64(divisor))
            remainder = current % UInt64(divisor)
        }
        return (UInt128Text(limbs: out).trimmed(), UInt32(remainder))
    }

    private func trimmed() -> UInt128Text {
        var copy = limbs
        while copy.count > 1, copy.last == 0 { copy.removeLast() }
        return UInt128Text(limbs: copy.isEmpty ? [0] : copy)
    }
}
