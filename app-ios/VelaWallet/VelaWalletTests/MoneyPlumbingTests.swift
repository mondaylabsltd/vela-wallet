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

// MARK: - The send machine, driven end to end without a network

/// The picker's rows are the holdings the balance machine already found.
///
/// Written after the device showed an EMPTY picker: not the fixture list, not
/// the wallet's own — nothing at all. A screen with no rows cannot say whether
/// the shell answered badly or the core refused the answer, so this drives the
/// real machine with a scripted holding and asks it.
@MainActor
struct SendMachineTests {
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /// A `BalanceViewWire` built the way the core would emit one. Decoded from
    /// JSON rather than constructed, because the wire's initialiser is the
    /// decoder and a hand-built value could drift from what actually arrives.
    private func balance(symbol: String = "xDAI", chainId: Int = 100, amount: String = "0.53097") throws -> BalanceViewWire {
        let json = """
        {
          "address": "\(golden)",
          "display_total_usd": 0.53, "balance_unknown": false, "balance_partial": false,
          "notice": null, "hidden": false, "refreshing": false,
          "last_refreshed_at_ms": null,
          "tokens": [{
            "chain_id": \(chainId), "symbol": "\(symbol)", "name": "\(symbol)",
            "balance": "\(amount)", "decimals": 18, "token_address": null,
            "price_usd": 1.0, "spam": false
          }],
          "unpriced_tokens": [],
          "failed_chain_ids": [], "rate_limited_chain_ids": [], "banner_chain_ids": [],
          "holdings_loading": false, "cached_total_usd": null,
          "switcher": { "open": false, "loading": false, "balances": [] }
        }
        """
        return try CoreJSON.decode(BalanceViewWire.self, from: CoreJSON.object(json))
    }

    private func networks(chainId: Int = 100) throws -> NetViewWire {
        let json = """
        {
          "loaded": true, "last_added_chain_id": null, "endpoints": [], "providers": [],
          "networks": [{
            "id": "chain-\(chainId)", "chain_id": \(chainId), "display_name": "Gnosis",
            "native_symbol": "xDAI", "is_custom": false,
            "rpc_url": "", "explorer_url": "", "bundler_url": "",
            "rpc_health": null, "explorer_health": null, "rpc_chain_mismatch": null,
            "rpc_save_deferred": false
          }],
          "wizard": {
            "phase": "idle", "query": "", "custom_rpc": "", "suggestions": [],
            "chain_info": null, "compat": null, "error": null, "can_add": false
          }
        }
        """
        return try CoreJSON.decode(NetViewWire.self, from: CoreJSON.object(json))
    }

    /// Drive the real `send` machine with a scripted world and read its view.
    @Test func thePickerShowsTheHoldingsTheBalanceMachineFound() async throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let port = ScriptedRelayPort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accountPort = ScriptedAccounts()
        let fees = FeeStore(relay: relay, accounts: accountPort)
        let held = try balance()
        let nets = try networks()

        let executor = SendExecutor(
            store: store, relay: relay, pool: RpcPool(store: store, accounts: accounts),
            spine: UserOpSpine(relay: relay, accounts: accountPort, signer: { CountingSigner() }),
            accounts: accountPort, fees: fees,
            identity: RecipientIdentity(
                store: store, pool: RpcPool(store: store, accounts: accounts), accounts: accounts
            ),
            metadata: TokenMetadata(store: store, pool: RpcPool(store: store, accounts: accounts)),
            accountStore: accounts,
            balances: { held },
            networks: { nets },
            ports: SendExecutor.Ports()
        )
        let send = SendStore(executor: executor)
        send.open(
            accountId: "cred-0", address: golden, name: "Parallel One",
            displayCode: "USD", displayRate: 1, fiatDecimals: 2
        )

