//
//  Probes083ParityTests.swift
//  VelaWalletTests
//
//  Spec 084 CHECK PASS (not a fix): probes that MEASURE what iOS does today for
//  the 083 findings. Nothing here changes production code. Each probe writes
//  what it OBSERVED to a log line ("PROBE083|<id>|...") and asserts only the
//  precondition ("the run reached the step it claims to measure"), so the suite
//  stays green while the verdicts are read from the log against the hand-off's
//  GOOD/BAD text.
//
//  The verdict lines go to /private/tmp/.../scratchpad/probe083-ios.log (the
//  simulator shares the Mac's filesystem) and to stdout.
//

import Foundation
import SwiftUI
import Testing
import Network
import VelaCore
@testable import VelaWallet

private enum Probe {
    static let path = "/private/tmp/claude-501/-Volumes-data-production-agent-2-vela-wallet/7177859b-46e8-4e29-be2f-0104b6530a2c/scratchpad/probe083-ios.log"
    static func log(_ id: String, _ text: String) {
        let line = "PROBE083|\(id)|\(text)\n"
        print(line, terminator: "")
        if let data = line.data(using: .utf8) {
            if let handle = FileHandle(forWritingAtPath: path) {
                handle.seekToEndOfFile(); handle.write(data); try? handle.close()
            } else {
                FileManager.default.createFile(atPath: path, contents: data)
            }
        }
    }
}

private let golden083 = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
private let limits083: [String: Any] = [
    "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
]

/// Every fixture key on the Safe, the first pinned (`ParallelSpaceBinding.enter`).
@MainActor
private func fixtureAccounts083() -> ScriptedAccounts {
    let accounts = ScriptedAccounts()
    if let fixtures = try? fixtureAccounts() {
        accounts.keyList = fixtures.map {
            WalletKeyRecord(credentialId: $0.credentialIdHex, publicKeyHex: $0.publicKeyHex)
        }
    }
    return accounts
}

@MainActor
private func spine083(_ port: ScriptedRelayPort) -> (UserOpSpine, RelayClient) {
    let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
    let spine = UserOpSpine(
        relay: relay, accounts: fixtureAccounts083(),
        signer: { FixtureUserOpSigner(preferred: nil) }
    )
    return (spine, relay)
}

@MainActor
struct Probes083Bytes {

    // MARK: - I-S3b : another operation's hash answered

