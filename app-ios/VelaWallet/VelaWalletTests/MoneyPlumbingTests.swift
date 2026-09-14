//
//  MoneyPlumbingTests.swift
//  VelaWalletTests
//
//  Spec 052 phase 1: the money plumbing, pinned before anything can spend.
//
//  Every test here is hermetic — the relay is a scripted port, the signer is a
//  counter — because the two properties worth proving are about ORDER and
//  REFUSAL, and both are invisible once a real network is in the way.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - Scripted transports

/// A `RelayPort` whose every answer is written down in advance.
@MainActor
final class ScriptedRelayPort: RelayPort {
    /// Keyed by method. A method with no entry answers `.failed`.
    var rpc: [String: RpcOutcome] = [:]
    var rest: [String: CoreHTTP.RestAnswer] = [:]
    var base: String? = "https://relay.test"
    var bestUrl: String? = "https://rpc.test"
    /// Every call made, in order — the order IS the contract.
    private(set) var calls: [String] = []
    private(set) var restPaths: [String] = []
    private(set) var xRpcUrls: [String?] = []

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        calls.append(method)
        return rpc[method] ?? .failed(rateLimited: false)
    }

    func bundlerBase(chainId: Int) async -> String? { base }
    func bestRpcUrl(chainId: Int) async -> String? { bestUrl }

    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer {
        restPaths.append(url)
        xRpcUrls.append(xRpcUrl)
        for (suffix, answer) in rest where url.hasSuffix(suffix) { return answer }
        return .failed
    }
}

/// An account store that answers one wallet, and a signer that counts.
@MainActor
final class ScriptedAccounts: UserOpSpine.AccountPort {
    var keyList: [WalletKeyRecord] = [
        WalletKeyRecord(credentialId: "cred-0", publicKeyHex: "04" + String(repeating: "11", count: 64)),
    ]

    func keys(of address: String) async -> [WalletKeyRecord] { keyList }
    func routing(of address: String) async -> (transports: String, method: KeyMethod) {
        ("internal", .platform)
    }
}

/// The seam spec 052 FR-009 counts. It never produces a usable assertion —
/// every test here stops before one is needed, which is the point.
@MainActor
final class CountingSigner: UserOpSigner {
    private(set) var calls = 0
    var failure: PasskeyFailure = PasskeyFailure(kind: .cancelled, message: "cancelled")

    func sign(
        challenge: Data,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod
    ) async throws -> Assertion {
        calls += 1
        throw failure
    }
}

// MARK: - The spine

@MainActor
struct UserOpSpineTests {
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let gnosis = 100