        // The fetch is one hop through the effect loop, with no network in it.
        for _ in 0..<40 where send.view?.tokens.isEmpty ?? true {
            try? await Task.sleep(nanoseconds: 25_000_000)
        }
        let view = try #require(send.view)
        #expect(view.tokens.count == 1)
        #expect(view.tokens.first?.symbol == "xDAI")
        // And the id the picker sends back is the core's own spelling.
        #expect(view.tokens.first?.id == "chain-100_native_xDAI")
    }
    /// Picking a token moves the machine to the form.
    ///
    /// The screen navigates by `SendLive.flowState(view)` rather than by
    /// remembering where it came from, so if the core does not move, nothing
    /// on screen does either — which is exactly what the device showed.
    @Test func pickingATokenMovesTheMachineToTheForm() async throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let port = ScriptedRelayPort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accountPort = ScriptedAccounts()
        let fees = FeeStore(relay: relay, accounts: accountPort)
        let held = try balance()
        let nets = try networks()
        let pool = RpcPool(store: store, accounts: accounts)

        let executor = SendExecutor(
            store: store, relay: relay, pool: pool,
            spine: UserOpSpine(relay: relay, accounts: accountPort, signer: { CountingSigner() }),
            accounts: accountPort, fees: fees,
            identity: RecipientIdentity(store: store, pool: pool, accounts: accounts),
            metadata: TokenMetadata(store: store, pool: pool),
            accountStore: accounts,
            balances: { held }, networks: { nets }, ports: SendExecutor.Ports()
        )
        let send = SendStore(executor: executor)
        send.open(
            accountId: "cred-0", address: golden, name: "Parallel One",
            displayCode: "USD", displayRate: 1, fiatDecimals: 2
        )
        for _ in 0..<40 where send.view?.tokens.isEmpty ?? true {
            try? await Task.sleep(nanoseconds: 25_000_000)
        }
        let id = try #require(send.view?.tokens.first?.id)

        send.selectToken(id: id)
        for _ in 0..<40 where send.view?.stage != .enterDetails {
            try? await Task.sleep(nanoseconds: 25_000_000)
        }
        let view = try #require(send.view)
        #expect(view.stage == .enterDetails)
        #expect(view.selectedToken?.symbol == "xDAI")
        // And the screen would follow, because it navigates by this.
        #expect(SendLive.flowState(view, feeSheetOpen: false) == .sd2)
    }

    /// Confirming reaches the ceremony **once**, and a cancel ends the attempt.
    ///
    /// FR-009 counted, not asserted from a reading: the signer records every
    /// call, and a second prompt after a cancel is the defect that test exists
    /// for. The spine refuses this attempt before the relay is reachable
    /// anyway, which is fine — what is under test is the checkpoint, not the
    /// submission.
    @Test func confirmingAsksTheSignerOnceAndACancelEndsTheAttempt() async throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let port = ScriptedRelayPort()
        // A deployed wallet with a readable nonce, so the spine gets as far as
        // the ceremony rather than refusing before it.
        port.rpc["eth_getCode"] = .ok("0x6080604052")
        port.rpc["eth_call"] = .ok("0x" + String(repeating: "0", count: 63) + "1")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accountPort = ScriptedAccounts()
        let signer = CountingSigner()
        let fees = FeeStore(relay: relay, accounts: accountPort)
        let pool = RpcPool(store: store, accounts: accounts)

        let executor = SendExecutor(
            store: store, relay: relay, pool: pool,
            spine: UserOpSpine(relay: relay, accounts: accountPort, signer: { signer }),
            accounts: accountPort, fees: fees,
            identity: RecipientIdentity(store: store, pool: pool, accounts: accounts),
            metadata: TokenMetadata(store: store, pool: pool),
            accountStore: accounts,
            balances: { try? balance() }, networks: { try? networks() },
            ports: SendExecutor.Ports()
        )

        // Drive the submit arm directly: the screen's job is to raise it, and
        // the core's gating is already its own tested business.
        let answer = await executor.perform([
            "type": "submit_user_op",
            "chain_id": 100,
            "account": golden,
            "public_key_hex": "04" + String(repeating: "11", count: 64),
            "calls": [["to": golden, "value": "1000", "data": "0x"] as [String: Any]],
            "gas_fee_token": NSNull(),
            "quoted_fee": ["amount": "1000", "recipient": golden] as [String: Any],
        ])
        let reply = try CoreJSON.object(answer)
        #expect(reply["type"] as? String == "submit_failed")
        // Exactly one ceremony for the attempt.
        #expect(signer.calls == 1)
        // And a cancelled one is reported as a CANCEL, which the core routes
        // back to confirm — not as an error surface over a ceremony the person
        // themselves stopped.
        let failure = reply["failure"] as? [String: Any] ?? [:]
        #expect(failure["type"] as? String == "passkey_cancelled")
    }

    /// The pending row is written before anything tracks it.
    ///
    /// The core emits `persist_tx_records` and then `track_submitted`; a shell
    /// that wrote asynchronously would let the tracker's patch land on nothing.
    /// This asserts the write is DONE when the operation is answered.
    @Test func aSubmittedSendIsOnDiskBeforeItIsTracked() async throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let port = ScriptedRelayPort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accountPort = ScriptedAccounts()
        let pool = RpcPool(store: store, accounts: accounts)
        var tracked: [String] = []

        let executor = SendExecutor(
            store: store, relay: relay, pool: pool,
            spine: UserOpSpine(relay: relay, accounts: accountPort, signer: { CountingSigner() }),
            accounts: accountPort, fees: FeeStore(relay: relay, accounts: accountPort),
            identity: RecipientIdentity(store: store, pool: pool, accounts: accounts),
            metadata: TokenMetadata(store: store, pool: pool),
            accountStore: accounts,
            balances: { nil }, networks: { nil },
            ports: SendExecutor.Ports(trackSubmitted: { _, ids, _ in
                // Read the store from INSIDE the handoff: if the write were
                // asynchronous, this is where it would still be empty.
                tracked = TxRecords.pending(store: store).compactMap { $0["id"] as? String }
                _ = ids
            })
        )

        _ = await executor.perform([
            "type": "persist_tx_records",
            "records": [[
                "id": "rec-1", "user_op_hash": "0xaa", "tx_hash": "",
                "from": golden, "to": golden, "value": "1", "symbol": "XDAI",
                "decimals": 18, "logo_urls": [String](), "chain_id": 100,
                "timestamp_s": 1_700_000_000, "usd": NSNull(),
            ] as [String: Any]],
        ])
        _ = await executor.perform([
            "type": "track_submitted", "user_op_hash": "0xaa",
            "record_ids": ["rec-1"], "chain_id": 100,
        ])

        #expect(tracked == ["rec-1"])
        // And it is a PENDING row: status pending, an operation hash, no
        // transaction hash yet — which is what makes it resumable.
        let row = try #require(TxRecords.load(store: store).first)
        #expect(row["status"] as? String == "pending")
        #expect(row["userOpHash"] as? String == "0xaa")
        #expect((row["txHash"] as? String ?? "").isEmpty)
        // Seconds, not milliseconds.
        #expect((row["timestamp"] as? NSNumber)?.doubleValue == 1_700_000_000)
    }

    /// The fee session settles, and the estimate it settles on is the core's.
    ///
    /// Written because the device showed 估算中… forever: a quote that never
    /// settles holds `estimate_fee` open, which holds the confirm gate shut,
    /// and nothing on screen says why.
    @Test func theFeeSessionSettlesOnAQuote() async throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let port = ScriptedRelayPort()
        // Everything the fee machine reads, answered.
        port.rpc["eth_gasPrice"] = .ok("0x3b9aca00")
        port.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x3b9aca00"] as [String: Any])
        port.rpc["eth_maxPriorityFeePerGas"] = .ok("0x3b9aca00")
        port.rpc["pimlico_getUserOperationGasPrice"] = .ok([
            "fast": ["maxFeePerGas": "0x77359400"] as [String: Any],
        ] as [String: Any])
        port.rpc["vela_getInBandGasQuote"] = .ok([
            [
                "recipient": golden, "asset": "native", "feeToken": NSNull(),
                "balance": "0xde0b6b3a7640000", "decimals": 18, "symbol": "XDAI",
                "usdBalance": "1", "usdPrice": "1",
            ] as [String: Any],
        ])
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x186a0",
            "callGasLimit": "0x186a0",
            "preVerificationGas": "0x186a0",
        ] as [String: Any])
        port.rpc["eth_getCode"] = .ok("0x6080604052")
        port.rpc["eth_call"] = .ok("0x" + String(repeating: "0", count: 63) + "1")
        port.rest["/v1/account/100/\(golden.lowercased())"] = .ok([
            "activeDepositAddress": golden, "status": "ACTIVE",
        ])

        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts())
        let settled = await fees.quote(
            chainId: 100, account: golden, deployed: true, publicKeyAvailable: true,
            calls: [["to": golden, "value": "1000", "data": "0x"] as [String: Any]],
            feeToken: nil
        )
        // Before anything else: did a view ever decode? A wire that does not
        // match leaves `view` nil and every settle check false, which looks
        // exactly like a machine that is still working.
        #expect(fees.view != nil, "no fee view ever decoded — the wire does not match")
        if settled == nil, let stuck = fees.view {
            Issue.record("""
            the quote never settled — busy=\(stuck.busy) failed=\(stuck.failed ?? "nil") \
            fee=\(stuck.fee == nil ? "nil" : "present") options=\(stuck.options.count) \
            calls=\(port.calls.joined(separator: ","))
            """)
        }
        let view = try #require(settled, "the quote never settled")
        #expect(!view.busy)
        // A settled quote either priced it or said why. Both are answers; a
        // hang is not.
        #expect(view.fee != nil || view.failed != nil)
        if let fee = view.fee {
            #expect(fee.chainId == 100)
            #expect(!fee.totalWei.isEmpty)
        }
    }

    /// A launch picks up what a previous run left in flight.
    ///
    /// There is no tracker key on any client: the pending set is DERIVED from
    /// `vela.transactionHistory`, which is exactly what makes a force-quit
    /// lose nothing. This drives the real machine against a store that already
    /// holds a pending row, as a relaunch would find it.
    @Test func aRelaunchPicksUpWhatWasStillInFlight() async throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        // What the previous run wrote at submit. The timestamp is RECENT on
        // purpose: the core refuses to give a verdict to an operation older
        // than twenty-four hours (its rule ④), so a fixture dated 2023 is not
        // "a pending send" to it — it is one it has rightly stopped following.
        TxRecords.writeRecords([[
            "id": "rec-1", "userOpHash": "0xcf9f", "txHash": "",
            "from": golden, "to": golden, "value": "1", "symbol": "XDAI",
            "decimals": 18, "chainId": 100,
            "timestamp": Date().timeIntervalSince1970,
            "status": "pending", "type": "send",
        ]], store: store)

        let port = ScriptedRelayPort()
        // The relay says it landed.
        port.rpc["eth_getUserOperationReceipt"] = .ok([
            "success": true,
            "sender": golden,
            "receipt": [
                "transactionHash": "0x151d", "logs": [[String: Any]](),
            ] as [String: Any],
        ] as [String: Any])
        var notified: [String] = []
        let executor = TrackerExecutor(
            store: store,
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            ports: TrackerExecutor.Ports(
                notifyConfirmed: { hash, _, _ in notified.append(hash) }
            )
        )
        let tracker = TrackerStore(executor: executor)
        tracker.boot()

        // Wait for the RECORD, not for the entry: the verdict lands in the
        // view first and reaches disk one effect later, and a test that stops
        // at the view would pass while the store still said pending.
        for _ in 0..<60 where !TxRecords.pending(store: store).isEmpty {
            try? await Task.sleep(nanoseconds: 25_000_000)
        }
        let entry = try #require(tracker.view?.entries.first, "the pending row was not picked up")
        #expect(entry.userOpHash == "0xcf9f")
        #expect(entry.status == "confirmed")
        #expect(entry.txHash == "0x151d")
        // And the record on disk says so, in place.
        let row = try #require(TxRecords.load(store: store).first)
        #expect(row["status"] as? String == "confirmed")
        #expect(row["txHash"] as? String == "0x151d")
        #expect(TxRecords.pending(store: store).isEmpty)
        // The confirmation was announced exactly once.
        #expect(notified == ["0xcf9f"])
    }

    /// An unreachable bundler is not a failure.
    ///
    /// The row stays pending and the record is not patched: "we could not find
    /// out" and "it did not land" are different facts, and only one of them is
    /// safe to tell somebody about their money.
    @Test func anUnreachableBundlerLeavesTheRecordAlone() async throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        TxRecords.writeRecords([[
            "id": "rec-1", "userOpHash": "0xcf9f", "txHash": "",
            "from": golden, "to": golden, "value": "1", "symbol": "XDAI",
            "decimals": 18, "chainId": 100,
            "timestamp": Date().timeIntervalSince1970,
            "status": "pending", "type": "send",
        ]], store: store)

        // Nothing scripted: every bundler call sweeps clean.
        let executor = TrackerExecutor(
            store: store,
            relay: RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0)
        )
        let tracker = TrackerStore(executor: executor)
        tracker.boot()
        for _ in 0..<20 where tracker.view?.entries.isEmpty ?? true {
            try? await Task.sleep(nanoseconds: 25_000_000)
        }

        #expect(TxRecords.load(store: store).first?["status"] as? String == "pending")
        #expect(!TxRecords.pending(store: store).isEmpty)
    }