    @Test func iS3b_existingHashMarkerIsReturnedAsThisOpsHash() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        port.rpc["eth_estimateUserOperationGas"] = .ok(limits083)
        let other = "0x" + String(repeating: "11", count: 32)
        port.rpc["eth_sendUserOperation"] = .rpcError(
            code: -32521, message: "AA25 invalid account nonce [existingHash:\(other)]"
        )
        let (spine, _) = spine083(port)
        var outcome = "?"
        do {
            let hash = try await spine.submit(
                chainId: 100, account: golden083,
                calls: [UserOpCall(to: golden083, value: "1000000000000000", data: "0x")],
                gasFeeToken: nil,
                quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: golden083)
            )
            outcome = "RETURNED hash=\(hash) (the marker's hash=\(other)) equalsOther=\(hash.lowercased() == other)"
        } catch {
            outcome = "THREW \(error)"
        }
        Probe.log("I-S3b", "calls=\(port.calls) -> \(outcome)")
        #expect(port.calls.contains("eth_sendUserOperation"), "a run that never sent measured nothing")
    }

    // MARK: - I-S2 / I-S3 : the receipt wait

    private func signOp(_ id: String) -> [String: Any] {
        [
            "type": "sign_and_submit", "id": id, "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141","value":"0x0","data":"0x1234"}]"#,
            "chain_id": 100, "address": golden083,
            "quoted_fee": ["amount": "1000", "recipient": golden083],
        ]
    }

    private func outcomeOf(_ json: String) -> String {
        guard let object = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any],
              let outcome = object["outcome"] as? [String: Any]
        else { return "unparsed: \(json)" }
        return String(data: (try? JSONSerialization.data(withJSONObject: outcome, options: [.sortedKeys])) ?? Data(), encoding: .utf8) ?? "?"
    }

    private func persist(_ executor: SignExecutor, hash: String) async {
        _ = await executor.perform([
            "type": "persist_record",
            "record": [
                "record_id": "dapp-probe", "kind": "dapp_tx", "method": "eth_sendTransaction",
                "params_json": "[]", "result": "", "from": golden083, "chain_id": 100,
                "now_ms": 1_757_000_000_000, "status": "pending", "user_op_hash": hash,
                "dapp_origin": "https://probe.test",
            ] as [String: Any],
        ])
    }

    @Test func iS2_aRevertedReceiptIsAnsweredAsSucceeded() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        port.rpc["eth_estimateUserOperationGas"] = .ok(limits083)
        port.rpc["eth_sendUserOperation"] = .ok("0xop")
        port.rpc["eth_getUserOperationReceipt"] = .ok([
            "success": false,
            "receipt": ["transactionHash": "0xabc", "logs": [Any]()] as [String: Any],
        ] as [String: Any])
        let (spine, relay) = spine083(port)
        let store = VelaStore(defaults: UserDefaults(suiteName: "probe083.s2.\(UUID().uuidString)")!)
        let executor = SignExecutor(
            spine: spine, relay: relay, store: store, receiptWaitMs: 2_000, receiptPollMs: 100
        )
        await persist(executor, hash: "0xop")
        let answer = await executor.perform(signOp("r1"))
        Probe.log("I-S2", "calls=\(port.calls) -> outcome=\(outcomeOf(answer))")
        #expect(port.calls.contains("eth_sendUserOperation"))
    }

    @Test func iS3_aLateReceiptAnswersTheOpHash() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        port.rpc["eth_estimateUserOperationGas"] = .ok(limits083)
        port.rpc["eth_sendUserOperation"] = .ok("0xop")
        port.rpc["eth_getUserOperationReceipt"] = .ok(NSNull())
        let (spine, relay) = spine083(port)
        let store = VelaStore(defaults: UserDefaults(suiteName: "probe083.s3.\(UUID().uuidString)")!)
        let executor = SignExecutor(
            spine: spine, relay: relay, store: store, receiptWaitMs: 300, receiptPollMs: 50
        )
        await persist(executor, hash: "0xop")
        let started = Date()
        let answer = await executor.perform(signOp("r2"))
        Probe.log("I-S3", "calls=\(port.calls.filter { $0 != "eth_getUserOperationReceipt" }) receiptPolls=\(port.calls.filter { $0 == "eth_getUserOperationReceipt" }.count) elapsed=\(Int(Date().timeIntervalSince(started) * 1000))ms (window 300ms) -> outcome=\(outcomeOf(answer))")
        #expect(port.calls.contains("eth_sendUserOperation"))
    }

    /// The batch path answers the same way (I-B5792): a bare string, not `{id}`.
    @Test func iB5792_aBatchIsAnsweredLikeATransaction() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        port.rpc["eth_estimateUserOperationGas"] = .ok(limits083)
        port.rpc["eth_sendUserOperation"] = .ok("0xop")
        port.rpc["eth_getUserOperationReceipt"] = .ok([
            "success": true,
            "receipt": ["transactionHash": "0xdef", "logs": [Any]()] as [String: Any],
        ] as [String: Any])
        let (spine, relay) = spine083(port)
        let store = VelaStore(defaults: UserDefaults(suiteName: "probe083.b5792.\(UUID().uuidString)")!)
        let executor = SignExecutor(
            spine: spine, relay: relay, store: store, receiptWaitMs: 2_000, receiptPollMs: 100
        )
        await persist(executor, hash: "0xop")
        var op = signOp("r3")
        op["method"] = "wallet_sendCalls"
        op["params_json"] = #"[{"version":"2.0.0","from":"\#(golden083)","chainId":"0x64","calls":[{"to":"\#(golden083)","value":"0x0"}]}]"#
        let answer = await executor.perform(op)
        Probe.log("I-B5792", "calls=\(port.calls) -> outcome=\(outcomeOf(answer))")
        #expect(port.calls.contains("eth_sendUserOperation"))
    }

    // MARK: - I-R3, I-H4a : the receipt the person reads

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let opHash = "0xace642c724834405c56919fb198d6a6389cc2867ff3f3e3585b59d1921fe681d"
    private let txHash = "0x09c35478682ddce770c02e4aed27c9f0260f7b2f54ae6a7a194f50b70d604c9f"

    private func context(track: TrackEntryWire? = nil, explorer: String? = "https://gnosisscan.io") -> SigningLive.Context {
        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
            walletName: "Me", walletAddress: golden083.lowercased()
        )
        context.typicalS = 5
        context.track = track
        context.explorerBase = explorer
        return context
    }

    private func sign(
        error: SignErrorKind? = nil, detail: String? = nil, kind: SignMethodKind = .transaction
    ) -> SignViewWire {
        SignViewWire(
            surface: .sheet,
            request: SignRequestViewWire(
                id: "r1", method: kind == .personalSign ? "personal_sign" : "eth_sendTransaction",
                kind: kind, paramsJson: "[]", origin: "http://127.0.0.1:8137", dapp: nil,
                chainId: 100, signerAddress: nil
            ),
            isSigning: false, isSubmitting: false, pendingOpHash: nil,
            error: error.map { SignErrorNoticeWire(kind: $0, detail: detail) },
            funding: nil, confirmGateOpen: true, reconcilePending: false, swipeAction: .reject,
            trackerHandoff: nil, notice: nil, globalChainId: 100, blocked: nil
        )
    }

    private let blocks: [SigningBlock] = [
        .intent(text: "Send", tone: .neutral),
        .amount(line: AmountLine(sign: "−", value: "0.001", symbol: "XDAI")),
    ]

    private func describe(_ model: SendReceiptModel?) -> String {
        guard let model else { return "nil" }
        return "stage=\(model.stage) title=«\(model.title)» captions=\(model.captions) hash=\(model.hash?.copyValue ?? "nil") explorer=\(model.viewOnExplorer ?? "nil") cta=«\(model.cta)»"
    }

    @Test func iR3_aRevertReadsFundsSafe() {
        let dropped = TrackEntryWire(
            userOpHash: opHash, chainId: 100, recordIds: ["rec-1"], status: "dropped", txHash: txHash,
            polling: false, submittedAtMs: 1, outcome: "final"
        )
        let a = SigningLive.aftercareReceipt(
            .stillConfirming(chainId: 100, userOpHash: opHash), summary: "发送 · −0.001 XDAI",
            context: context(track: dropped)
        )
        Probe.log("I-R3a", describe(a))
        Probe.log("I-R3a", "expected-GOOD failedHint=«\(loc.t("componentsTx.receipt.failedHint"))»  generic=«\(loc.t("send.txErrorGeneric"))»")
        let b = SigningLive.receipt(
            sign: sign(error: .submitFailed, detail: "The transaction was included but reverted (0xabc)"),
            blocks: blocks, context: context()
        )
        Probe.log("I-R3b", describe(b))
        #expect(a.stage == .failed && b != nil)
    }

    @Test func iH4a_aFailedMessageShowsTheTransactionSentence() {
        let model = SigningLive.receipt(
            sign: sign(error: .submitFailed, detail: "eth_signTypedData_v4 carried nothing this wallet could sign", kind: .personalSign),
            blocks: blocks, context: context()
        )
        Probe.log("I-H4a", describe(model))
        Probe.log("I-H4a", "expected-GOOD offChainNote=«\(loc.t("connect.detail.offChainNote"))»")
        #expect(model != nil)
    }

    // MARK: - I-1 .. I-4 : what a page can put in a request

    @Test func i1_aStrayCallsKeyLeadsTheSheet() {
        let params = #"[{"from":"\#(golden083)","to":"0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83","value":"0x0","data":"0x095ea7b3","calls":[{"to":"0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141","value":"0x1"}]}]"#
        let kick = SigningController.clearKickoff(method: "eth_sendTransaction", paramsJson: params, chainId: 100, origin: nil)
        let signed = SignExecutor.callsOf(method: "eth_sendTransaction", paramsJson: params)
        Probe.log("I-1", "clearKickoff to=\(kick?["to"] ?? "?") value=\(kick?["value"] ?? "?") data=\(kick?["data"] ?? "?") | SIGNED calls=\(signed?.map { "\($0.to) v=\($0.value) d=\($0.data)" } ?? [])")
        #expect(kick != nil && signed != nil)
    }

    @Test func i2_aMultiCallBatchLeadsWithItsFirstLeg() {
        let params = #"[{"version":"2.0.0","calls":[{"to":"0x000000000000000000000000000000000000dEaD","value":"0x1"},{"to":"0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83","data":"0xa9059cbb"}]}]"#
        let kick = SigningController.clearKickoff(method: "wallet_sendCalls", paramsJson: params, chainId: 100, origin: nil)
        let signed = SignExecutor.callsOf(method: "wallet_sendCalls", paramsJson: params)
        Probe.log("I-2", "clearKickoff to=\(kick?["to"] ?? "?") data=\(kick?["data"] ?? "?") | SIGNED legs=\(signed?.count ?? 0): \(signed?.map { $0.to } ?? [])")
        #expect(kick != nil)
    }

    @Test func i3_inputIsNotReadAsCalldata() {
        let params = #"[{"to":"0xd6145b2d3f379919e8cdeda7b97e37c4b2ca9c40","value":"0x0","input":"0x3593564c"}]"#
        let signed = SignExecutor.callsOf(method: "eth_sendTransaction", paramsJson: params)
        let kick = SigningController.clearKickoff(method: "eth_sendTransaction", paramsJson: params, chainId: 100, origin: nil)
        Probe.log("I-3", "SIGNED data=\(signed?.first?.data ?? "nil") | kickoff data=\(kick?["data"] ?? "?")")
        #expect(signed != nil)
    }

    @Test func i4_aJsonNumberValueIsReadAsZero() {
        let params = #"[{"to":"0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141","value":1000000000000000}]"#
        let signed = SignExecutor.callsOf(method: "eth_sendTransaction", paramsJson: params)
        let kick = SigningController.clearKickoff(method: "eth_sendTransaction", paramsJson: params, chainId: 100, origin: nil)
        let row = SignExecutor.recordRow([
            "record_id": "dapp-4", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": params, "result": "", "from": golden083, "chain_id": 100,
            "now_ms": 1_757_000_000_000, "status": "pending", "user_op_hash": "0xop", "dapp_origin": "https://x.test",
        ], nativeSymbol: "xDAI")
        Probe.log("I-4", "SIGNED value=\(signed?.first?.value ?? "nil") | kickoff value=\(kick?["value"] ?? "nil") | record value=\(row["value"] ?? "?")")
        #expect(signed != nil)
    }

    // MARK: - I-REC : the record's to / value

    @Test func iREC_aBatchRecordCarriesThePageWrittenToAndValue() {
        let params = #"[{"version":"2.0.0","from":"\#(golden083)","chainId":"0x64","to":"0x000000000000000000000000000000000000dEaD","value":"0xffffffffffffffffffff","calls":[{"to":"\#(golden083)","value":"0x0"}]}]"#
        let row = SignExecutor.recordRow([
            "record_id": "dapp-rec", "kind": "dapp_tx", "method": "wallet_sendCalls",
            "params_json": params, "result": "", "from": golden083, "chain_id": 100,
            "now_ms": 1_757_000_000_000, "status": "pending", "user_op_hash": "0xop", "dapp_origin": "http://127.0.0.1:8000",
        ], nativeSymbol: "xDAI")
        Probe.log("I-REC", "record to=\(row["to"] ?? "?") value=\(row["value"] ?? "?") dappOrigin=\(row["dappOrigin"] ?? "?") dappUrl=\(row["dappUrl"] ?? "absent")")
        #expect(row["type"] as? String == "dapp_tx")
    }

    // MARK: - I-H2 : does a dApp record reach the feed

    @Test func iH2_toWireCarriesNoDappFields() {
        let row = SignExecutor.recordRow([
            "record_id": "dapp-h2", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xd6145b2d3f379919e8cdeda7b97e37c4b2ca9c40","value":"0x0","data":"0x3593564c"}]"#,
            "result": "", "from": golden083, "chain_id": 8453, "now_ms": 1_757_000_000_000,
            "status": "pending", "user_op_hash": "0xop", "dapp_origin": "https://app.uniswap.org",
            "intent": "Execute",
        ], nativeSymbol: "ETH")
        let wire = TxRecords.toWire(row)
        Probe.log("I-H2", "row keys=\(row.keys.sorted()) | toWire keys=\((wire ?? [:]).keys.sorted()) dapp_url=\(String(describing: wire?["dapp_url"])) intent=\(String(describing: wire?["intent"])) to=\(String(describing: wire?["to"])) value=\(String(describing: wire?["value"]))")
        #expect(row["type"] as? String == "dapp_tx")
    }
}