    /// A default argument is evaluated in a NONISOLATED context, so
    /// `= ScriptedAccounts()` does not compile against a `@MainActor` type.
    /// `nil` and a fallback inside the body is the shape that does.
    private func spine(
        _ port: ScriptedRelayPort,
        _ signer: CountingSigner,
        accounts: ScriptedAccounts? = nil
    ) -> UserOpSpine {
        UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: accounts ?? ScriptedAccounts(),
            signer: { signer }
        )
    }

    private func transfer() -> [UserOpCall] {
        [UserOpCall(to: golden, value: "1000000000000000", data: "0x")]
    }

    private func usableQuote() -> UserOpSpine.Quoted {
        UserOpSpine.Quoted(amount: "1000", recipient: golden)
    }

    /// A wallet whose deployment state could not be read stops **before** the
    /// ceremony. The assertion is the expensive, interruptive step: raising a
    /// Face ID sheet for an operation the relay must reject spends somebody's
    /// attention on nothing.
    @Test func anUnreachableChainRefusesBeforeThePrompt() async {
        let port = ScriptedRelayPort()   // eth_getCode unscripted ⇒ .failed
        let signer = CountingSigner()
        await #expect(throws: UserOpSpine.Refused.self) {
            try await spine(port, signer).submit(
                chainId: gnosis, account: golden, calls: transfer(),
                gasFeeToken: nil, quotedFee: usableQuote()
            )
        }
        #expect(signer.calls == 0)
    }

    /// A deployed wallet whose nonce could not be read stops before the
    /// ceremony too, and for the same reason.
    @Test func anUnreadableNonceRefusesBeforeThePrompt() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080604052")   // deployed
        // eth_call (getNonce) unscripted ⇒ .failed
        let signer = CountingSigner()
        await #expect(throws: UserOpSpine.Refused.self) {
            try await spine(port, signer).submit(
                chainId: gnosis, account: golden, calls: transfer(),
                gasFeeToken: nil, quotedFee: usableQuote()
            )
        }
        #expect(signer.calls == 0)
        // Proof of ORDER, not just of outcome: the deployment read happened,
        // the nonce read happened, and nothing after it did.
        #expect(port.calls == ["eth_getCode", "eth_call"])
    }

    /// A missing or stale quote is refused before the prompt: the whole point
    /// of `quoted_fee` is that the figure on the confirm screen is the figure
    /// signed, so there is nothing to sign without one.
    @Test func anAbsentQuoteRefusesBeforeThePrompt() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")             // not deployed ⇒ nonce 0x0
        let signer = CountingSigner()
        await #expect(throws: UserOpSpine.Refused.self) {
            try await spine(port, signer).submit(
                chainId: gnosis, account: golden, calls: transfer(),
                gasFeeToken: nil, quotedFee: nil
            )
        }
        #expect(signer.calls == 0)
    }

    /// The core's own predicate decides whether a quote is usable — the shell
    /// does not re-derive it. A zero amount is not payable.
    @Test func theQuoteGateIsTheCores() {
        #expect(quotedFeeUsable(amount: "1000", recipient: golden))
        #expect(!quotedFeeUsable(amount: "0", recipient: golden))
        #expect(!quotedFeeUsable(amount: "1000", recipient: ""))
    }

    /// A wallet with no key never reaches the network at all.
    @Test func anAccountWithoutAKeyRefusesImmediately() async {
        let port = ScriptedRelayPort()
        let signer = CountingSigner()
        let accounts = ScriptedAccounts()
        accounts.keyList = []
        await #expect(throws: UserOpSpine.Refused.self) {
            try await spine(port, signer, accounts: accounts).submit(
                chainId: gnosis, account: golden, calls: transfer(),
                gasFeeToken: nil, quotedFee: usableQuote()
            )
        }
        #expect(signer.calls == 0)
        #expect(port.calls.isEmpty)
    }

    /// A cancelled ceremony is `passkeyCancelled` — never `other` — because the
    /// core routes the two differently: one returns to confirm, the other
    /// raises an error surface.
    @Test func aCancelledCeremonyIsCancelledRatherThanAnError() async throws {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        let signer = CountingSigner()
        signer.failure = PasskeyFailure(kind: .cancelled, message: "")

        var caught: UserOpSpine.Failure?
        do {
            _ = try await spine(port, signer).submit(
                chainId: gnosis, account: golden, calls: transfer(),
                gasFeeToken: nil, quotedFee: usableQuote()
            )
        } catch let refused as UserOpSpine.Refused {
            caught = refused.failure
        }
        #expect(caught == .passkeyCancelled)
        // Exactly one prompt for the attempt (FR-009). Counted, not assumed.
        #expect(signer.calls == 1)
    }

    /// The SafeOp hash the ceremony signs is **32 bytes and the core's**. A
    /// shell that pre-hashed, or passed the wrong length, would sign bytes the
    /// Safe does not verify — and would only find out from a reverted
    /// transaction.
    @Test func theChallengeIsTheCoresThirtyTwoByteSafeOpHash() async throws {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        let signer = LengthRecordingSigner()
        let spine = UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: ScriptedAccounts(),
            signer: { signer }
        )
        _ = try? await spine.submit(
            chainId: gnosis, account: golden, calls: transfer(),
            gasFeeToken: nil, quotedFee: usableQuote()
        )
        #expect(signer.challengeLength == 32)
    }
}

/// Records what it was asked to sign, then refuses.
@MainActor
final class LengthRecordingSigner: UserOpSigner {
    private(set) var challengeLength = -1

    func sign(
        challenge: Data,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod
    ) async throws -> Assertion {
        challengeLength = challenge.count
        throw PasskeyFailure(kind: .cancelled, message: "")
    }
}

// MARK: - The relay client