// MARK: - What the screen says when the core refuses

/// Every refusal reaches the screen as the core's sentence, and none of them
/// contains a raw unit.
///
/// The second half is the one that bit Android: every figure in the same-asset
/// ceiling is a **base-unit decimal string** and the shell formats it. Its
/// first cut printed `5000000000000000000 XDAI` on a phone — a true number
/// nobody can read, about somebody's own money.
@MainActor
struct SendRefusalTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// A view carrying one refusal and nothing else.
    private func view(
        warning: SendAmountWarningWire? = nil,
        sameAsset: SendFeeIssueWire? = nil,
        decimals: Int = 18
    ) -> SendViewWire {
        let json = """
        {
          "stage": "enter_details", "loading": false, "locked": false,
          "amount_locked": false, "resolving_lock": false, "adding_network": false,
          "tokens": [], "selected_token": {
            "network": "chain-100", "chain_id": 100, "symbol": "XDAI",
            "balance": "0.5", "decimals": \(decimals), "token_address": null,
            "price_usd": 1.0, "logo_urls": [], "spam": false
          },
          "recipient": "", "amount": "", "amount_fiat_code": null,
          "denom_toggle_shown": false, "denom_toggle_enabled": false,
          "denom_toggle_reason": null, "confirm_amount_issue": null,
          "token_amount": "", "confirm_amount": "",
          "split_mode": false, "recipients": [], "split_over_balance": false,
          "split_import_room": 60,
          "multi_select_mode": false, "multi_selected_ids": [],
          "multi_valuable_ids": [], "multi_chain_id": null, "multi_specs": [],
          "show_scanner": false, "show_contact_picker": false,
          "show_batch_import": false,
          "estimating_gas": false, "fee_busy": false, "fee": null,
          "gas_fee_token": null, "amount_warning": null,
          "same_asset_fee_issue": null,
          "can_continue": false, "can_confirm": false, "sending": false,
          "tx_status": "idle", "tx_error": null, "tx_hash": null,
          "user_op_hash": null, "receipt": null, "treasury_bootstrap": null,
          "recipient_identity": null, "recipient_risk": null
        }
        """
        var object = (try? CoreJSON.object(json)) ?? [:]
        if let warning {
            switch warning {
            case .notEnoughToken(let symbol):
                object["amount_warning"] = ["type": "not_enough_token", "symbol": symbol]
            case .insufficientForGas(let symbol):
                object["amount_warning"] = [
                    "type": "insufficient_for_gas",
                    "symbol": symbol.map { $0 as Any } ?? NSNull(),
                ]
            case .insufficientGas(let symbol):
                object["amount_warning"] = [
                    "type": "insufficient_gas",
                    "symbol": symbol.map { $0 as Any } ?? NSNull(),
                ]
            case .needGas(let symbol):
                object["amount_warning"] = [
                    "type": "need_gas", "symbol": symbol.map { $0 as Any } ?? NSNull(),
                ]
            case .cannotConvert(let code, let symbol):
                object["amount_warning"] = [
                    "type": "cannot_convert", "code": code, "symbol": symbol,
                ]
            }
        }
        if let sameAsset {
            object["same_asset_fee_issue"] = [
                "symbol": sameAsset.symbol,
                "transfer_amount": sameAsset.transferAmount,
                "balance": sameAsset.balance,
                "fee_amount": sameAsset.feeAmount,
                "total": sameAsset.total,
                "max_transfer_amount": sameAsset.maxTransferAmount,
            ]
        }
        // swiftlint:disable:next force_try
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }

    /// Any run of ten or more digits is a raw unit that escaped formatting.
    /// A human figure has a decimal point long before it gets that long.
    private func hasRawUnits(_ text: String) -> Bool {
        var run = 0
        for character in text {
            if character.isNumber {
                run += 1
                if run >= 10 { return true }
            } else {
                run = 0
            }
        }
        return false
    }

    @Test func everyAmountWarningHasASentence() {
        let warnings: [SendAmountWarningWire] = [
            .notEnoughToken(symbol: "XDAI"),
            .insufficientForGas(symbol: "XDAI"),
            .insufficientGas(symbol: "XDAI"),
            .needGas(symbol: "XDAI"),
            .cannotConvert(code: "CNY", symbol: "XDAI"),
        ]
        for warning in warnings {
            let text = SendLive.formWarning(view(warning: warning), loc: loc)
            let sentence = try? #require(text)
            #expect(sentence?.isEmpty == false, "a refusal with no sentence is a silent refusal")
            // And it is not a raw key left un-interpolated.
            #expect(sentence?.contains("{{") == false)
            #expect(hasRawUnits(sentence ?? "") == false)
        }
    }

    /// The same-asset ceiling, formatted.
    ///
    /// Five base-unit figures go in; five human ones must come out. This is the
    /// exact sentence Android printed in wei.
    @Test func theSameAssetCeilingIsFormattedNotPrinted() throws {
        let issue = SendFeeIssueWire(
            symbol: "XDAI",
            transferAmount: "5000000000000000000",
            balance: "5100000000000000000",
            feeAmount: "2100000000000000",
            total: "5002100000000000000",
            maxTransferAmount: "5097900000000000000"
        )
        let text = try #require(SendLive.formWarning(view(sameAsset: issue), loc: loc))
        #expect(text.contains("5 "), "the transfer amount should read as 5, not as 5e18")
        #expect(!hasRawUnits(text), "a base-unit figure reached the screen: \(text)")
        #expect(!text.contains("{{"))
    }

    /// A six-decimal token's ceiling is scaled by SIX, not by eighteen.
    ///
    /// The decimals come from the selected token, and a stablecoin read at 18
    /// renders a five-dollar fee as `0.000000000005`.
    @Test func theCeilingUsesTheTokensOwnDecimals() throws {
        let issue = SendFeeIssueWire(
            symbol: "USDC", transferAmount: "5000000", balance: "5100000",
            feeAmount: "2100", total: "5002100", maxTransferAmount: "5097900"
        )
        let text = try #require(
            SendLive.formWarning(view(sameAsset: issue, decimals: 6), loc: loc)
        )
        #expect(text.contains("5 ") || text.contains("5\u{00A0}"), "5 USDC should read as 5")
        #expect(!hasRawUnits(text))
    }

    /// Every alert the core can raise has a title and, where the corpus has
    /// one, a body — and none of them is an empty string pair.
    @Test func everyAlertKindHasWords() {
        let kinds: [[String: Any]] = [
            ["type": "invalid_address"],
            ["type": "invalid_amount"],
            ["type": "insufficient_balance", "warning": NSNull()],
            ["type": "split_over_balance"],
            ["type": "load_tokens_failed"],
            ["type": "estimate_failed", "kind": "other"],
            ["type": "account_unavailable"],
        ]
        for kind in kinds {
            let text = SendLive.alertText(kind, loc: loc)
            #expect(!text.title.isEmpty, "\(kind["type"] ?? "?") has no title")
            #expect(!text.title.contains("{{"))
            #expect(!text.body.contains("{{"))
        }
    }

    /// **A form with no token quotes no balance** (issue #209).
    ///
    /// The drawn SD2 arrives carrying the mocks' "USDT · Ethereum · Balance
    /// 53.4836", and the live form used to keep that card whenever the core
    /// had not chosen a token — which it has not while a handed-off recipient
    /// waits on the token list, and never will on an account that holds
    /// nothing. The web shell showed exactly that beside a wallet reading
    /// $0.00.
    @Test func aFormWithoutATokenShowsNoCardAndNamesNoSymbol() throws {
        let object = try CoreJSON.object(SendCore().view())
        let view = try CoreJSON.decode(SendViewWire.self, from: object)
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("sd2 is not a send form")
            return
        }
        #expect(drawn.token != nil, "the drawn card is the thing that must not survive")

        let model = SendLive.form(view, fee: nil, display: .usd, on: drawn, loc: loc)
        #expect(view.selectedToken == nil, "a fresh core has chosen nothing")
        #expect(model.token == nil, "no token, no balance to quote")
        #expect(model.header.title == loc.t("tokenDetail.send"))
    }

    /// Issue 201: the fee was the one figure on the send screens with no money
    /// beside it. The amount had its "≈" line; the fee did not, so a person who
    /// does not track the coin's price could not tell what a transfer cost.
    @Test func theFeeSaysWhatItCostsWhenSomethingCanPriceIt() throws {
        let quote = FeeEstimateWire(
            chainId: 100, totalWei: "10000000000000000", maxFeePerGas: "1",
            totalGas: "1", deployed: true, quoted: true, feeAsset: .native,
            feeRecipient: nil
        )
        // Nothing can price the coin ⇒ the coin alone, never an invented figure.
        #expect(SendLive.feeLine(quote, view: nil, fee: nil, display: .usd) == "0.01 xDAI")

        // The relay's published row prices it.
        let priced = FeeViewWire(
            busy: false, failed: nil, fee: quote, stale: false, feeToken: nil,
            options: [
                FeeOptionWire(
                    symbol: "XDAI", contract: nil, decimals: 18,
                    balance: "480000000000000000", recipient: "0x1",
                    usdBalance: "0.48", usdPrice: "1", amount: "10000000000000000",
                    insufficient: false, selected: true
                )
            ],
            confirmFeeReady: true
        )
        #expect(SendLive.feeLine(quote, view: nil, fee: priced, display: .usd) == "0.01 xDAI · ≈$0.01")

        // Under half a cent the coin amount is the honest primary: "$0.00"
        // beside a real fee reads as free.
        let dust = FeeEstimateWire(
            chainId: 100, totalWei: "1000000000000", maxFeePerGas: "1",
            totalGas: "1", deployed: true, quoted: true, feeAsset: .native,
            feeRecipient: nil
        )
        #expect(SendLive.feeLine(dust, view: nil, fee: priced, display: .usd) == "0.000001 xDAI")
    }
}