// MARK: - The feed: what a dApp record becomes in 活动 (I-H2, I-F1, I-F3, I-8)

@MainActor
struct Probes083Feed {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"

    private func rows(of feed: FeedViewWire?) -> [FeedItemWire] {
        (feed?.rows ?? []).compactMap { row -> FeedItemWire? in
            if case .item(let item) = row { return item }
            return nil
        }
    }

    /// The records exactly as SignExecutor.recordRow writes them for a Send dust
    /// and a Uniswap swap, run through the real activity core and the shell's
    /// own row / detail builders.
    @Test func iH2_iF1_iF3_whatTheFeedDrawsForDappRecords() async {
        let defaults = UserDefaults(suiteName: "probe083.feed.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        let held = HeldTokens()
        let trust = TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held)
        let now = Date().timeIntervalSince1970
        let dust = SignExecutor.recordRow([
            "record_id": "dapp-dust", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"\#(founder)","value":"0x38d7ea4c68000"}]"#,
            "result": "0x7ca7aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa9269", "from": golden083, "chain_id": 100,
            "now_ms": now * 1000, "status": "confirmed", "user_op_hash": "0xdd", "dapp_origin": "http://192.168.50.17:8000",
        ], nativeSymbol: "xDAI")
        let swap = SignExecutor.recordRow([
            "record_id": "dapp-swap", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xd6145b2d3f379919e8cdeda7b97e37c4b2ca9c40","value":"0x0","data":"0x3593564c"}]"#,
            "result": "0x09c35478682ddce770c02e4aed27c9f0260f7b2f54ae6a7a194f50b70d604c9f", "from": golden083, "chain_id": 8453,
            "now_ms": (now - 5) * 1000, "status": "confirmed", "user_op_hash": "0xss", "dapp_origin": "https://app.uniswap.org",
            "intent": "Execute",
        ], nativeSymbol: "ETH")
        TxRecords.writeRecords([dust, swap], store: store)
        Probe.log("I-H2", "stored records: dust=\(dust) ; swap=\(swap)")

        let activity = ActivityStore(store: store, accounts: accounts, held: held, trust: trust)
        activity.open(address: golden083)
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline, rows(of: activity.feed).count < 2 { try? await Task.sleep(nanoseconds: 100_000_000) }

        let items = rows(of: activity.feed)
        Probe.log("I-H2", "feed items=\(items.count)")
        for item in items {
            let row = WalletLive.activityRow(item, loc: loc, hidden: false)
            var drawn = drawnDetail(item)
            drawn = drawn.isEmpty ? "(detail unavailable)" : drawn
            Probe.log("I-H2/F1/F3", "ROW title=«\(row.title)» subtitle=«\(row.subtitle)» amount=«\(row.amount)» unit=«\(row.unit)» | DETAIL \(drawn)")
        }
        #expect(!items.isEmpty)
    }

    private func drawnDetail(_ item: FeedItemWire) -> String {
        guard case .txDetail(let drawn)? = WalletFlowFixtures.build(.a3, loc: loc).sheet else { return "" }
        let detail = FlowsLive.txDetail(item, record: nil, on: drawn, loc: loc)
        let facts = detail.facts.map { "\($0.label)=«\($0.value)»" }.joined(separator: ", ")
        return "title=«\(detail.title)» amount=«\(detail.amount)» facts=[\(facts)]"
    }

    /// What the shipped core emits for the SAME dApp record once the shell
    /// stores dapp_url + intent (the desktop's shape), read as raw JSON: shows
    /// the core half of I-H2/I-F1/I-F3 exists in this build while FeedItemWire
    /// drops it.
    @Test func iH2_theCoreEmitsADappItemWhenGivenDappUrl() async {
        let defaults = UserDefaults(suiteName: "probe083.feedraw.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        let held = HeldTokens()
        let trust = TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held)
        let now = Date().timeIntervalSince1970
        var swap = SignExecutor.recordRow([
            "record_id": "dapp-swap", "kind": "dapp_tx", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xd6145b2d3f379919e8cdeda7b97e37c4b2ca9c40","value":"0x0","data":"0x3593564c"}]"#,
            "result": "0x09c3", "from": golden083, "chain_id": 8453,
            "now_ms": now * 1000, "status": "confirmed", "user_op_hash": "0xss", "dapp_origin": "https://app.uniswap.org",
        ], nativeSymbol: "ETH")
        swap["dapp_url"] = "https://app.uniswap.org"
        swap["calldata"] = true
        TxRecords.writeRecords([swap], store: store)
        let executor = ActivityExecutor(store: store, accounts: accounts, held: held, trust: trust)
        var lastView: [String: Any] = [:]
        let driver = CoreDriver(
            bridge: ActivityFeedCore(),
            perform: { operation in await executor.perform(operation) },
            onView: { lastView = $0 },
            onFault: { print("fault \($0)") }
        )
        driver.dispatch(CoreJSON.string(["type": "account_switched", "address": golden083]))
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline, ((lastView["rows"] as? [Any])?.count ?? 0) < 2 { try? await Task.sleep(nanoseconds: 100_000_000) }
        let raw = (try? JSONSerialization.data(withJSONObject: lastView["rows"] ?? [], options: [.sortedKeys])).flatMap { String(data: $0, encoding: .utf8) } ?? "?"
        Probe.log("I-H2core", "raw feed rows for a record WITH dapp_url/calldata (desktop shape): \(raw.prefix(1400))")
        #expect(!lastView.isEmpty)
    }

    // MARK: I-8 : the op-hash receipt translation ignores `confirmed`

    @Test func i8_userOpReceiptOfARevertedOp() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getUserOperationReceipt"] = .ok([
            "success": false,
            "receipt": ["transactionHash": "0xabc", "logs": [Any]()] as [String: Any],
        ] as [String: Any])
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let answer = await relay.userOpReceipt(chainId: 100, userOpHash: "0xop")
        // The shipped closure in RootView (resolveUserOp) is:
        //   guard case .resolved(_, let txHash, _, _) = answer, !txHash.isEmpty else { return nil }; return txHash
        var resolved: String?
        var confirmed: Bool?
        if case .resolved(let c, let txHash, _, _) = answer, !txHash.isEmpty { resolved = txHash; confirmed = c }
        Probe.log("I-8", "relay.userOpReceipt(reverted op) -> confirmed=\(String(describing: confirmed)) txHash=\(String(describing: resolved)); the resolveUserOp closure at RootView.swift:411-415 returns that tx hash whatever `confirmed` says (code-read)")
        #expect(resolved != nil)
    }

    // MARK: I-H4a trigger + a zero approval on the guard

    @Test func iH4a_theSheetsSlideThenFailsOnUnhashableTypedData() async {
        let port = ScriptedRelayPort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let spine = UserOpSpine(relay: relay, accounts: ScriptedAccounts(), signer: { CountingSigner() })
        let store = VelaStore(defaults: UserDefaults(suiteName: "probe083.h4a.\(UUID().uuidString)")!)
        let executor = SignExecutor(spine: spine, relay: relay, store: store)
        let answer = await executor.perform([
            "type": "sign_and_submit", "id": "r9", "method": "eth_signTypedData_v4",
            "params_json": #"["\#(golden083)", "{\"types\":{\"EIP712Domain\":[]},\"primaryType\":\"Missing\",\"domain\":{},\"message\":{}}"]"#,
            "chain_id": 100, "address": golden083,
        ])
        let outcome = (try? JSONSerialization.jsonObject(with: Data(answer.utf8)) as? [String: Any])?["outcome"]
        Probe.log("I-H4a-exec", "sign_and_submit(unhashable typed data) -> \(String(describing: outcome)) | relay calls=\(port.calls)")
        #expect(port.calls.isEmpty)
    }
}

