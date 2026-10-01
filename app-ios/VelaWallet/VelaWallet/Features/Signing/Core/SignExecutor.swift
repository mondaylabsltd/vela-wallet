//
//  SignExecutor.swift
//  VelaWallet
//
//  The `sign_request` machine's seven arms, and the shell decides none of
//  them.
//
//  Single-flight, "a rejected pipeline may not submit", the record-then-respond
//  order and the account sequencing all live in the core. This answers the
//  transport, writes the record, asks the relay, and runs the ceremony —
//  through `UserOpSpine`, the **same** pipeline a person's own transfer runs.
//  Deliberately: a second signing path is a second set of rules about the
//  same Safe.
//
//  Ported from `app-android/.../feature/signing/core/SignExecutor.kt` (spec
//  044 T029).
//
//  ## It reports twice
//
//  The accepted user-op hash goes back mid-flight through `op_submitted`, so
//  the durable record precedes anything the dApp could poll. Then the final
//  outcome. The page is answered with a **transaction** hash once the receipt
//  arrives — a reverted one included (ruling 9: the page gets its hash, the
//  sheet and Activity say it failed) — and with the op hash when it is late;
//  the tracker alone closes the row.
//
//  ## A lost reply is not a failure (spec 082 RA1–RA3)
//
//  When the relay's answer never came back after a request that may have
//  been acted on, the op is reported `op_submitted{maybe_sent: true}` under
//  its LOCAL hash and waited for exactly like an accepted one: the page gets
//  ONE answer, a hash, never "-32603, try again" (G21's double payment).
//
//  Blocking the page on a receipt is what spec 028 found made
//  `eth_sendTransaction` time out with -32603 on a real dApp.
//
//  ## The record before the bytes (spec 082 RJ1)
//
//  Once the op is signed and its local hash known, it goes to the core
//  (`op_signed`) BEFORE any POST: the core writes the record "may have been
//  sent" and hands it to the tracker, and only then clears the POST
//  (`clear_to_post`). No clearance within `userOpWriteAheadWaitMs`, no POST —
//  the page is told nothing was sent. A quit from there on leaves one pending
//  row the tracker resolves on the next launch (G34).
//
//  ## The answer follows the tracker (spec 082 RJ4)
//
//  The core may answer the page from what the tracker knows — the tx hash,
//  or a refusal — before this executor's receipt wait is over. The wait then
//  ends: its late result is the core's to drop.
//

import Foundation
import VelaCore

@MainActor
final class SignExecutor {

    static let operations = [
        "send_response", "check_bundler_funding", "attempt_sponsorship",
        "sign_and_submit", "persist_record", "update_record", "switch_active_account",
        "clear_to_post", "delete_record",
    ]