// MARK: - The three arms spec 050 and 051 left for this cut

@MainActor
struct InheritedArmTests {
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let router = "0x1111111111111111111111111111111111111111"

    private func store() -> VelaStore {
        VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
    }

    /// The send history counts SENDS, never dApp transactions.
    ///
    /// A `dapp_tx` reaches a router, a token contract or a dApp. Counting those
    /// as people would put a Uniswap router in somebody's address book and,
    /// worse, would tell the core it has "interacted before" with a contract —
    /// which is the very signal the address-poisoning warning rests on.
    @Test func theSendHistoryExcludesDappTransactions() throws {
        let store = store()
        TxRecords.writeRecords([
            [
                "id": "a", "userOpHash": "0x1", "txHash": "0x1", "from": golden,
                "to": golden, "value": "1", "symbol": "XDAI", "decimals": 18,
                "chainId": 100, "timestamp": 1_700_000_000,
                "status": "confirmed", "type": "send",
            ],
            [
                "id": "b", "userOpHash": "0x2", "txHash": "0x2", "from": golden,
                "to": router, "value": "0", "symbol": "XDAI", "decimals": 18,
                "chainId": 100, "timestamp": 1_700_000_001,
                "status": "confirmed", "type": "dapp_tx",
            ],
        ], store: store)

        let history = try #require(ContactsExecutor.sendHistory(store: store))
        #expect(history.count == 1)
        #expect((history.first?["to"] as? String) == golden)
        // Milliseconds on the wire; seconds in the store.
        #expect((history.first?["timestamp_ms"] as? Double) == 1_700_000_000_000)
    }