// MARK: - Copy, caBLE and the fee row (I-H1, I-W20, I-U1b, I-U1c)

@MainActor
struct Probes083Misc {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    @Test func iH1_signInSheetCopy() {
        // WelcomeScreen.swift:102 `SignInMethodSheet` draws `methodCopy(method)` for
        // every KeyMethod; the create chooser draws the same function.
        var lines: [String] = []
        for method in KeyMethod.allCases {
            let copy = methodCopy(method)
            lines.append("\(method): title=«\(loc.t(copy.title))» body=«\(loc.t(copy.body))»")
        }
        Probe.log("I-H1", lines.joined(separator: " | "))
        #expect(!lines.isEmpty)
    }

    @Test func iW20_cableConnectUrlCase() {
        var plaintext = Data(repeating: 0, count: 11)
        plaintext.append(contentsOf: [0xAB, 0xCD, 0xEF])   // routing id: letters, to show case
        plaintext.append(contentsOf: [0x01, 0x00])           // tunnel domain 1
        let url = cableConnectUrl(
            staticSeed: Data(repeating: 9, count: 32),
            qrSecret: Data(repeating: 7, count: 16),
            advertPlaintext: plaintext
        )
        let hex = url.map { $0.split(separator: "/").suffix(2).joined(separator: " ") } ?? "nil"
        Probe.log("I-W20", "cableConnectUrl -> \(url ?? "nil")  ids=\(hex)  upperCase=\(url.map { u in u.split(separator: "/").suffix(2).allSatisfy { $0 == $0.uppercased() } } ?? false)")
        #expect(url != nil)
    }

