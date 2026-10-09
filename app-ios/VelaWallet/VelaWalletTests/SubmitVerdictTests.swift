//
//  SubmitVerdictTests.swift
//  VelaWalletTests
//
//  Spec 082 T104 (RA1, RA5, RA10, ruling 8): what a submit came to is the
//  core's `submit_step`, driven by what the pool could say about delivery.
//
//  G21 was a paid double spend: the relay received the op, its reply was
//  lost, the wallet said "failed — try again", and the second attempt landed
//  too. Every case here is a stub transport, so the three answers are proved
//  without a network:
//
//  - a POST that may have been acted on and never answered → MaybeSent,
//    under the op's own locally computed hash;
//  - nothing left the device → NotSent, and the page is told the core's
//    fixed sentence, never the pool's text;
//  - the relay's `[existingHash:0x…]` marker → Accepted with that hash.
//
//  Spec 082 round 2 (T237, RJ1, RJ3): the record before the bytes — nothing
//  is POSTed before the core clears it, no clearance is no POST at all, and a
//  relay's refusal is `failed{refused: true}`, never "try again".
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SubmitVerdictTests {

    private let local = "0x" + String(repeating: "11", count: 32)
    private let relayHash = "0x" + String(repeating: "22", count: 32)

    private func client(_ port: ScriptedRelayPort) -> RelayClient {
        RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
    }

    private func answer(_ outcome: RpcOutcome, delivered: Bool, held: String? = nil) -> RpcCallResult {
        RpcCallResult(outcome: outcome, maybeDelivered: delivered, heldErrorJson: held)
    }

    // MARK: - The relay client's loop

    /// The relay went mute after the request left: may have been sent, and
    /// followed under the LOCAL hash — never "failed".
    @Test func aMuteRelayIsMaybeSentUnderTheLocalHash() async {
        let port = ScriptedRelayPort()
        port.detailed["eth_sendUserOperation"] = [answer(.failed(rateLimited: false), delivered: true)]
        let verdict = await client(port).sendUserOp(chainId: 100, opJson: "{}", localHash: local)
        #expect(verdict == .maybeSent(userOpHash: local))
        #expect(port.calls.filter { $0 == "eth_sendUserOperation" }.count == 1, "a lost reply is not re-POSTed")
    }

    /// Nothing left the device (DNS, refused, TLS — `not_connected` on every
    /// POST): the one case where "not sent" is true.
    @Test func aRelayNeverReachedIsNotSent() async {
        let port = ScriptedRelayPort()
        port.detailed["eth_sendUserOperation"] = [answer(.failed(rateLimited: false), delivered: false)]
        let verdict = await client(port).sendUserOp(chainId: 100, opJson: "{}", localHash: local)
        #expect(verdict == .notSent(rejection: nil))
    }

    /// A refusal no earlier attempt could have caused is NotSent, in the
    /// core's class — the sentence is `relay_error_message`'s, not the jargon.
    @Test func aRefusalIsNotSentInTheCoresClass() async {
        let port = ScriptedRelayPort()
        port.detailed["eth_sendUserOperation"] = [
            answer(.rpcError(code: -32521, message: "AA25 invalid account nonce"), delivered: false),
        ]
        let verdict = await client(port).sendUserOp(chainId: 100, opJson: "{}", localHash: local)
        #expect(verdict == .notSent(rejection: .other(message: "Transaction nonce mismatch. Please try again.")))
    }

    /// The marker names the op already pending for this nonce. When it is
    /// THIS op (a retried POST that had arrived) it is accepted; another op's
    /// hash is never this request's (083): not sent, the nonce held by it. It
    /// is searched in the raw error first: the translator replaces an AA25
    /// sentence wholesale and the marker would go with it.
    @Test func theExistingHashMarkerIsAccepted() async {
        func verdict(marking marked: String) async -> RelayClient.SubmitVerdict {
            let port = ScriptedRelayPort()
            port.detailed["eth_sendUserOperation"] = [answer(
                .rpcError(code: -32521, message: "AA25 invalid account nonce [existingHash:\(marked)]"),
                delivered: false,
                held: #"{"code":-32521,"message":"AA25 invalid account nonce [existingHash:\#(marked)]"}"#
            )]
            return await client(port).sendUserOp(chainId: 100, opJson: "{}", localHash: local)
        }
        #expect(await verdict(marking: local) == .accepted(userOpHash: local))
        let another = "0x" + String(repeating: "ab", count: 32)
        #expect(await verdict(marking: another) == .notSent(rejection: .nonceHeld(userOpHash: another)))
    }

    /// The relay's hash wins over the local one.
    @Test func theRelaysHashWins() async {
        let port = ScriptedRelayPort()
        port.detailed["eth_sendUserOperation"] = [answer(.ok(relayHash), delivered: false)]
        let verdict = await client(port).sendUserOp(chainId: 100, opJson: "{}", localHash: local)
        #expect(verdict == .accepted(userOpHash: relayHash))
    }

    /// "Currently processing" is the core's to retry — the identical op, up
    /// to its limit — and then a refusal like any other.
    @Test func aBusyRelayIsRetriedByTheCoreAndThenReported() async {
        let port = ScriptedRelayPort()
        port.detailed["eth_sendUserOperation"] = [
            answer(.rpcError(code: -32000, message: "currently processing"), delivered: false),
        ]
        let verdict = await client(port).sendUserOp(chainId: 100, opJson: "{}", localHash: local)
        guard case .notSent(.some) = verdict else {
            Issue.record("a busy relay that never frees up is a refusal, got \(verdict)")
            return
        }
        // One attempt plus the core's three retries.
        #expect(port.calls.filter { $0 == "eth_sendUserOperation" }.count == 4)
    }

    /// A refusal on attempt two proves nothing when attempt one may have been
    /// delivered: the delivery is OR-ed over the whole submit.
    @Test func aRefusalAfterADeliveryThatMayHaveHappenedIsMaybeSent() async {
        let port = ScriptedRelayPort()
        port.detailed["eth_sendUserOperation"] = [
            // The pool timed out on one endpoint, then the next said busy.
            answer(.rpcError(code: -32000, message: "currently processing"), delivered: true),
            answer(.rpcError(code: -32521, message: "AA25 invalid account nonce"), delivered: false),
        ]
        let verdict = await client(port).sendUserOp(chainId: 100, opJson: "{}", localHash: local)
        #expect(verdict == .maybeSent(userOpHash: local))
    }

    /// Every reply shape reaches the core as its `SubmitReply`.
    @Test func eachAnswerIsTheCoresReply() {
        #expect(RelayClient.submitReply(answer(.ok(relayHash), delivered: false)) == .hash(hash: relayHash))
        #expect(RelayClient.submitReply(answer(.failed(rateLimited: false), delivered: true)) == .noAnswer)
        #expect(RelayClient.submitReply(answer(
            .rpcError(code: 1, message: "x"), delivered: false, held: #"{"code":1,"message":"x"}"#
        )) == .rpcError(errorJson: #"{"code":1,"message":"x"}"#))
        // A 200 with no error member is the relay's RESULT, whatever it holds:
        // whether it is a hash is the core's to say (`submit_step`), never a
        // refusal made up here (082 review; the desktop and Android agree).
        #expect(RelayClient.submitReply(answer(.ok(NSNull()), delivered: false)) == .hash(hash: ""))
        #expect(RelayClient.submitReply(answer(.ok(nil), delivered: false)) == .hash(hash: ""))
        #expect(RelayClient.submitReply(answer(.ok("pending"), delivered: false)) == .hash(hash: "pending"))
    }

    /// The relay answered 200 with no error and no readable hash (a null, a
    /// word, an object): it spoke without refusing, so it may hold the op.
    /// "Failed — try again" here is G21's double payment; the core's rule is
    /// "may have been sent" under the local hash (082 review).
    @Test func anAnswerWithNoReadableHashIsMaybeSentNeverNotSent() async {
        for result in [NSNull() as Any, "", "pending", ["userOpHash": "0x12"] as [String: Any]] {
            let port = ScriptedRelayPort()
            port.detailed["eth_sendUserOperation"] = [answer(.ok(result), delivered: false)]
            let verdict = await client(port).sendUserOp(chainId: 100, opJson: "{}", localHash: local)
            #expect(verdict == .maybeSent(userOpHash: local), "result \(result) read as \(verdict)")
        }
    }

    // MARK: - The spine, end to end (signed on the Trusted Signer's page)

    private let fixture = TrustedSignerFixture()

    private func spine(_ port: ScriptedRelayPort) -> (UserOpSpine, ScriptedTrustedSigner) {
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.recordJson = fixture.pageRecordJson
        let spine = UserOpSpine(relay: client(port), accounts: accounts, signer: { CountingSigner() })
        let fixture = self.fixture
        let page = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        spine.trustedSigner = page
        return (spine, page)
    }

    private func readyPort() -> ScriptedRelayPort {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        port.rpc["eth_blockNumber"] = .ok("0x2e3b9d9")
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        return port
    }

    /// A mute relay after the signature: the spine answers the op's own hash,
    /// marked may-have-been-sent, with the head read before the POST — and
    /// the write-ahead got that same hash and head before the POST (RJ1).
    @Test func theSpineAnswersAMaybeSentOpWithItsLocalHashAndHead() async throws {
        let port = readyPort()
        port.detailed["eth_sendUserOperation"] = [answer(.failed(rateLimited: false), delivered: true)]
        let (spine, _) = spine(port)
        var written: (hash: String, block: UInt64?, callsBefore: [String])?
        let submitted = try await spine.submit(
            chainId: 100, account: fixture.account,
            calls: [UserOpCall(to: fixture.account, value: "1000", data: "0x")], gasFeeToken: nil,
            quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: fixture.account),
            writeAhead: { hash, block in
                written = (hash, block, port.calls)
                return true
            }
        )
        #expect(submitted.maybeSent)
        #expect(submitted.userOpHash.hasPrefix("0x") && submitted.userOpHash.count == 66)
        #expect(submitted.submitBlock == 0x2e3b9d9)
        // The head was asked before the relay was.
        let head = try #require(port.calls.firstIndex(of: "eth_blockNumber"))
        let post = try #require(port.calls.firstIndex(of: "eth_sendUserOperation"))
        #expect(head < post)
        let ahead = try #require(written)
        #expect(ahead.hash == submitted.userOpHash, "the record is written under the op's own hash")
        #expect(ahead.block == 0x2e3b9d9)
        #expect(!ahead.callsBefore.contains("eth_sendUserOperation"), "the record precedes the bytes")
    }

    /// RJ1: nothing is POSTed until the core clears it — however long that
    /// takes — and then exactly once.
    @Test func noPostBeforeTheClearance() async throws {
        let port = readyPort()
        port.rpc["eth_sendUserOperation"] = .ok(relayHash)
        let (spine, _) = spine(port)
        let gate = WriteAheadGate()
        let signed = Flag()
        let local = Recorded()
        let account = fixture.account
        let submit = Task { @MainActor in
            try await spine.submit(
                chainId: 100, account: account,
                calls: [UserOpCall(to: account, value: "1000", data: "0x")], gasFeeToken: nil,
                quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: account),
                writeAhead: { hash, block in
                    local.signed.append((hash, block, false))
                    gate.arm(hash)
                    signed.set()
                    return await gate.wait(hash, ms: 60_000)
                }
            )
        }
        await Wait.until { signed.isSet }
        // Signed, hashed — and held: the relay has not been asked.
        for _ in 0..<20 { await Task.yield() }
        try await Task.sleep(nanoseconds: 50_000_000)
        #expect(!port.calls.contains("eth_sendUserOperation"), "no POST before the clearance")

        gate.open(try #require(local.signed.first?.hash))
        let submitted = try await submit.value
        #expect(submitted.userOpHash == relayHash)
        #expect(port.calls.filter { $0 == "eth_sendUserOperation" }.count == 1)
    }

    /// RJ1: no clearance, no POST — the op is not sent, and says so.
    @Test func noClearanceIsNoPostAndNotSent() async {
        let port = readyPort()
        port.rpc["eth_sendUserOperation"] = .ok(relayHash)
        let (spine, _) = spine(port)
        var caught: UserOpSpine.Failure?
        do {
            _ = try await spine.submit(
                chainId: 100, account: fixture.account,
                calls: [UserOpCall(to: fixture.account, value: "1000", data: "0x")], gasFeeToken: nil,
                quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: fixture.account),
                writeAhead: { _, _ in false }
            )
        } catch let refused as UserOpSpine.Refused {
            caught = refused.failure
        } catch {}
        #expect(caught == .notSent)
        #expect(!port.calls.contains("eth_sendUserOperation"), "zero POSTs")
    }

    /// Nothing reached the relay: the spine's refusal is `notSent` — the one
    /// failure after a signature that may say "not sent".
    @Test func theSpineRefusesNotSentWhenNothingLeft() async {
        let port = readyPort()
        port.detailed["eth_sendUserOperation"] = [answer(.failed(rateLimited: false), delivered: false)]
        let (spine, _) = spine(port)
        var caught: UserOpSpine.Failure?
        do {
            _ = try await spine.submit(
                chainId: 100, account: fixture.account,
                calls: [UserOpCall(to: fixture.account, value: "1000", data: "0x")], gasFeeToken: nil,
                quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: fixture.account),
                writeAhead: { _, _ in true }
            )
        } catch let refused as UserOpSpine.Refused {
            caught = refused.failure
        } catch {}
        #expect(caught == .notSent)
    }

    /// A page that is gone gets nothing signed and nothing sent (RB2).
    @Test func aGonePageGetsNoPromptAndNoPost() async {
        let port = readyPort()
        port.rpc["eth_sendUserOperation"] = .ok(relayHash)
        let (spine, page) = spine(port)
        var caught: UserOpSpine.Failure?
        do {
            _ = try await spine.submit(
                chainId: 100, account: fixture.account,
                calls: [UserOpCall(to: fixture.account, value: "1000", data: "0x")], gasFeeToken: nil,
                quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: fixture.account),
                askerLive: { false },
                writeAhead: { _, _ in true }
            )
        } catch let refused as UserOpSpine.Refused {
            caught = refused.failure
        } catch {}
        #expect(caught == .askerGone)
        #expect(page.asked.isEmpty, "no prompt for a page that left")
        #expect(!port.calls.contains("eth_sendUserOperation"))
    }

    // MARK: - What the dApp is told

    /// The executor, with the core's write-ahead played by the test: on
    /// `op_signed` the record is persisted and the POST cleared, in that
    /// order — unless `clears` is false, when nothing ever clears it.
    private func signExecutor(
        _ port: ScriptedRelayPort, clears: Bool = true, clearanceWaitMs: Double = 60_000
    ) -> (SignExecutor, Recorded) {
        let (spine, _) = spine(port)
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let executor = SignExecutor(
            spine: spine, relay: client(port), store: store, receiptWaitMs: { _ in 0 },
            clearanceWaitMs: clearanceWaitMs
        )
        let recorded = Recorded()
        executor.ports.opSigned = { [weak executor] id, hash, block in
            recorded.signed.append((hash, block, port.calls.contains("eth_sendUserOperation")))
            guard clears else { return }
            Task { @MainActor in
                _ = await executor?.perform([
                    "type": "persist_record",
                    "record": [
                        "record_id": "dapp-1-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
                        "params_json": "[]", "result": "", "from": "", "chain_id": 100, "now_ms": 0,
                        "status": "pending", "user_op_hash": hash, "dapp_origin": "",
                        "maybe_sent": true, "submit_block": block.map { $0 as Any } ?? NSNull(),
                    ] as [String: Any],
                ])
                _ = await executor?.perform(["type": "clear_to_post", "id": id, "user_op_hash": hash])
            }
        }
        executor.ports.opSubmitted = { [weak executor] id, hash, maybeSent, block in
            recorded.submitted.append((hash, maybeSent, block))
            // The core persists the record on `op_submitted`; here the test
            // stands in for it, so the answer's wait for the record is short.
            Task { @MainActor in
                _ = await executor?.perform([
                    "type": "persist_record",
                    "record": [
                        "record_id": "dapp-1-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
                        "params_json": "[]", "result": "", "from": "", "chain_id": 100, "now_ms": 0,
                        "status": "pending", "user_op_hash": hash, "dapp_origin": "",
                        "maybe_sent": maybeSent, "submit_block": block.map { $0 as Any } ?? NSNull(),
                    ] as [String: Any],
                ])
                _ = id
            }
        }
        executor.ports.signingStarted = { recorded.ceremony.append("started:\($0)") }
        executor.ports.ceremonyDone = { recorded.ceremony.append("done:\($0)") }
        return (executor, recorded)
    }

    private func signAndSubmit() -> [String: Any] {
        [
            "type": "sign_and_submit", "id": "req-1", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"\#(fixture.account)","value":"0x3e8"}]"#,
            "chain_id": 100, "address": fixture.account, "credential_id": "",
            "max_fee_per_gas": NSNull(), "gas_fee_token": NSNull(),
            "quoted_fee": ["amount": "1000", "recipient": fixture.account] as [String: Any],
        ]
    }

    /// NotSent answers the page -32603 with the core's fixed detail — never
    /// the pool's text (RA10).
    @Test func aNotSentPageGetsTheFixedDetail() async throws {
        let port = readyPort()
        port.detailed["eth_sendUserOperation"] = [answer(.failed(rateLimited: false), delivered: false)]
        let (executor, recorded) = signExecutor(port)
        let reply = try CoreJSON.object(await executor.perform(signAndSubmit()))
        let outcome = try #require(reply["outcome"] as? [String: Any])
        #expect(outcome["type"] as? String == "failed")
        #expect(outcome["message"] as? String == userOpNotSentDetail())
        #expect(outcome["refused"] as? Bool == false, "a relay never reached is not a refusal")
        #expect(userOpNotSentDetail() == "relay unreachable; nothing was sent")
        #expect(recorded.submitted.isEmpty, "nothing was submitted, nothing is tracked")
        #expect(recorded.ceremony == ["started:req-1", "done:req-1"], "the prompt is bracketed for the core (RA9)")
    }

    /// RJ1 through the executor: the op is handed to the core before any
    /// POST, and with no clearance the page is told nothing was sent — and
    /// nothing was: zero POSTs.
    @Test func noClearanceSendsNothingAndSaysSo() async throws {
        let port = readyPort()
        port.rpc["eth_sendUserOperation"] = .ok(relayHash)
        let (executor, recorded) = signExecutor(port, clears: false, clearanceWaitMs: 100)
        let reply = try CoreJSON.object(await executor.perform(signAndSubmit()))
        let outcome = try #require(reply["outcome"] as? [String: Any])
        #expect(outcome["type"] as? String == "failed")
        #expect(outcome["message"] as? String == userOpNotSentDetail())
        #expect(outcome["refused"] as? Bool == false)
        let signed = try #require(recorded.signed.first, "op_signed went to the core")
        #expect(signed.hash.count == 66)
        #expect(signed.block == 0x2e3b9d9)
        #expect(!signed.postedBefore)
        #expect(!port.calls.contains("eth_sendUserOperation"), "no clearance, no POST")
        #expect(recorded.submitted.isEmpty)
    }

    /// RJ1: with the clearance, the op is POSTed after `op_signed` and never
    /// before it.
    @Test func theClearedOpIsPostedAfterItsRecord() async throws {
        let port = readyPort()
        port.rpc["eth_sendUserOperation"] = .ok(relayHash)
        let (executor, recorded) = signExecutor(port)
        let reply = try CoreJSON.object(await executor.perform(signAndSubmit()))
        let outcome = try #require(reply["outcome"] as? [String: Any])
        #expect(outcome["type"] as? String == "receipt_pending")
        let signed = try #require(recorded.signed.first)
        #expect(!signed.postedBefore, "op_signed precedes the POST")
        #expect(port.calls.filter { $0 == "eth_sendUserOperation" }.count == 1)
        #expect(recorded.submitted.first?.hash == relayHash, "the relay's hash wins")
    }

    /// RJ3: the relay refused the op — `failed{refused: true}`, whatever its
    /// sentence, so the page hears the core's "refused" and the sheet never
    /// says "try again". "Relayer unavailable" is not a refusal.
    @Test func aRelayRefusalIsRefused() async throws {
        let port = readyPort()
        port.detailed["eth_sendUserOperation"] = [
            answer(.rpcError(code: -32500, message: "AA23 reverted: ERC20: transfer amount exceeds balance"),
                   delivered: false),
        ]
        let (executor, _) = signExecutor(port)
        let reply = try CoreJSON.object(await executor.perform(signAndSubmit()))
        let outcome = try #require(reply["outcome"] as? [String: Any])
        #expect(outcome["type"] as? String == "failed")
        #expect(outcome["refused"] as? Bool == true)
        #expect(userOpRefusedDappDetail() == "the network refused this transaction; nothing was sent")
    }

    /// MaybeSent reaches the core as `op_submitted{maybe_sent}` under the
    /// local hash, and the page gets ONE answer: a hash, never an error.
    @Test func aMaybeSentOpIsReportedAndThePageGetsAHash() async throws {
        let port = readyPort()
        port.detailed["eth_sendUserOperation"] = [answer(.failed(rateLimited: false), delivered: true)]
        let (executor, recorded) = signExecutor(port)
        let reply = try CoreJSON.object(await executor.perform(signAndSubmit()))
        let outcome = try #require(reply["outcome"] as? [String: Any])
        let submitted = try #require(recorded.submitted.first)
        #expect(submitted.maybeSent)
        #expect(submitted.block == 0x2e3b9d9)
        #expect(outcome["type"] as? String == "receipt_pending")
        #expect(outcome["user_op_hash"] as? String == submitted.hash)
    }

    /// The row a dApp request writes keeps what the submit could not prove
    /// (T183), and a batch names its first leg and the sum of its values.
    @Test func theRecordKeepsMaybeSentAndABatchSumsItsLegs() {
        let row = SignExecutor.recordRow([
            "record_id": "dapp-9-tx", "kind": "dapp_tx", "method": "wallet_sendCalls",
            "params_json": #"[{"calls":[{"to":"0xAa00000000000000000000000000000000000001","value":"0x3e8"},{"to":"0xbb","value":"1000"},{"to":"0xcc","value":"nonsense"}]}]"#,
            "result": "", "from": "0x1", "chain_id": 100, "now_ms": 1_000,
            "status": "pending", "user_op_hash": local, "dapp_origin": "http://127.0.0.1:8137",
            "maybe_sent": true, "submit_block": 48_478_681,
        ], nativeSymbol: "XDAI")
        #expect(row["to"] as? String == "0xAa00000000000000000000000000000000000001")
        #expect(row["value"] as? String == "0x7d0", "0x3e8 + 1000 = 2000")
        #expect(row["maybeSent"] as? Bool == true)
        #expect((row["submitBlock"] as? NSNumber)?.uint64Value == 48_478_681)
        let plain = SignExecutor.recordRow([
            "record_id": "dapp-9-tx", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xbb","value":"0x5"}]"#, "user_op_hash": local, "chain_id": 100,
        ], nativeSymbol: "XDAI")
        #expect(plain["value"] as? String == "0x5")
        #expect(plain["maybeSent"] == nil, "an ordinary row is what it always was")
        #expect(plain["submitBlock"] == nil)
    }
}

@MainActor
final class Recorded {
    var submitted: [(hash: String, maybeSent: Bool, block: UInt64?)] = []
    var ceremony: [String] = []
    /// `op_signed` as the core heard it, and whether a POST had gone first.
    var signed: [(hash: String, block: UInt64?, postedBefore: Bool)] = []
}