    struct Ports {
        /// The answer, to the transport (tab) that owns the request — the
        /// core's own `SignResponsePayload` (`{type: ok, result}` or
        /// `{type: err, code, kind, message}`), untouched. Whoever owns the
        /// transport builds the page's message from it: for the in-app
        /// browser that is `dapp_browser` (spec 070), whose words for a
        /// refusal are the core's rather than a table here.
        var respond: (_ transportId: String, _ id: String, _ payload: [String: Any]) -> Void = { _, _, _ in }
        /// The relay accepted — or may have: `maybeSent` when its reply was
        /// lost (RA3). `submitBlock` is the head read before the POST. The
        /// core must hear this **before** the submit resolves.
        var opSubmitted: (_ id: String, _ userOpHash: String, _ maybeSent: Bool, _ submitBlock: UInt64?) -> Void
        = { _, _, _, _ in }
        /// The op is signed and its local hash known, and nothing has been
        /// POSTed (spec 082 RJ1): the core's `op_signed`, which writes the
        /// record ahead and answers with `clear_to_post`. Unwired, nothing is
        /// ever cleared and nothing is ever sent.
        var opSigned: (_ id: String, _ userOpHash: String, _ submitBlock: UInt64?) -> Void = { _, _, _ in }
        /// The core has answered this request (spec 082 RJ4: from what the
        /// tracker knows): the receipt wait ends.
        var answered: () -> Bool = { false }
        /// A ceremony is about to be raised for request `id` (the core's
        /// `ceremony_started`, RA9).
        var signingStarted: (_ id: String) -> Void = { _ in }
        /// The ceremony for request `id` returned a signature (`ceremony_done`).
        var ceremonyDone: (_ id: String) -> Void = { _ in }
        /// The page that asked is still there (RB2). Asked before the prompt
        /// and before the relay POST; `false` ends the request with nothing
        /// signed, nothing sent and no answer.
        var askerLive: () -> Bool = { true }
        /// When the person approved — the answer window's start (RA12).
        var approvedAtMs: () -> Double? = { nil }
        /// The feed re-reads its store.
        var recordsPersisted: () -> Void = {}
        /// One record is on disk — by its id, so a hand-off naming it waits
        /// for THAT write, not for any write (a delete included).
        var recordWritten: (_ recordId: String) -> Void = { _ in }
        /// The session's active-account switch.
        var switchAccount: (_ index: Int) async -> Bool = { _ in false }
        /// The chain's native symbol, for the record row.
        var nativeSymbol: (_ chainId: Int) -> String = { _ in "" }
        /// Who asked, as the Trusted Signer's page is told it (spec 071): the
        /// origin the browser observed, empty for the wallet's own request.
        var origin: () -> String = { "" }
        /// Spec 079: whether `origin` was read from the in-app browser (a
        /// page), never the wallet's own request.
        var originSeenByBrowser: () -> Bool = { false }
        /// The Trusted Signer ended without a signature. The core hears a
        /// cancelled ceremony — the request stays open and may be signed
        /// another way — and the sheet says which sentence applies.
        var trustedSignerEnded: (TrustedSignerNotice) -> Void = { _ in }
    }

    private let spine: UserOpSpine
    private let relay: RelayClient
    private let store: VelaStore
    var ports: Ports

    /// How long the final answer waits for a receipt before handing back the
    /// op hash, given how long ago the person approved: the core's
    /// `dapp_receipt_wait_ms` (RA12) — what is left of its answer window,
    /// never less than its floor. A test seam, and nothing else.
    private let receiptWaitMs: (_ elapsedMs: Double) -> Double
    private let receiptPollMs: Double

    /// User-op hashes whose pending record is on disk. The response never
    /// precedes the record.
    private var persisted: Set<String> = []

    /// The core's clearance for each POST (RJ1).
    private let gate = WriteAheadGate()
    /// How long a signed op waits for its clearance — the core's
    /// `userOpWriteAheadWaitMs`. A test seam, and nothing else.
    private let clearanceWaitMs: Double