@MainActor
struct RelayClientTests {
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func client(_ port: ScriptedRelayPort, now: @escaping () -> Double = { 0 }) -> RelayClient {
        RelayClient(port: port, now: now, retryDelayMs: 0)
    }

    /// **404 is "this chain is not covered", never "the relay is down".** The
    /// two open different screens: one says the network has no gas sponsorship
    /// at all, the other says try again.
    @Test func aMissingTreasuryIsUncoveredRatherThanUnknown() async {
        let port = ScriptedRelayPort()
        port.rest["/v1/treasury/100"] = .status(404)
        if case .uncovered = await client(port).probeTreasury(chainId: 100) {} else {
            Issue.record("a 404 must read as uncovered")
        }

        port.rest["/v1/treasury/100"] = .status(503)
        if case .unknown = await client(port).probeTreasury(chainId: 100) {} else {
            Issue.record("a 503 must read as unknown")
        }
    }

    /// The relay reads the chain through the endpoint THIS wallet picked.
    @Test func everyRestCallCarriesThePoolsChosenRpcUrl() async {
        let port = ScriptedRelayPort()
        port.bestUrl = "https://rpc.gnosischain.example"
        port.rest["/v1/treasury/100"] = .status(404)
        _ = await client(port).probeTreasury(chainId: 100)
        #expect(port.xRpcUrls == ["https://rpc.gnosischain.example"])
        #expect(port.restPaths == ["https://relay.test/v1/treasury/100"])
    }

    /// A trailing slash on the configured base must not produce `//v1/…`.
    @Test func theBaseIsJoinedWithoutADoubleSlash() async {
        let port = ScriptedRelayPort()
        port.base = "https://relay.test/"
        port.rest["/v1/treasury/100"] = .status(404)
        _ = await client(port).probeTreasury(chainId: 100)
        #expect(port.restPaths == ["https://relay.test/v1/treasury/100"])
    }

    /// The two shapes that dropped every row on Android's first device run: the
    /// relay writes `balance` as HEX, and `feeToken` as null on the native row.
    @Test func aQuoteRowIsParsedInTheRelaysOwnShapes() async throws {
        let port = ScriptedRelayPort()
        port.rpc["vela_getInBandGasQuote"] = .ok([
            [
                "recipient": golden, "asset": "native", "feeToken": NSNull(),
                "balance": "0xde0b6b3a7640000", "decimals": 18, "symbol": "XDAI",
                "usdBalance": "0.99975138", "usdPrice": "0.99975138",
            ] as [String: Any],
        ])
        let quotes = try #require(await client(port).inBandQuotes(chainId: 100, safe: golden))
        #expect(quotes.count == 1)
        // 0xde0b6b3a7640000 = one whole token in raw units.
        #expect(quotes[0]["balance"] as? String == "1000000000000000000")
        #expect(quotes[0]["fee_token"] is NSNull)
        #expect(quotes[0]["symbol"] as? String == "XDAI")
    }

    /// An ERC-20 row without a price cannot be converted, so it is dropped
    /// rather than shown as a fee asset nobody can price.
    @Test func anUnpricedErc20RowIsDropped() async {
        let port = ScriptedRelayPort()
        port.rpc["vela_getInBandGasQuote"] = .ok([
            [
                "recipient": golden, "asset": "erc20", "feeToken": golden,
                "balance": "0x1", "decimals": 6, "symbol": "USDC",
                "usdBalance": "1", "usdPrice": NSNull(),
            ] as [String: Any],
        ])
        #expect(await client(port).inBandQuotes(chainId: 100, safe: golden) == nil)
    }

    /// The quote cache spares the relay four questions a second while somebody
    /// taps through the fee sheet — and `clearCaches()` is what the settings
    /// machine's `clear_bundler_cache` means here.
    @Test func quotesAreCachedAndTheSettingsMachineCanClearThem() async {
        let port = ScriptedRelayPort()
        port.rpc["vela_getInBandGasQuote"] = .ok([
            [
                "recipient": golden, "asset": "native", "feeToken": NSNull(),
                "balance": "0x1", "decimals": 18, "symbol": "XDAI",
                "usdBalance": "1", "usdPrice": "1",
            ] as [String: Any],
        ])
        let relay = client(port)
        _ = await relay.inBandQuotes(chainId: 100, safe: golden)
        _ = await relay.inBandQuotes(chainId: 100, safe: golden)
        #expect(port.calls.filter { $0 == "vela_getInBandGasQuote" }.count == 1)

        relay.clearCaches()
        _ = await relay.inBandQuotes(chainId: 100, safe: golden)
        #expect(port.calls.filter { $0 == "vela_getInBandGasQuote" }.count == 2)
    }