    /// A store that has never been written answers `nil`, not an empty list.
    ///
    /// Empty would tell the core nobody has ever been paid, and every address
    /// would then wear the first-interaction warning forever. That is why the
    /// arm answered `history_failed` for two whole cuts rather than `[]`.
    @Test func anUnwrittenStoreIsNotAnEmptyHistory() {
        #expect(ContactsExecutor.sendHistory(store: store()) == nil)
    }

    /// A contact's page shows what passed between the two of you, both ways.
    @Test func aContactsActivityIsThisDevicesOwnRecord() {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        let them = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
        let rows = ContactsLive.activityRows(
            with: them,
            records: [
                [
                    "id": "a", "from": golden, "to": them, "value": "0.0001",
                    "symbol": "XDAI", "chainId": 100,
                    "timestamp": Date().timeIntervalSince1970, "type": "send",
                ],
                [
                    "id": "b", "from": them, "to": golden, "value": "5",
                    "symbol": "XDAI", "chainId": 100,
                    "timestamp": Date().timeIntervalSince1970, "type": "receive",
                ],
                [
                    "id": "c", "from": golden, "to": router, "value": "1",
                    "symbol": "XDAI", "chainId": 100,
                    "timestamp": Date().timeIntervalSince1970, "type": "send",
                ],
            ],
            loc: loc
        )
        #expect(rows.count == 2, "somebody else's transfer reached this page")
        #expect(rows.first?.amount == "−0.0001")
        #expect(rows.first?.positive == false)
        #expect(rows.last?.amount == "+5")
        #expect(rows.last?.positive == true)
    }
}

}
