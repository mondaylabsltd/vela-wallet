//
//  SigningFollowsTrackerTests.swift
//  VelaWalletTests
//
//  Spec 082 round 2 (T237, T238, RJ1, RJ3, RJ4), reviewed through the REAL
//  controller — its own executor, its own `sign_request` core, the real
//  spine signing on the Trusted Signer fixture, a scripted relay that quotes
//  a fee. What the executor-level tests cannot see is the controller's glue:
//  which hand-offs reach the tracker and when, what reaches the core as
//  `op_tracked`, and how many answers the page gets.
//
//  - The page's answer follows the tracker's verdict at once (its tx hash),
//    exactly once, and the receipt wait still running neither answers again
//    nor keeps asking the relay. A "refused" verdict stays on the sheet and
//    is answered on its close (spec 097 N4).
//  - A "not sent" the tracker reached while the POST was still out never
//    answers an op the relay then accepted.
//  - A page that left after the write-ahead gets nothing POSTed: the record
//    goes and the tracker lets the op go.
//  - A relay that answers another hash: the write-ahead op is withdrawn and
//    the relay's op is handed over once its own record is on disk.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct SigningFollowsTrackerTests {

    private let fixture = TrustedSignerFixture()
    private let feeRecipient = "0x7777777777777777777777777777777777777777"
    private let payee = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    private let tx = "0x" + String(repeating: "ab", count: 32)

    private static let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// What the sheet's receipt is drawn against — the page's chain, the account.
    private static func receiptContext() -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894"
        )
    }

    /// What one run saw.
    @MainActor
    final class Seen {
        var answers: [[String: Any]] = []
        var tracked: [TrackSubmission] = []
        var withdrawn: [(hash: String, ids: [String])] = []
        var controller: SigningController?
    }

    struct Run {
        let controller: SigningController
        let seen: Seen
        let scripted: ScriptedRelayPort
        let port: WriteAheadProbePort
        let store: VelaStore
    }

    /// A controller on a scripted world that quotes a fee and never has a
    /// receipt. `post` answers the submit POST, given the store as the POST
    /// goes out; `handedOff` sees each hand-off to the tracker as it happens.
    private func approved(
        post: @escaping (_ store: VelaStore, _ seen: Seen) -> RpcCallResult,
        handedOff: @escaping (_ submission: TrackSubmission, _ seen: Seen) -> Void = { _, _ in }
    ) async throws -> Run {
        let scripted = ScriptedRelayPort()
        scripted.rpc["eth_getCode"] = .ok("0x")
        scripted.rpc["eth_blockNumber"] = .ok("0x2e3b9d9")
        scripted.rpc["eth_gasPrice"] = .ok("0x3b9aca00")
        scripted.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x3b9aca00"] as [String: Any])
        scripted.rpc["eth_maxPriorityFeePerGas"] = .ok("0x1")
        let tier: [String: Any] = [
            "maxFeePerGas": "0x77359400", "maxPriorityFeePerGas": "0x1",
            "networkFeePerGas": "0x3b9aca00", "relayerFeePerGas": "0x3b9aca00",
        ]
        scripted.rpc["pimlico_getUserOperationGasPrice"] = .ok(
            ["slow": tier, "standard": tier, "fast": tier] as [String: Any]
        )
        scripted.rpc["vela_getInBandGasQuote"] = .ok([[
            "recipient": feeRecipient, "asset": "native", "feeToken": NSNull(), "decimals": 18,
            "symbol": "XDAI", "balance": "0xde0b6b3a7640000", "usdPrice": "1", "usdBalance": "1",
        ] as [String: Any]])
        scripted.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        // No receipt for as long as the test runs.
        scripted.rpc["eth_getUserOperationReceipt"] = .ok(NSNull())

        let port = WriteAheadProbePort(scripted)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.recordJson = fixture.recordJson(signedInWith: UserOpSpine.trustedSignerMethod)
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() })
        let fixture = self.fixture
        spine.trustedSigner = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let seen = Seen()
        port.onPost = { post(store, seen) }
        let controller = SigningController(
            wallet: (address: fixture.account, credentialId: fixture.credentialHex),
            relay: relay, accounts: accounts, spine: spine, store: store,
            pool: RpcPool(store: store, accounts: AccountStore(defaults: defaults)),
            ports: SigningController.Ports(
                respond: { _, _, payload, _ in seen.answers.append(payload) },
                trackSubmitted: { submission in
                    seen.tracked.append(submission)
                    handedOff(submission, seen)
                },
                trackWithdrawn: { hash, ids in seen.withdrawn.append((hash, ids)) },
                knownChains: { [100] }
            )
        )
        seen.controller = controller
        controller.open(SigningController.Incoming(
            id: "req-1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"\#(payee)","value":"0x3e8"}]"#,
            origin: "https://app.example", transportId: "tab-1", chainId: 100,
            grantedAddress: fixture.account
        ))
        await Wait.until { controller.fee?.fee?.feeRecipient != nil && controller.fee?.busy == false }
        controller.approve()
        return Run(controller: controller, seen: seen, scripted: scripted, port: port, store: store)
    }

    /// The relay takes the op under its own hash, read back from the record
    /// written ahead.
    private static func accept(_ store: VelaStore) -> RpcCallResult {
        let hash = TxRecords.load(store: store).first?["userOpHash"] as? String ?? ""
        return RpcCallResult(outcome: .ok(hash), maybeDelivered: false, heldErrorJson: nil)
    }

    private func view(_ hash: String, status: String, txHash: String?) -> TrackViewWire {
        TrackViewWire(entries: [TrackEntryWire(
            userOpHash: hash, chainId: 100, recordIds: [], status: status, txHash: txHash,
            polling: false, submittedAtMs: 1_757_000_000_000, outcome: "final"
        )])
    }

    private func receiptPolls(_ run: Run) -> Int {
        run.scripted.calls.filter { $0 == "eth_getUserOperationReceipt" }.count
    }

    /// The tracker found the op's transaction: the page gets that tx hash at
    /// once, not at the end of the two-minute window — and only once.
    @Test func aConfirmedVerdictAnswersThePageOnceWithItsTxHash() async throws {
        let run = try await approved(post: { store, _ in Self.accept(store) })
        await Wait.until { run.seen.tracked.contains { $0.admitted } }
        let hash = try #require(run.controller.submittedUserOp)
        #expect(run.seen.answers.isEmpty, "nothing answered before the tracker's verdict")

        run.controller.trackerChanged(view(hash, status: "pending", txHash: nil))
        try await Task.sleep(nanoseconds: 100_000_000)
        #expect(run.seen.answers.isEmpty, "a live entry answers nothing")

        run.controller.trackerChanged(view(hash, status: "confirmed", txHash: tx))
        await Wait.until { !run.seen.answers.isEmpty }
        let answer = try #require(run.seen.answers.first)
        #expect(answer["type"] as? String == "ok")
        #expect(answer["result"] as? String == tx)
        #expect(run.controller.hasAnswered)

        // The receipt wait ends with the answer: no second answer, and the
        // relay is not asked for the receipt again.
        let polls = receiptPolls(run)
        try await Task.sleep(nanoseconds: 3_500_000_000)
        #expect(run.seen.answers.count == 1, "exactly one answer: \(run.seen.answers)")
        #expect(receiptPolls(run) == polls, "the receipt wait went on asking after the page was answered")
        run.controller.trackerChanged(view(hash, status: "confirmed", txHash: tx))
        #expect(run.seen.answers.count == 1)
    }

    /// The relay refused the op after taking it (DX-W3): the page gets the
    /// core's -32603 "refused" once, and the sheet says refused.
    ///
    /// Spec 097 N4: after "Submitted" the refusal stays on the sheet — the
    /// receipt says Failed and "refused", Done and no Try again — and the
    /// page hears it when the person closes the sheet, not under the words.
    @Test func aRejectedVerdictAnswersRefusedOnce() async throws {
        let run = try await approved(post: { store, _ in Self.accept(store) })
        await Wait.until { run.seen.tracked.contains { $0.admitted } }
        let hash = try #require(run.controller.submittedUserOp)
        await Wait.until { run.controller.sign.pendingOpHash != nil }

        run.controller.trackerChanged(view(hash, status: "rejected", txHash: nil))
        await Wait.until { run.controller.sign.failureRefused }
        let sign = run.controller.sign
        #expect(sign.isVisible, "the sheet stays")
        #expect(!sign.failureRetryable, "the rid went out: no Try again")
        #expect(sign.pendingOpHash == nil, "no longer submitted")
        let receipt = try #require(SigningLive.receipt(
            sign: sign, blocks: [], context: Self.receiptContext()
        ))
        #expect(receipt.stage == .failed)
        #expect(receipt.captions.contains(Self.loc.t("componentsUi.signing.refused")))
        #expect(receipt.retry == nil)
        #expect(receipt.cta == Self.loc.t("componentsTx.receipt.done"))
        try await Task.sleep(nanoseconds: 500_000_000)
        #expect(run.seen.answers.isEmpty, "held while it shows: \(run.seen.answers)")
        #expect(!run.controller.hasAnswered)

        run.controller.swipeDismissed()
        await Wait.until { !run.seen.answers.isEmpty }
        let answer = try #require(run.seen.answers.first)
        #expect(answer["type"] as? String == "err")
        #expect((answer["code"] as? NSNumber)?.intValue == -32603)
        #expect(answer["message"] as? String == userOpRefusedDappDetail())
        #expect(run.controller.hasAnswered)

        try await Task.sleep(nanoseconds: 500_000_000)
        #expect(run.seen.answers.count == 1, "exactly one answer: \(run.seen.answers)")
    }

    /// Spec 097 N4: the page went while the refusal showed. The browser
    /// answered it (4900); the core drops the held answer — and the
    /// controller is no longer "committed", so it is not kept, unanswerable,
    /// among the retired ones.
    @Test func aPageGoneUnderTheRefusalLeavesNothingCommitted() async throws {
        let run = try await approved(post: { store, _ in Self.accept(store) })
        await Wait.until { run.seen.tracked.contains { $0.admitted } }
        let hash = try #require(run.controller.submittedUserOp)
        await Wait.until { run.controller.sign.pendingOpHash != nil }
        #expect(run.controller.committed, "on its way")
        run.controller.trackerChanged(view(hash, status: "rejected", txHash: nil))
        await Wait.until { run.controller.sign.failureRefused }
        #expect(!run.controller.committed, "a held failure ends the pipeline")
        run.controller.transportDropped()
        await Wait.until { !run.controller.sign.isVisible }
        try await Task.sleep(nanoseconds: 300_000_000)
        #expect(run.seen.answers.isEmpty, "the page is gone; nothing more is said")
    }

    /// The tracker reached "not sent" while the POST was still out (the op
    /// was handed over before it); then the relay took it. That stale verdict
    /// never answers the op — the page waits for the real one.
    @Test func aStaleNotSentNeverAnswersAnAcceptedOp() async throws {
        let run = try await approved(post: { store, seen in
            let hash = TxRecords.load(store: store).first?["userOpHash"] as? String ?? ""
            seen.controller?.trackerChanged(TrackViewWire(entries: [TrackEntryWire(
                userOpHash: hash, chainId: 100, recordIds: [], status: "not_sent", txHash: nil,
                polling: false, submittedAtMs: 1_757_000_000_000, outcome: "final"
            )]))
            return RpcCallResult(outcome: .ok(hash), maybeDelivered: false, heldErrorJson: nil)
        })
        await Wait.until { run.seen.tracked.contains { $0.admitted } }
        let hash = try #require(run.controller.submittedUserOp)
        try await Task.sleep(nanoseconds: 300_000_000)
        #expect(run.seen.answers.isEmpty, "a stale 'not sent' answered an accepted op: \(run.seen.answers)")

        run.controller.trackerChanged(view(hash, status: "confirmed", txHash: tx))
        await Wait.until { !run.seen.answers.isEmpty }
        #expect(run.seen.answers.first?["result"] as? String == tx)
        #expect(run.seen.answers.count == 1)
    }

    /// The op landed while its POST was still retrying: the tracker already
    /// has the tx hash when the relay answers, and the page gets it at once.
    @Test func aVerdictBeforeTheRelaysAnswerIsAnsweredAtOnce() async throws {
        let tx = self.tx
        let run = try await approved(post: { store, seen in
            let hash = TxRecords.load(store: store).first?["userOpHash"] as? String ?? ""
            seen.controller?.trackerChanged(TrackViewWire(entries: [TrackEntryWire(
                userOpHash: hash, chainId: 100, recordIds: [], status: "confirmed", txHash: tx,
                polling: false, submittedAtMs: 1_757_000_000_000, outcome: "final"
            )]))
            return RpcCallResult(outcome: .ok(hash), maybeDelivered: false, heldErrorJson: nil)
        })
        await Wait.until { !run.seen.answers.isEmpty }
        #expect(run.seen.answers.first?["result"] as? String == tx)
        #expect(receiptPolls(run) <= 1, "the page waited on the relay's receipt")
        try await Task.sleep(nanoseconds: 500_000_000)
        #expect(run.seen.answers.count == 1)
    }

    /// The page left after the write-ahead (its record is on disk, the op
    /// handed over) and before the POST: nothing is POSTed, the record goes,
    /// the tracker lets the op go, and nobody is answered.
    @Test func anAskerGoneAfterTheWriteAheadSendsNothingAndLeavesNoRecord() async throws {
        let run = try await approved(
            post: { store, _ in Self.accept(store) },
            handedOff: { submission, seen in
                // The write-ahead hand-off: the record is on disk, and the
                // clearance has not been given yet.
                if submission.maybeSent, !submission.admitted { seen.controller?.transportDropped() }
            }
        )
        await Wait.until { !run.seen.withdrawn.isEmpty }
        let ahead = try #require(run.seen.tracked.first)
        #expect(ahead.maybeSent && !ahead.admitted)
        #expect(run.port.posts == 0, "a POST for a page that had gone")
        #expect(run.seen.withdrawn.first?.hash.lowercased() == ahead.userOpHash.lowercased())
        // The write-ahead's hand-off names no record; the withdrawal names the
        // written-ahead record it takes back (082 second review).
        #expect(ahead.recordIds.isEmpty)
        #expect(run.seen.withdrawn.first?.ids.isEmpty == false)
        await Wait.until { TxRecords.load(store: run.store).isEmpty }
        #expect(TxRecords.load(store: run.store).isEmpty, "a record for an op never sent")
        try await Task.sleep(nanoseconds: 300_000_000)
        #expect(run.seen.answers.isEmpty, "the page is gone; nothing is answered")
    }

    /// The relay refuses the POST itself (RJ3): nothing was sent. The page
    /// gets the core's -32603 "refused" once, the record written ahead goes,
    /// the tracker lets the op go, and the sheet says refused.
    @Test func aRelayRefusalAtTheSubmitAnswersRefusedAndLeavesNoRecord() async throws {
        let run = try await approved(post: { _, _ in
            RpcCallResult(
                outcome: .rpcError(code: -32500, message: "AA23 reverted: transfer amount exceeds balance"),
                maybeDelivered: false, heldErrorJson: nil
            )
        })
        // Spec 096 F8: the sheet says refused, and the page hears it on the
        // close — the window is not taken away over the words.
        await Wait.until { run.controller.sign.failureRefused }
        #expect(run.seen.answers.isEmpty, "held while the sheet shows it")
        #expect(!run.controller.sign.failureRetryable, "a refusal is not tried again")
        run.controller.swipeDismissed()
        await Wait.until { !run.seen.answers.isEmpty }
        let answer = try #require(run.seen.answers.first)
        #expect(answer["type"] as? String == "err")
        #expect((answer["code"] as? NSNumber)?.intValue == -32603)
        #expect(answer["message"] as? String == userOpRefusedDappDetail(),
                "the relay's own sentence reached the page: \(answer)")
        await Wait.until { !run.seen.withdrawn.isEmpty }
        #expect(run.seen.withdrawn.first?.hash.lowercased() == run.seen.tracked.first?.userOpHash.lowercased())
        await Wait.until { TxRecords.load(store: run.store).isEmpty }
        #expect(!run.controller.sign.isVisible, "closed by the person, after the words")
        #expect(run.controller.submittedUserOp == nil)
        try await Task.sleep(nanoseconds: 300_000_000)
        #expect(run.seen.answers.count == 1)
        #expect(run.port.posts == 1)
    }

    /// The ✕ after the confirm refuses nothing (spec 079): the op written ahead
    /// is still POSTed, and the page still gets its one answer from the
    /// tracker's verdict.
    @Test func aCloseDuringTheWriteAheadStillSendsAndAnswersOnce() async throws {
        let run = try await approved(
            post: { store, _ in Self.accept(store) },
            handedOff: { submission, seen in
                if submission.maybeSent, !submission.admitted { seen.controller?.swipeDismissed() }
            }
        )
        await Wait.until { run.seen.tracked.contains { $0.admitted } }
        #expect(run.port.posts == 1)
        let hash = try #require(run.controller.submittedUserOp)
        #expect(run.seen.answers.isEmpty, "a close after the confirm is not a refusal: \(run.seen.answers)")
        run.controller.trackerChanged(view(hash, status: "confirmed", txHash: tx))
        await Wait.until { !run.seen.answers.isEmpty }
        #expect(run.seen.answers.first?["result"] as? String == tx)
        try await Task.sleep(nanoseconds: 300_000_000)
        #expect(run.seen.answers.count == 1)
    }

    /// The relay answers another hash (`userop.hash_mismatch`): the
    /// write-ahead op is withdrawn and the relay's is handed over — after
    /// its OWN record is on disk, and exactly once.
    @Test func aRelayHashThatIsNotOursHandsOverTheRelaysOpOnItsOwnRecord() async throws {
        let other = "0x" + String(repeating: "cd", count: 32)
        let run = try await approved(post: { _, _ in
            RpcCallResult(outcome: .ok(other), maybeDelivered: false, heldErrorJson: nil)
        })
        await Wait.until { run.seen.tracked.contains { $0.userOpHash == other } }
        let ahead = try #require(run.seen.tracked.first)
        #expect(ahead.userOpHash.lowercased() != other)
        #expect(run.seen.withdrawn.map { $0.hash.lowercased() } == [ahead.userOpHash.lowercased()])
        let relays = run.seen.tracked.filter { $0.userOpHash == other }
        #expect(relays.count == 1, "the relay's op handed over once: \(relays)")
        let handed = try #require(relays.first)
        #expect(handed.recordIds != ahead.recordIds, "the relay's op has its own record")
        let rows = TxRecords.load(store: run.store)
        #expect(rows.count == 1, "one record, the relay's: \(rows)")
        #expect((rows.first?["userOpHash"] as? String) == other)
        #expect(handed.recordIds == [rows.first?["id"] as? String ?? ""])
    }
}