    init(
        spine: UserOpSpine,
        relay: RelayClient,
        store: VelaStore,
        ports: Ports = Ports(),
        receiptWaitMs: @escaping (_ elapsedMs: Double) -> Double = { dappReceiptWaitMs(elapsedMs: $0) },
        receiptPollMs: Double = 3_000,
        clearanceWaitMs: Double = Double(userOpWriteAheadWaitMs())
    ) {
        self.spine = spine
        self.relay = relay
        self.store = store
        self.ports = ports
        self.receiptWaitMs = receiptWaitMs
        self.receiptPollMs = receiptPollMs
        self.clearanceWaitMs = clearanceWaitMs
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "send_response":
            ports.respond(
                operation["transport_id"] as? String ?? "",
                operation["id"] as? String ?? "",
                operation["payload"] as? [String: Any] ?? Self.noAnswer
            )
            return CoreJSON.string(["type": "responded"])

        // `null` means "proceed to submit" — including when the check itself
        // failed. The core's own doc is explicit that a timed-out or errored
        // pre-check is not a refusal, and the submit's underfunded answer is
        // the authority. Desktop and Android answer the same.
        case "check_bundler_funding":
            return CoreJSON.string(["type": "pre_check", "funding": NSNull()])

        // Denied with no reason rather than invented: sponsorship is a relay
        // feature this shell does not reach, and `funded` would be a claim
        // that somebody else paid.
        case "attempt_sponsorship":
            return CoreJSON.string([
                "type": "sponsorship",
                "outcome": ["type": "denied", "reason": NSNull()],
            ])

        case "sign_and_submit":
            return CoreJSON.string([
                "type": "submit",
                "outcome": await signAndSubmit(operation),
                "now_ms": Date().timeIntervalSince1970 * 1000,
            ])

        case "persist_record":
            let record = operation["record"] as? [String: Any] ?? [:]
            let row = Self.recordRow(
                record,
                nativeSymbol: ports.nativeSymbol((record["chain_id"] as? NSNumber)?.intValue ?? 0)
            )
            TxRecords.writeRecords([row], store: store)
            if let hash = record["user_op_hash"] as? String, !hash.isEmpty {
                persisted.insert(hash.lowercased())
            }
            ports.recordsPersisted()
            if let id = row["id"] as? String, !id.isEmpty { ports.recordWritten(id) }
            return CoreJSON.string(["type": "record_persisted"])

        case "update_record":
            let id = operation["record_id"] as? String ?? ""
            let close = operation["close"] as? [String: Any] ?? [:]
            var fields: [String: Any] = [:]
            switch close["type"] as? String {
            case "confirmed":
                fields["status"] = "confirmed"
                if let txHash = close["tx_hash"] as? String, !txHash.isEmpty {
                    fields["txHash"] = txHash
                }
            // The relay took the op the write-ahead record announced (RJ1):
            // it is no longer "may have been sent", and it stays pending for
            // the tracker to close.
            case "admitted":
                fields["maybeSent"] = NSNull()
            default:
                fields["status"] = "failed"
            }
            TxRecords.patch(ids: [id], fields: fields, store: store)
            ports.recordsPersisted()
            return CoreJSON.string(["type": "record_updated"])

        // The core cleared this op's POST (RJ1): its record is on disk.
        case "clear_to_post":
            gate.open(operation["user_op_hash"] as? String ?? "")
            return CoreJSON.string(["type": "responded"])

        // A write-ahead record the core proved never sent (RJ1): the row goes.
        case "delete_record":
            TxRecords.delete(id: operation["record_id"] as? String ?? "", store: store)
            ports.recordsPersisted()
            return CoreJSON.string(["type": "record_updated"])

        // Best effort, and the core is told either way: it sequences "switch
        // first, then the approval surface may act" off this acknowledgement,
        // so withholding it would strand the grant.
        case "switch_active_account":
            _ = await ports.switchAccount((operation["index"] as? NSNumber)?.intValue ?? 0)
            return CoreJSON.string(["type": "account_switched"])

        default:
            VelaLog.failure(.sign, kind: "unhandled_operation", "\(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "send_response": return CoreJSON.string(["type": "responded"])
        case "check_bundler_funding": return CoreJSON.string(["type": "pre_check", "funding": NSNull()])
        case "attempt_sponsorship":
            return CoreJSON.string([
                "type": "sponsorship", "outcome": ["type": "denied", "reason": NSNull()],
            ])
        case "sign_and_submit":
            return CoreJSON.string([
                "type": "submit",
                "outcome": Self.failed("Signing failed"),
                "now_ms": Date().timeIntervalSince1970 * 1000,
            ])
        case "persist_record": return CoreJSON.string(["type": "record_persisted"])
        case "update_record", "delete_record": return CoreJSON.string(["type": "record_updated"])
        // Acknowledged, never opened: an op whose clearance was lost is not
        // POSTed (RJ1).
        case "clear_to_post": return CoreJSON.string(["type": "responded"])
        default: return CoreJSON.string(["type": "account_switched"])
        }
    }

    // MARK: - The ceremony