    /// A refusal reaches the caller as the relay's own sentence. Before spec
    /// 052 the pool flattened a JSON-RPC error to `.ok(nil)` and this sentence
    /// was lost — which is how "you already have an operation pending" became
    /// "the relay is unreachable", and a wallet would submit twice.
    @Test func aRefusalCarriesTheRelaysSentence() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_sendUserOperation"] = .rpcError(code: -32521, message: "AA25 invalid account nonce")
        let answer = await client(port).sendUserOp(chainId: 100, opJson: "{\"sender\":\"0x0\"}")
        guard case .rejected(let json) = answer else {
            Issue.record("a refusal must be rejected, not unreachable")
            return
        }
        // And the sentence that reaches the screen is the CORE's, not the
        // relay's jargon: `relay_error_message` is a translator.
        #expect(relayErrorMessage(errorJson: json) == "Transaction nonce mismatch. Please try again.")
    }

    /// An operation already pending for this nonce is the SAME operation. Its
    /// hash is answered so the tracker follows what is really on the wire —
    /// submitting again would be a second spend.
    ///
    /// The marker is read from the RAW error json, because the translator
    /// replaces a message matching a known rung wholesale: "AA25 … 
    /// [existingHash:0x…]" becomes "Transaction nonce mismatch." and the marker
    /// is gone. A duplicate submit is exactly an AA25 condition, so this is the
    /// case where it matters most.
    @Test func anAlreadyPendingOperationSurvivesTheTranslation() {
        let raw = "{\"message\":\"AA25 invalid account nonce [existingHash:0xabc123]\"}"
        let translated = relayErrorMessage(errorJson: raw)
        #expect(translated == "Transaction nonce mismatch. Please try again.")
        #expect(parseExistingUserOpHash(message: translated) == nil)
        #expect(parseExistingUserOpHash(message: raw) == "0xabc123")
    }

    /// An unreachable relay is NOT a rejection: nothing was refused, so nothing
    /// is classified, and the screen says try again.
    @Test func anUnreachableRelayIsNotARejection() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_sendUserOperation"] = .failed(rateLimited: false)
        if case .unreachable = await client(port).sendUserOp(chainId: 100, opJson: "{}") {} else {
            Issue.record("a swept-clean pool must read as unreachable")
        }
    }

    /// "Currently processing" is the relay asking for a moment, not a refusal.
    /// It is retried up to three times and then reported honestly.
    @Test func aBusyRelayIsRetriedAndThenReported() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_sendUserOperation"] = .rpcError(code: -32000, message: "currently processing")
        let answer = await client(port).sendUserOp(chainId: 100, opJson: "{}")
        guard case .rejected = answer else {
            Issue.record("a busy relay that never frees up is a rejection")
            return
        }
        // One attempt plus three retries.
        #expect(port.calls.filter { $0 == "eth_sendUserOperation" }.count == 4)
    }

    /// A hex quantity becomes a decimal string, in string arithmetic: a nonce
    /// or a balance routinely exceeds what a `Double` can carry, and rounding
    /// one is how a wallet comes to disagree with the chain.
    @Test func quantitiesCrossAsDecimalStrings() {
        #expect(RelayClient.decimalOfHex("0xa8867319d2da000") == "758970000000000000")
        #expect(RelayClient.decimalOfHex("0x0") == "0")
        #expect(RelayClient.decimalOfHex("not hex") == nil)
        #expect(RelayClient.decimalOfAny(NSNumber(value: 7)) == "7")
        #expect(RelayClient.decimalOfAny("0xff") == "255")
    }

    /// The bundler wants `0x0`, not a padded word — and the round trip must
    /// survive a number no `UInt64` holds.
    @Test func aNonceIsMintedAsAMinimalQuantity() {
        #expect(RelayClient.hexQuantity(decimal: "0") == "0x0")
        #expect(RelayClient.hexQuantity(decimal: "255") == "0xff")
        let huge = "123456789012345678901234567890"
        let back = RelayClient.decimalOfHex(RelayClient.hexQuantity(decimal: huge))
        #expect(back == huge)
    }

    /// A price is kept **as written**. Parsing is only to reject nonsense; a
    /// round trip through `Double` is how a price loses its last digits.
    @Test func aPriceIsKeptAsTheRelayWroteIt() {
        #expect(RelayClient.decimalText("0.99975138") == "0.99975138")
        #expect(RelayClient.decimalText("-1") == nil)
        #expect(RelayClient.decimalText("abc") == nil)
    }
}