    private func fee(failed: String?, options: Int) -> FeeViewWire {
        let coin = FeeOptionWire(
            symbol: "ETH", contract: nil, decimals: 18, balance: "1000000000000000000",
            recipient: "0xrelay", usdBalance: "1", usdPrice: nil, amount: "1000",
            insufficient: false, selected: false
        )
        let other = FeeOptionWire(
            symbol: "USDC", contract: "0xusdc", decimals: 6, balance: "1000000",
            recipient: "0xrelay", usdBalance: "1", usdPrice: nil, amount: "10",
            insufficient: false, selected: true
        )
        return FeeViewWire(
            busy: false, failed: failed, fee: nil, stale: false, feeToken: "0xusdc",
            options: options > 1 ? [coin, other] : [other], confirmFeeReady: false
        )
    }

    @Test func iU1b_iU1c_theFeeRowOverAFailedQuote() {
        var context = SigningLive.Context(
            loc: loc, chainName: "Base", chainDot: .blue, nativeSymbol: "ETH",
            walletName: "MultiTest", walletAddress: golden083.lowercased()
        )
        context.typicalS = 2
        let clear = ClearSigningViewWire(
            resolving: false, resolved: true, result: nil, message: nil,
            surface: .clearSign, confirm: .confirm, blindTyped: nil, dangerHaptic: false
        )
        let request = SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0xd6145b2d3f379919e8cdeda7b97e37c4b2ca9c40","value":"0x0","data":"0x3593564c"}]"#,
            origin: "https://app.uniswap.org", transportId: "tab-1", chainId: 8453
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 8453, blocked: nil
        )
        // The relay refuses a large op: the shell answers `simulation_failed`; the
        // core turns that into EstimateFailed (a retryable, network-worded failure)
        // — see the U1b code-read. What the sheet then draws for each failure word:
        for failed in ["estimate_failed", "would_fail"] {
            let model = SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
                clear: clear, guard: .empty, fee: fee(failed: failed, options: 2), context: context
            )
            Probe.log("I-U1b/c", "fee failed=\(failed), 2 coins: fee=\(String(describing: model.fee)) chevron=\(model.feeChevron) refresh=\(String(describing: model.feeRefresh?.label)) slideEnabled=\(String(describing: model.confirm?.enabled)) | words: estimateFailed=«\(loc.t("componentsUi.gas.estimateFailed"))» denialNetworkError=«\(loc.t("componentsUi.funding.denialNetworkError"))» simWillFail=«\(loc.t("componentsUi.signing.simWillFail"))»")
        }
        #expect(true)
    }
}