    private func signAndSubmit(_ operation: [String: Any]) async -> [String: Any] {
        let method = operation["method"] as? String ?? ""
        let paramsJson = operation["params_json"] as? String ?? "[]"
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
        let address = operation["address"] as? String ?? ""
        let id = operation["id"] as? String ?? ""

        if method == "personal_sign" || method == "eth_sign" || method.contains("signTypedData") {
            return await signMessage(id: id, method: method, paramsJson: paramsJson, chainId: chainId, address: address)
        }

        guard let calls = Self.callsOf(method: method, paramsJson: paramsJson) else {
            return Self.failed("\(method) carried no transaction this wallet could read")
        }

        let quoted = (operation["quoted_fee"] as? [String: Any]).map {
            UserOpSpine.Quoted(
                amount: $0["amount"] as? String ?? "0",
                recipient: $0["recipient"] as? String ?? "",
                // The speed the displayed fee was priced at (spec 069).
                tier: $0["tier"] as? String
            )
        }

        do {
            let submitted = try await spine.submit(
                chainId: chainId,
                account: address,
                calls: calls,
                gasFeeToken: operation["gas_fee_token"] as? String,
                quotedFee: quoted,
                // The FINAL params — a guard's rewrite included — are what the
                // page decodes, as they are what is signed.
                asked: UserOpSpine.Asked(
                    method: method, paramsJson: paramsJson, origin: ports.origin(),
                    seenByBrowser: ports.originSeenByBrowser()
                ),
                signingStarted: { [ports] in ports.signingStarted(id) },
                signingDone: { [ports] in ports.ceremonyDone(id) },
                askerLive: { [ports] in ports.askerLive() },
                writeAhead: { [weak self] hash, block in
                    await self?.writeAhead(id: id, hash: hash, block: block) ?? false
                }
            )
            let hash = submitted.userOpHash
            VelaLog.notice(
                .sign,
                "submit verdict=\(submitted.maybeSent ? "maybe_sent" : "accepted") hash=\(VelaLog.short(hash)) chain=\(chainId)"
            )
            ports.opSubmitted(id, hash, submitted.maybeSent, submitted.submitBlock)
            // The durable record precedes anything the dApp could poll: the
            // core persists it on `op_submitted`, and the answer waits for it.
            await waitForRecord(of: hash)
            let elapsed = ports.approvedAtMs().map { Date().timeIntervalSince1970 * 1000 - $0 } ?? 0
            let receipt = await awaitReceipt(
                chainId: chainId, userOpHash: hash, waitMs: receiptWaitMs(elapsed)
            )
            if let receipt, !receipt.confirmed {
                // Landed and reverted: the page hears the revert, naming the
                // transaction — never the hash a site reads as done (083,
                // owner ruling 2026-10-01). The tracker, polling the same
                // receipt, closes the record failed. Nothing here patches it.
                VelaLog.failure(.sign, kind: "reverted", "hash=\(VelaLog.short(hash)) tx=\(VelaLog.short(receipt.txHash))")
            }
            return Self.afterReceiptWait(
                userOpHash: hash, receipt: receipt?.txHash, reverted: receipt?.confirmed == false
            )
        } catch let refused as UserOpSpine.Refused {
            switch refused.failure {
            case .passkeyCancelled:
                return ["type": "passkey_cancelled"]
            case .trustedSigner(let notice):
                ports.trustedSignerEnded(notice)
                return ["type": "passkey_cancelled"]
            case .askerGone:
                VelaLog.notice(.sign, "asker gone before \(method) was sent — nothing signed or sent")
                return ["type": "asker_gone"]
            case .bundlerUnderfunded:
                return [
                    "type": "underfunded",
                    "message": "The relay's gas account is underfunded",
                    "funding": NSNull(),
                ]
            case .relayerUnavailable:
                return Self.failed("The gas relayer is unavailable right now")
            case .notSent:
                // The core's fixed sentence, never the pool's text (RA10):
                // true, and nothing in it for a page to misread.
                return Self.failed(userOpNotSentDetail())
            // The relay refused it (RJ3): the page is answered the core's
            // "the network refused this" whatever the sentence, and the sheet
            // never says "try again".
            case .rejected(let message):
                VelaLog.failure(.sign, kind: "refused", "chain=\(chainId)")
                return Self.failed(message ?? userOpRefusedDappDetail(), refused: true)
            case .other(let message):
                return Self.failed(message ?? "Signing failed")
            }
        } catch {
            return Self.failed("Signing failed")
        }
    }

    /// `SignSubmitOutcome::Failed`, whole: `refused` is the relay's refusal
    /// (RJ3), `false` for everything else.
    static func failed(_ message: String, refused: Bool = false) -> [String: Any] {
        ["type": "failed", "message": message, "refused": refused]
    }

    /// The record before the bytes (RJ1): tell the core the op is signed,
    /// then wait for it to clear the POST. `false` sends nothing.
    private func writeAhead(id: String, hash: String, block: UInt64?) async -> Bool {
        let started = Date()
        gate.arm(hash)
        ports.opSigned(id, hash, block)
        let cleared = await gate.wait(hash, ms: clearanceWaitMs)
        if cleared {
            VelaLog.notice(.sign, "write-ahead cleared hash=\(VelaLog.short(hash)) in=\(VelaLog.ms(since: started))")
        } else {
            VelaLog.failure(.sign, kind: "write_ahead_timeout", "hash=\(VelaLog.short(hash)) after=\(VelaLog.ms(since: started))")
        }
        return cleared
    }