// MARK: - The transaction store's write half

@MainActor
struct TxRecordWriteTests {
    private func store() -> VelaStore {
        VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
    }

    private func record(
        _ id: String,
        status: String = "pending",
        userOpHash: String = "0xaa",
        txHash: String = "",
        timestamp: Double = 1_700_000_000
    ) -> [String: Any] {
        [
            "id": id, "userOpHash": userOpHash, "txHash": txHash,
            "from": "0x1", "to": "0x2", "value": "1", "symbol": "XDAI",
            "decimals": 18, "chainId": 100, "timestamp": timestamp,
            "status": status, "type": "send",
        ]
    }

    /// Sibling records land together. A split that wrote once per leg could be
    /// interrupted halfway, leaving half a payment in somebody's history.
    @Test func siblingRecordsAreWrittenInOneGo() {
        let store = store()
        TxRecords.writeRecords([record("a"), record("b"), record("c")], store: store)
        #expect(TxRecords.load(store: store).count == 3)
    }

    /// A re-submit of the same operation updates its row rather than doubling
    /// it — the id is the identity.
    @Test func aRepeatedIdReplacesRatherThanDuplicates() {
        let store = store()
        TxRecords.writeRecords([record("a", status: "pending")], store: store)
        TxRecords.writeRecords([record("a", status: "confirmed", txHash: "0xbb")], store: store)
        let rows = TxRecords.load(store: store)
        #expect(rows.count == 1)
        #expect(rows[0]["status"] as? String == "confirmed")
    }

    /// The cap is the Expo client's 200, on every client.
    @Test func theStoreKeepsTheNewestTwoHundred() {
        let store = store()
        let many = (0..<250).map { record("id-\($0)", timestamp: Double(1_700_000_000 + $0)) }
        TxRecords.writeRecords(many, store: store)
        let rows = TxRecords.load(store: store)
        #expect(rows.count == 200)
        // Newest first, so the last-written id survives and the oldest is gone.
        #expect(rows.first?["id"] as? String == "id-249")
        #expect(!rows.contains { ($0["id"] as? String) == "id-0" })
    }

    /// A patch lands **in place**. One that appended would show somebody their
    /// payment twice, one of them pending forever.
    @Test func aPatchTouchesTheSameRowsAndMakesNoNewOnes() {
        let store = store()
        TxRecords.writeRecords([record("a"), record("b")], store: store)
        TxRecords.patch(ids: ["a"], fields: ["status": "confirmed", "txHash": "0xcc"], store: store)
        let rows = TxRecords.load(store: store)
        #expect(rows.count == 2)
        let patched = rows.first { ($0["id"] as? String) == "a" }
        #expect(patched?["status"] as? String == "confirmed")
        #expect(patched?["txHash"] as? String == "0xcc")
        #expect(rows.first { ($0["id"] as? String) == "b" }?["status"] as? String == "pending")
    }

    /// The pending set is DERIVED, which is why a force-quit loses nothing:
    /// pending, with an operation hash, and no transaction hash yet.
    @Test func thePendingSetIsDerivedFromTheStore() {
        let store = store()
        TxRecords.writeRecords([
            record("pending-one"),
            record("already-landed", status: "confirmed", txHash: "0xdd"),
            record("no-hash", userOpHash: ""),
            record("has-tx", txHash: "0xee"),
        ], store: store)
        let pending = TxRecords.pending(store: store).compactMap { $0["id"] as? String }
        #expect(pending == ["pending-one"])
    }
}