// MARK: - I-W13 : a dApp read behind a silent first node

@MainActor
struct Probes083Pool {

    @Test func iW13_theFirstReadAfterAFirstNodeGoesSilent() async {
        // A node that accepts the connection and never answers, listed FIRST as the
        // person's own endpoint (cold-start tier order: user, provider, default, public).
        let listener = try! NWListener(using: .tcp, on: .any)
        var held: [NWConnection] = []
        var port: UInt16 = 0
        listener.newConnectionHandler = { connection in held.append(connection); connection.start(queue: .main) }
        listener.stateUpdateHandler = { state in if case .ready = state { port = listener.port?.rawValue ?? 0 } }
        listener.start(queue: .main)
        let deadline = Date().addingTimeInterval(5)
        while port == 0, Date() < deadline { try? await Task.sleep(nanoseconds: 20_000_000) }

        let defaults = UserDefaults(suiteName: "probe083.pool.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        store.writeList(VelaStore.Key.networkConfig, [["chainId": 100, "rpcURL": "https://127.0.0.1:\(port)/"]])
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        var lines: [String] = []
        for index in 0..<3 {
            let started = Date()
            let outcome = await pool.call(chainId: 100, method: "eth_blockNumber", params: [], kind: "rpc")
            var shown = "?"
            switch outcome {
            case .ok(let v): shown = "ok \(String(describing: v))"
            case .failed(let rl): shown = "failed rateLimited=\(rl)"
            default: shown = "other"
            }
            lines.append("read \(index): \(Int(Date().timeIntervalSince(started) * 1000)) ms -> \(shown)")
        }
        Probe.log("I-W13", "user RPC #1 for Gnosis = a silent node (accepts, never answers); dApp-path reads via RpcPool.call: \(lines.joined(separator: " | "))")
        listener.cancel()
        held.forEach { $0.cancel() }
        #expect(lines.count == 3)
    }
}