    /// A message: hashed the way the page's verifier hashes it, signed once,
    /// answered as the EIP-1271 envelope.
    private func signMessage(
        id: String, method: String, paramsJson: String, chainId: Int, address: String
    ) async -> [String: Any] {
        guard let original = Self.messageHash(method: method, paramsJson: paramsJson) else {
            return Self.failed("\(method) carried nothing this wallet could sign")
        }
        do {
            let signature = try await spine.signMessage(
                chainId: chainId,
                account: address,
                originalHash: original,
                asked: UserOpSpine.Asked(
                    method: method, paramsJson: paramsJson, origin: ports.origin(),
                    seenByBrowser: ports.originSeenByBrowser()
                ),
                signingStarted: { [ports] in ports.signingStarted(id) },
                signingDone: { [ports] in ports.ceremonyDone(id) },
                askerLive: { [ports] in ports.askerLive() }
            )
            return ["type": "succeeded", "result": signature]
        } catch let refused as UserOpSpine.Refused {
            if case .passkeyCancelled = refused.failure { return ["type": "passkey_cancelled"] }
            if case .askerGone = refused.failure { return ["type": "asker_gone"] }
            if case .trustedSigner(let notice) = refused.failure {
                ports.trustedSignerEnded(notice)
                return ["type": "passkey_cancelled"]
            }
            if case .other(let message) = refused.failure {
                return Self.failed(message ?? "Signing failed")
            }
            return Self.failed("Signing failed")
        } catch {
            return Self.failed("Signing failed")
        }
    }

    /// Bounded: five seconds, then answer anyway. The record is on its way and
    /// a page held open forever is worse than a page answered a moment early.
    private func waitForRecord(of hash: String) async {
        let key = hash.lowercased()
        let deadline = Date().addingTimeInterval(5)
        while !persisted.contains(key), Date() < deadline {
            try? await Task.sleep(nanoseconds: 50_000_000)
        }
    }

    /// The receipt, if one comes within `waitMs`: its tx hash and whether the
    /// op succeeded. `confirmed: false` is kept (RA8) — a revert is still a
    /// landing, and still the page's hash.
    ///
    /// Ends early once the core has answered the request from the tracker
    /// (RJ4): nothing this wait finds can be the page's answer any more.
    private func awaitReceipt(
        chainId: Int, userOpHash: String, waitMs: Double
    ) async -> (txHash: String, confirmed: Bool)? {
        let deadline = Date().addingTimeInterval(waitMs / 1000)
        let answered = ports.answered
        while !answered() {
            let remaining = deadline.timeIntervalSinceNow
            if remaining <= 0 { break }
            // Each poll gets only what is left of the window (spec 079,
            // Android device pass): with the relay unreachable one poll hung
            // for its own timeouts and retries, and the page waited 268 s for
            // a two-minute wait. A late answer is dropped; the tracker keeps
            // following the operation either way.
            let relay = self.relay
            let answer = await Self.within(seconds: remaining, until: answered) {
                await relay.userOpReceipt(chainId: chainId, userOpHash: userOpHash)
            }
            if case .resolved(let confirmed, let txHash, _, _)? = answer, !txHash.isEmpty {
                return (txHash, confirmed)
            }
            let left = deadline.timeIntervalSinceNow
            if left <= 0 || answered() { break }
            let resume = Date().addingTimeInterval(min(receiptPollMs / 1000, left))
            while !answered() {
                let pause = resume.timeIntervalSinceNow
                if pause <= 0 { break }
                try? await Task.sleep(nanoseconds: UInt64(min(pause, 0.05) * 1_000_000_000))
            }
        }
        if answered() {
            VelaLog.notice(.sign, "receipt wait ended: answered hash=\(VelaLog.short(userOpHash))")
        }
        return nil
    }

    /// `work`'s answer if it comes within `seconds`, else `nil` — and the
    /// caller goes on at the deadline, whatever `work` is still doing (it is
    /// cancelled, and a late answer is dropped). The first to finish wins;
    /// both sides run on the main actor, so exactly one resumes.
    ///
    /// `until` ends the wait early the moment it holds (checked every 50 ms):
    /// the request was answered, and nobody needs `work` any more.
    static func within<T>(
        seconds: Double,
        until stop: @escaping @MainActor () -> Bool = { false },
        _ work: @escaping @MainActor () async -> T
    ) async -> T? {
        let once = FirstOnce()
        return await withCheckedContinuation { (continuation: CheckedContinuation<T?, Never>) in
            let job = Task { @MainActor in
                let value = await work()
                guard !once.done else { return }
                once.done = true
                continuation.resume(returning: value)
            }
            Task { @MainActor in
                let deadline = Date().addingTimeInterval(max(0, seconds))
                while !once.done, !stop() {
                    let left = deadline.timeIntervalSinceNow
                    if left <= 0 { break }
                    try? await Task.sleep(nanoseconds: UInt64(min(left, 0.05) * 1_000_000_000))
                }
                guard !once.done else { return }
                once.done = true
                job.cancel()
                continuation.resume(returning: nil)
            }
        }
    }

    /// Which side of `within` answered first.
    private final class FirstOnce { var done = false }

    // MARK: - The pure parts

    /// What the receipt wait means for the core.
    ///
    /// In time: `succeeded` with the TX hash — a dApp's `eth_sendTransaction`
    /// resolves to a tx hash — or `reverted` when the receipt says the op
    /// reverted (083: the core answers the page the revert). Late:
    /// `receipt_pending` with the op hash — the core answers a transaction
    /// "not confirmed yet" and leaves the record PENDING for the tracker. A
    /// late receipt is not a confirmation (issue 262: an op that never landed
    /// was recorded "confirmed").
    static func afterReceiptWait(
        userOpHash: String, receipt: String?, reverted: Bool = false
    ) -> [String: Any] {
        guard let txHash = receipt else {
            return ["type": "receipt_pending", "user_op_hash": userOpHash]
        }
        if reverted {
            return ["type": "reverted", "user_op_hash": userOpHash, "tx_hash": txHash]
        }
        return ["type": "succeeded", "result": txHash]
    }

    /// What the page's verifier will hash.
    ///
    /// `personal_sign` is the EIP-191 prefix over the bytes; `eth_sign` is the
    /// same envelope over `params[1]` (EIP-1474's rule, the params swapped);
    /// typed data is its EIP-712 digest, computed by the core.
    /// What the site asked to sign, before the Safe's wrap — the core's one
    /// rule (`sign_message::original_hash`), shared with Android, the desktop
    /// and the Trusted Signer's page. The copy that lived here read typed data
    /// from `params[1]` even for `eth_signTypedData`, which carries it first.
    static func messageHash(method: String, paramsJson: String) -> Data? {
        signMessageHash(method: method, paramsJson: paramsJson)
    }

    /// What a transport is told when the operation carried no payload at
    /// all — a shape drift, answered rather than left hanging.
    static let noAnswer: [String: Any] = [
        "type": "err", "code": -32603, "kind": "submit_failed",
        "message": "The wallet produced no answer",
    ]

    /// The calls a request carries: one for `eth_sendTransaction`, many for
    /// `wallet_sendCalls` — and **an empty batch is not a batch**. Hex value
    /// on the wire, decimal to the core.
    static func callsOf(method: String, paramsJson: String) -> [UserOpCall]? {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any],
              let first = params.first as? [String: Any]
        else { return nil }

        if method == "wallet_sendCalls" {
            guard let raw = first["calls"] as? [[String: Any]] else { return nil }
            let calls = raw.compactMap(call(from:))
            return calls.isEmpty ? nil : calls
        }
        return call(from: first).map { [$0] }
    }

    private static func call(from raw: [String: Any]) -> UserOpCall? {
        guard let to = raw["to"] as? String, !to.isEmpty else { return nil }
        let valueHex = (raw["value"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "0x0"
        guard let value = GuardExecutor.decimal(fromWordHex: padded(valueHex)) else { return nil }
        let dataHex = (raw["data"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "0x"
        return UserOpCall(to: to, value: value, data: dataHex)
    }

    /// The decimal converter wants whole bytes; a dApp writes `0x1`.
    private static func padded(_ hex: String) -> String {
        let digits = hex.hasPrefix("0x") ? String(hex.dropFirst(2)) : hex
        return digits.count % 2 == 0 ? "0x" + digits : "0x0" + digits
    }

    /// The feed's row for a dApp's request.
    ///
    /// A signature carries **no** value and **no** symbol: it moves nothing,
    /// and a row that claimed a value would show up in the feed as money.
    ///
    /// A `wallet_sendCalls` row names its first leg, as its sheet does (RC7),
    /// and the SUM of every leg's value: its `params[0]` is `{calls}`, not a
    /// transaction, and reading `.value` there recorded every batch as moving
    /// nothing. `maybeSent` / `submitBlock` are kept with the row (spec 082
    /// T183), so a relaunch follows a may-have-been-sent op as one.
    static func recordRow(_ record: [String: Any], nativeSymbol: String) -> [String: Any] {
        let paramsJson = record["params_json"] as? String ?? "[]"
        let first = (try? JSONSerialization.jsonObject(
            with: Data(paramsJson.utf8)
        )).flatMap { ($0 as? [Any])?.first as? [String: Any] }

        let kind: String
        var to = ""
        var value = "0"
        var symbol = ""
        var decimals = 0
        switch record["kind"] as? String ?? "" {
        case "dapp_tx":
            kind = "dapp_tx"
            let method = record["method"] as? String ?? ""
            let leg = method == "wallet_sendCalls"
                ? (first?["calls"] as? [[String: Any]])?.first
                : first
            to = leg?["to"] as? String ?? ""
            value = requestedValueHex(method: method, first: first)
            symbol = nativeSymbol
            decimals = 18
        case "sign_typed_data":
            kind = "sign_typed_data"
        default:
            kind = "sign_message"
        }

        let clipped = paramsJson.utf8.count > 4096
        var row: [String: Any] = [
            "id": record["record_id"] as? String ?? "",
            "userOpHash": record["user_op_hash"] as? String ?? "",
            "txHash": record["result"] as? String ?? "",
            "from": record["from"] as? String ?? "",
            "to": to,
            "value": value,
            "symbol": symbol,
            "decimals": decimals,
            "chainId": (record["chain_id"] as? NSNumber)?.intValue ?? 0,
            // SECONDS.
            "timestamp": Int(((record["now_ms"] as? NSNumber)?.doubleValue ?? 0) / 1000),
            "status": (record["status"] as? String) == "confirmed" ? "confirmed" : "pending",
            "type": kind,
            "dappOrigin": record["dapp_origin"] as? String ?? "",
            // 083 H2: the origin the request arrived from — what Activity
            // names the site by.
            "dappUrl": record["dapp_url"] as? String ?? "",
            "signedRequest": clipped ? String(paramsJson.prefix(4096)) : paramsJson,
            "requestTruncated": clipped,
        ]
        if let intent = record["intent"] as? String, !intent.isEmpty { row["intent"] = intent }
        if record["maybe_sent"] as? Bool == true { row["maybeSent"] = true }
        if let block = record["submit_block"] as? NSNumber { row["submitBlock"] = block.uint64Value }
        return row
    }

    /// The native value a transaction request moves, as hex wei: `value` of an
    /// `eth_sendTransaction` as written, the sum over every leg of a
    /// `wallet_sendCalls`. Hex or decimal legs; anything unreadable counts as
    /// zero (the submit refuses it anyway).
    static func requestedValueHex(method: String, first: [String: Any]?) -> String {
        guard method == "wallet_sendCalls" else {
            return (first?["value"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "0x0"
        }
        var total = "0"
        for call in first?["calls"] as? [[String: Any]] ?? [] {
            guard let raw = call["value"] as? String, !raw.isEmpty, raw != "0x" else { continue }
            let decimal: String?
            if raw.hasPrefix("0x") || raw.hasPrefix("0X") {
                decimal = TokenReads.scaled(hex: raw, decimals: 0)
            } else {
                decimal = raw.allSatisfy { $0.isASCII && $0.isNumber } ? raw : nil
            }
            if let decimal { total = TokenReads.addDecimal(total, decimal) }
        }
        return RelayClient.hexQuantity(decimal: total)
    }
}
