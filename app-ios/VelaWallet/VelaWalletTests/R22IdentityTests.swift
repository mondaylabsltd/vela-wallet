//
//  R22IdentityTests.swift
//  VelaWalletTests
//
//  Round 22 "identity, standards, mailbox" (§37), the iOS shell's half —
//  each rule is the core's; these prove the shell hands the core what it
//  reads and draws what it answers:
//
//    A. identity (review 1-7): who vouched for a unit's members reaches the
//       core (`registry_unit.verified_by`); a stored account's `identity`
//       survives every write; Receive shows no address, code, copy, share or
//       request for an account the core says may not be received at, walks
//       the chain to confirm it (the sign-in walks' own `eth_call`
//       executor), and saves what the core wrote;
//    B. approvals (review 1-12): the kept-unlimited batch legs reach the
//       approve opts, the warning is drawn from the core's key, a nested
//       approval decodes;
//    C. sign-in refusal and ERC-6492 (review 1-11): a sign-in that does not
//       bind is refused in its own words; every message signature is the
//       core's `message_signature` over the account's code on the
//       request's chain — the passkey's and the trusted page's alike;
//    D. the mailbox (review 1-13): proposals no member vouches for are a
//       disclosure, each with its warning; `ownerNotBound` is worded.
//

import CryptoKit
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - A. Identity

/// The recorded chains (`registry-chain.json`, the golden key's Gnosis unit
/// 10 and Ethereum unit 0), answered from memory.
struct RecordedRegistryChains: @unchecked Sendable {
    let publicKey: String
    let answers: [String: [String: String]]

    static let shared: RecordedRegistryChains = {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let url = repoRoot
            .appendingPathComponent("app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json")
        let json = (try? JSONSerialization.jsonObject(with: Data(contentsOf: url))) as? [String: Any] ?? [:]
        return RecordedRegistryChains(
            publicKey: json["publicKey"] as? String ?? "",
            answers: json["answers"] as? [String: [String: String]] ?? [:]
        )
    }()

    /// Every chain read, by chain id.
    final class Log: @unchecked Sendable {
        private let lock = NSLock()
        private var chains: [Int] = []
        func note(_ chain: Int) { lock.lock(); chains.append(chain); lock.unlock() }
        var all: [Int] { lock.lock(); defer { lock.unlock() }; return chains }
    }

    func resolver(down: Set<Int> = [], log: Log = Log()) -> RegistryResolver {
        let answers = answers
        return RegistryResolver(ethCall: { chainId, _, data in
            log.note(chainId)
            return down.contains(chainId) ? nil : answers[String(chainId)]?[data]
        })
    }
}

/// An index that must not be asked: every request counted, none answered.
final class CountingIndex: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    var asked: Int { lock.lock(); defer { lock.unlock() }; return count }
    var transport: RegistryClient.Transport {
        { [self] _ in
            lock.lock(); count += 1; lock.unlock()
            throw URLError(.cannotConnectToHost)
        }
    }
}

private final class QuietDeps: OnboardingExecutorDeps {
    func prompt(kind: PromptKind, confirmable: Bool) async -> Bool { false }
    func complete(mode: [String: Any]) async {}
}

@MainActor
struct R22RegistryUnitTests {
    private let chains = RecordedRegistryChains.shared

    /// review 1-7: `registry_unit` carries the resolver's `verified_by`,
    /// untouched — Gnosis's here; the index alone vouches for nobody.
    @Test func aUnitReadCarriesWhoVouchedForItsMembers() async throws {
        let client = RegistryClient(
            baseURL: "https://index.test", resolver: chains.resolver(), transport: IndexScript.unreachable
        )
        _ = try await client.queryByPublicKey(chains.publicKey)
        let unit = try await client.queryUnit(10)
        #expect(unit.members.count == 3)
        #expect(unit.verifiedBy == "gnosis")

        let executor = OnboardingExecutor(
            passkey: PasskeyExecutor(),
            registry: RegistryClient(
                baseURL: "https://index.test", resolver: chains.resolver(), transport: IndexScript.unreachable
            ),
            store: AccountStore(defaults: UserDefaults(suiteName: "vela.tests.r22.unit.\(UUID().uuidString)")!),
            deps: QuietDeps()
        )
        _ = await executor.perform(["type": "registry_query_by_public_key", "public_key_hex": chains.publicKey])
        let answered = try CoreJSON.object(await executor.perform(["type": "registry_query_unit", "unit_id": 10]))
        #expect(answered["type"] as? String == "registry_unit")
        #expect(answered["verified_by"] as? String == "gnosis", "\(answered)")

        // Ethereum's backup, under its own ids.
        let backup = RegistryClient(
            baseURL: "https://index.test", resolver: chains.resolver(down: [100]), transport: IndexScript.unreachable
        )
        _ = try await backup.queryByPublicKey(chains.publicKey)
        #expect(try await backup.queryUnit(0).verifiedBy == "ethereum")

        // No resolver: the index's word alone — nobody vouched.
        let unit10 = try await client.queryUnit(10)
        let indexOnly = RegistryClient(
            baseURL: "https://index.test", resolver: nil,
            transport: IndexScript(listing: nil, unit: Self.indexShape(unit10)).transport
        )
        #expect(try await indexOnly.queryUnit(10).verifiedBy == "none")
    }

    /// The index's shape for a unit, from what the chain proved.
    private static func indexShape(_ unit: UnitDetail) -> String {
        let object: [String: Any] = [
            "unit": ["metadata": unit.metadataHex],
            "members": [
                "total": unit.members.count,
                "items": unit.members.map {
                    ["credentialId": $0.credentialIdHex, "publicKey": $0.publicKeyHex]
                },
            ],
        ]
        return String(decoding: try! JSONSerialization.data(withJSONObject: object), as: UTF8.self)
    }
}

/// The address a set of keys makes, pinned key first — the core's derivation.
@MainActor func multiKeyAddress(_ hexes: [String]) throws -> String {
    let keys = try hexes.map { hex -> P256PublicKey in
        var body = hex.hasPrefix("0x") ? String(hex.dropFirst(2)) : hex
        if body.count == 130, body.hasPrefix("04") { body = String(body.dropFirst(2)) }
        let raw = UserOpSpine.unhex(body)
        try #require(raw.count == 64, "not an uncompressed P-256 key: \(hex)")
        return P256PublicKey(x: Data(raw.prefix(32)), y: Data(raw.suffix(32)))
    }
    return try computeSafeAddressMulti(keys: keys).address
}

@MainActor
struct R22ConfirmWalkTests {
    private let chains = RecordedRegistryChains.shared

    private func client(down: Set<Int> = [], log: RecordedRegistryChains.Log = .init(), index: CountingIndex) -> RegistryClient {
        RegistryClient(baseURL: "https://index.test", resolver: chains.resolver(down: down, log: log), transport: index.transport)
    }

    private func unit10Members() async throws -> [String] {
        let probe = RegistryClient(
            baseURL: "https://index.test", resolver: chains.resolver(), transport: IndexScript.unreachable
        )
        _ = try await probe.queryByPublicKey(chains.publicKey)
        return try await probe.queryUnit(10).members.map(\.publicKeyHex)
    }

    /// The chain holds the key's group and it makes this address: confirmed,
    /// by Gnosis — and the index is never asked.
    @Test func theGroupOnChainMakesThisAddress_confirmed() async throws {
        let address = try multiKeyAddress(try await unit10Members())
        let index = CountingIndex()
        let log = RecordedRegistryChains.Log()
        let verdict = await client(log: log, index: index).confirmIdentity(publicKeyHex: chains.publicKey, address: address)
        #expect(verdict == IdentityVerdict(identity: "confirmed", verifiedBy: "gnosis"))
        #expect(index.asked == 0, "the confirm walk asks the chain alone")
        #expect(!log.all.isEmpty && Set(log.all).isSubset(of: [100, 1]))
    }

    /// The chain holds the key's group and it makes ANOTHER address: refuted.
    @Test func theGroupOnChainMakesAnotherAddress_refuted() async throws {
        let index = CountingIndex()
        let verdict = await client(index: index).confirmIdentity(
            publicKeyHex: chains.publicKey, address: "0x1111111111111111111111111111111111111111"
        )
        #expect(verdict.identity == "refuted", "\(verdict)")
        #expect(verdict.verifiedBy != "none")
        #expect(index.asked == 0)
    }

    /// No chain answers: unconfirmed, nobody vouched — asked again later.
    @Test func noChainAnswers_unconfirmed() async throws {
        let address = try multiKeyAddress(try await unit10Members())
        let index = CountingIndex()
        let verdict = await client(down: [100, 1], index: index)
            .confirmIdentity(publicKeyHex: chains.publicKey, address: address)
        #expect(verdict == .unconfirmed)
        #expect(index.asked == 0)
        // And with no resolver at all: nothing to ask.
        let bare = RegistryClient(baseURL: "https://index.test", resolver: nil, transport: index.transport)
        #expect(await bare.confirmIdentity(publicKeyHex: chains.publicKey, address: address) == .unconfirmed)
    }
}

/// A stored multi-key account, as the core writes one.
private func storedAccount(
    address: String = "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
    identity: String? = "unconfirmed",
    firstKey: String = "04" + String(repeating: "11", count: 64)
) -> [String: Any] {
    var record: [String: Any] = [
        "id": "cred-1", "name": "Two keys", "address": address,
        "public_key_hex": firstKey, "created_at_iso": "2026-11-08T00:00:00.000Z",
        "keys": [
            ["credential_id": "cred-1", "public_key_hex": firstKey, "name": "One", "transports": "internal"],
            ["credential_id": "cred-2", "public_key_hex": "04" + String(repeating: "22", count: 64),
             "name": "Two", "transports": "internal"],
        ],
    ]
    if let identity { record["identity"] = identity }
    return record
}

private func json(_ object: [String: Any]) -> String {
    String(decoding: try! JSONSerialization.data(withJSONObject: object), as: UTF8.self)
}

@MainActor
struct R22AccountIdentityTests {

    private func store() -> AccountStore {
        AccountStore(defaults: UserDefaults(suiteName: "vela.tests.r22.identity.\(UUID().uuidString)")!)
    }

    /// review 1-7: `identity` survives every door a record is written
    /// through — the store, the onboarding machines' save, the session's —
    /// a re-save that dropped it would silently confirm the account.
    @Test func identityRoundTripsThroughEveryWrite() async throws {
        for identity in ["unconfirmed", "refuted"] {
            let store = store()
            await store.saveAccount(storedAccount(identity: identity))
            #expect(await store.loadAccounts().first?["identity"] as? String == identity)

            let onboarding = OnboardingExecutor(
                passkey: PasskeyExecutor(),
                registry: RegistryClient(baseURL: "https://r.test", transport: IndexScript.unreachable),
                store: store, deps: QuietDeps()
            )
            _ = await onboarding.perform(["type": "save_account", "account": storedAccount(identity: identity)])
            #expect(await store.loadAccounts().first?["identity"] as? String == identity)

            let session = SessionExecutor(store: store)
            _ = await session.perform(["type": "save_account", "account": storedAccount(identity: identity)])
            let loaded = try CoreJSON.object(await session.perform(["type": "load_accounts"]))
            let accounts = try #require(loaded["accounts"] as? [[String: Any]])
            #expect(accounts.count == 1)
            #expect(accounts.first?["identity"] as? String == identity)
        }
        // Confirmed is never written.
        let store = store()
        await store.saveAccount(storedAccount(identity: nil))
        #expect(await store.loadAccounts().first?["identity"] == nil)
    }

    /// The core's standing for each identity, as the shell decodes it.
    @Test func theStandingIsTheCores() throws {
        let unconfirmed = ReceiveStandingWire.of(accountJson: json(storedAccount(identity: "unconfirmed")))
        #expect(!unconfirmed.receivable)
        #expect(unconfirmed.identity == "unconfirmed")
        #expect(unconfirmed.titleKey == "receive.unconfirmed.title")
        #expect(unconfirmed.bodyKey == "receive.unconfirmed.body")
        #expect(unconfirmed.checkFirst)

        let refuted = ReceiveStandingWire.of(accountJson: json(storedAccount(identity: "refuted")))
        #expect(!refuted.receivable && !refuted.checkFirst)
        #expect(refuted.titleKey == "receive.refuted.title" && refuted.bodyKey == "receive.refuted.body")

        let confirmed = ReceiveStandingWire.of(accountJson: json(storedAccount(identity: nil)))
        #expect(confirmed.receivable && !confirmed.checkFirst && confirmed.titleKey == nil)

        // An identity this build cannot read is the strictest it can — never confirmed.
        let odd = ReceiveStandingWire.of(accountJson: json(storedAccount(identity: "maybe")))
        #expect(!odd.receivable)

        // A record the core cannot read: no address either.
        #expect(!ReceiveStandingWire.of(accountJson: "{}").receivable)
    }
}

/// The confirm walk, scripted, with what it was asked.
final class ScriptedConfirm: @unchecked Sendable {
    private let lock = NSLock()
    private var asked: [(String, String)] = []
    var verdict: IdentityVerdict
    init(_ verdict: IdentityVerdict) { self.verdict = verdict }
    var calls: [(publicKeyHex: String, address: String)] {
        lock.lock(); defer { lock.unlock() }; return asked.map { ($0.0, $0.1) }
    }
    func confirm(_ key: String, _ address: String) -> IdentityVerdict {
        lock.lock(); defer { lock.unlock() }
        asked.append((key, address))
        return verdict
    }
}

@MainActor
struct R22ReceiveGateTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func rig(_ verdict: IdentityVerdict, identity: String? = "unconfirmed")
        async -> (ReceiveStandingStore, AccountStore, ScriptedConfirm) {
        let store = AccountStore(defaults: UserDefaults(suiteName: "vela.tests.r22.receive.\(UUID().uuidString)")!)
        await store.saveAccount(storedAccount(identity: identity))
        let scripted = ScriptedConfirm(verdict)
        let gate = ReceiveStandingStore(accounts: store, confirm: { key, address in
            scripted.confirm(key, address)
        })
        return (gate, store, scripted)
    }

    /// Opening Receive on an unconfirmed account walks the chain FIRST — with
    /// the account's first key and its own address — and a chain that
    /// confirms it saves the core's record (no `identity`) and opens Receive.
    @Test func openingReceiveChecksFirstAndAConfirmationIsSaved() async throws {
        let (gate, store, scripted) = await rig(IdentityVerdict(identity: "confirmed", verifiedBy: "gnosis"))
        #expect(!gate.receivable(address), "not read yet: no address")
        await gate.open(address)
        #expect(scripted.calls.count == 1)
        #expect(scripted.calls.first?.publicKeyHex == "04" + String(repeating: "11", count: 64))
        #expect(scripted.calls.first?.address == address)
        #expect(gate.receivable(address))
        let saved = try #require(await store.loadAccounts().first)
        #expect(saved["identity"] == nil, "confirmed is the absence of the field")
        #expect((saved["keys"] as? [[String: Any]])?.count == 2, "nothing else touched")
        // Confirmed: opening again asks the chain nothing.
        await gate.open(address)
        #expect(scripted.calls.count == 1)
    }

    /// A chain that proves it another wallet's: refuted, saved, never
    /// received at, and never checked again.
    @Test func aRefutationIsSavedAndNeverCheckedAgain() async throws {
        let (gate, store, scripted) = await rig(IdentityVerdict(identity: "refuted", verifiedBy: "gnosis"))
        await gate.open(address)
        #expect(await store.loadAccounts().first?["identity"] as? String == "refuted")
        #expect(!gate.receivable(address))
        #expect(gate.standing(of: address)?.titleKey == "receive.refuted.title")
        await gate.open(address)
        #expect(scripted.calls.count == 1)
    }

    /// No chain answered: nothing saved, Receive stays shut, Check again
    /// asks again.
    @Test func noVerdictChangesNothingAndCheckAgainAsksAgain() async throws {
        let (gate, store, scripted) = await rig(.unconfirmed)
        await gate.open(address)
        #expect(!gate.receivable(address))
        #expect(await store.loadAccounts().first?["identity"] as? String == "unconfirmed")
        await gate.check(address)
        #expect(scripted.calls.count == 2)
        scripted.verdict = IdentityVerdict(identity: "confirmed", verifiedBy: "ethereum")
        await gate.check(address)
        #expect(gate.receivable(address))
    }

    /// App start: the active account, unconfirmed, is re-checked before
    /// first use.
    @Test func appStartReChecksAnUnconfirmedActiveAccount() async throws {
        let (gate, _, scripted) = await rig(IdentityVerdict(identity: "confirmed", verifiedBy: "gnosis"))
        await gate.recheckActive()
        #expect(scripted.calls.count == 1)
        #expect(gate.receivable(address))

        let (quiet, _, untouched) = await rig(.unconfirmed, identity: nil)
        await quiet.recheckActive()
        #expect(untouched.calls.isEmpty, "a confirmed account is not checked")
        #expect(quiet.receivable(address))
    }

    /// An address that is no stored account here (a multisig in front): the
    /// rule does not speak for it.
    @Test func anAddressThatIsNoAccountIsNotGated() async throws {
        let (gate, _, scripted) = await rig(.unconfirmed)
        let multisig = "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"
        await gate.open(multisig)
        #expect(gate.receivable(multisig))
        #expect(scripted.calls.isEmpty)
    }

    /// The list Receive draws: no row, no address — the core's sentence, with
    /// Check again, or "Checking on-chain…" while the walk is out.
    @Test func theBlockedListShowsNoAddress() throws {
        let fixture = try #require({ () -> ReceiveListModel? in
            if case .receive(let list) = WalletFlowFixtures.build(.r1, loc: loc).base { return list }
            return nil
        }())
        let live = FlowsLive.receiveList(address, on: fixture, loc: loc)
        #expect(!live.rows.isEmpty && live.address == address && live.blocked == nil)

        let unconfirmed = ReceiveStandingWire.of(accountJson: json(storedAccount(identity: "unconfirmed")))
        let blocked = FlowsLive.receiveBlocked(unconfirmed, checking: false, on: live, loc: loc)
        #expect(blocked.rows.isEmpty && blocked.address.isEmpty && blocked.subtitle.isEmpty)
        #expect(blocked.blocked == ReceiveBlockedModel(
            title: loc.t("receive.unconfirmed.title"), body: loc.t("receive.unconfirmed.body"),
            checkAgain: loc.t("receive.unconfirmed.checkAgain"), checking: nil
        ))
        #expect(blocked.blocked?.title == "Not yet confirmed on-chain")

        let checking = FlowsLive.receiveBlocked(unconfirmed, checking: true, on: live, loc: loc)
        #expect(checking.blocked?.checkAgain == nil)
        #expect(checking.blocked?.checking == loc.t("receive.unconfirmed.checking"))

        let refuted = FlowsLive.receiveBlocked(
            ReceiveStandingWire.of(accountJson: json(storedAccount(identity: "refuted"))),
            checking: false, on: live, loc: loc
        )
        #expect(refuted.blocked?.title == loc.t("receive.refuted.title"))
        #expect(refuted.blocked?.checkAgain == nil, "a refuted wallet is not checked again")

        let unread = FlowsLive.receiveBlocked(nil, checking: false, on: live, loc: loc)
        #expect(unread.rows.isEmpty && unread.blocked?.title == nil)
        #expect(unread.blocked?.checking == loc.t("receive.unconfirmed.checking"))
    }
}

// MARK: - B. Approvals

@MainActor
struct R22ApprovalTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let token = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"
    private let wallet = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private var unlimited: String {
        "0x095ea7b3" + String(repeating: "0", count: 24) + String(repeating: "1", count: 40)
            + String(repeating: "f", count: 64)
    }

    private func view(_ core: ApprovalGuardCore, _ event: [String: Any]) throws -> GuardViewWire {
        _ = try core.dispatch(eventJson: CoreJSON.string(event))
        return try CoreJSON.decode(GuardViewWire.self, from: try CoreJSON.object(core.view()))
    }

    /// review 1-12: a batch whose leg keeps its unlimited approval as asked
    /// names that leg — and the approve opts carry it, verbatim.
    @Test func aKeptUnlimitedLegIsNamedInTheApproveOpts() throws {
        let core = ApprovalGuardCore()
        let transfer = "0xa9059cbb" + String(repeating: "0", count: 24) + String(repeating: "2", count: 40)
            + String(format: "%064x", 1)
        let batch = try view(core, [
            "type": "approval_detected",
            "method": "wallet_sendCalls",
            "params_json": #"[{"version":"2.0.0","chainId":"0x64","from":"\#(wallet)","calls":[{"to":"\#(token)","data":"\#(transfer)"},{"to":"\#(token)","data":"\#(unlimited)"}]}]"#,
            "chain_id": 100,
            "wallet_address": wallet,
            "read_only": false,
            "now_ms": 1_757_000_000_000,
        ])
        #expect(batch.surface == .batch)
        #expect(batch.unlimitedConsented)
        #expect(batch.unlimitedConsentedLegs == [1], "\(String(describing: batch.unlimitedConsentedLegs))")
        let opts = SigningController.approveOpts(fee: nil, clear: .empty, guard: batch)
        #expect((opts["unlimited_approved_legs"] as? [NSNumber])?.map(\.intValue) == [1])
        #expect(opts["unlimited_approved"] as? Bool == true)
        // Nothing kept: none named.
        let none = SigningController.approveOpts(fee: nil, clear: .empty, guard: .empty)
        #expect((none["unlimited_approved_legs"] as? [NSNumber])?.isEmpty == true)
        // The warning key the core names.
        #expect(batch.unlimitedWarningKey == "componentsUi.signing.unlimitedWarning")
    }

    /// An approval carried inside a multicall is detected, and where it sits
    /// decodes (`nested_at`).
    @Test func aNestedApprovalDecodes() throws {
        let inner = String(unlimited.dropFirst(2))
        let innerBytes = inner.count / 2
        let padded = inner + String(repeating: "0", count: (32 - innerBytes % 32) % 32 * 2)
        let multicall = "0xac9650d8"
            + String(format: "%064x", 0x20) + String(format: "%064x", 1)
            + String(format: "%064x", 0x20) + String(format: "%064x", innerBytes) + padded
        let core = ApprovalGuardCore()
        let view = try view(core, [
            "type": "approval_detected",
            "method": "eth_sendTransaction",
            "params_json": #"[{"to":"\#(token)","data":"\#(multicall)"}]"#,
            "chain_id": 100, "wallet_address": wallet, "read_only": false, "now_ms": 1_757_000_000_000,
        ])
        let detected = try #require(view.detected, "the approval inside the multicall is seen")
        #expect(detected.isUnbounded)
        #expect((detected.nestedAt ?? 0) > 0, "nested_at \(String(describing: detected.nestedAt))")
        // A plain one has none.
        let plain = try CoreJSON.decoder.decode(GuardDetectedApprovalWire.self, from: Data(#"""
        {"kind":"erc20_approve","token_address":null,"spender":"0x1","amount_raw":"1","amount_bits":256,
         "is_unbounded":false,"is_boolean_grant":false,"is_reducing":false,"editable":true,
         "block_reason":null,"deadline":null,"locus":{"type":"calldata_word","word_index":1}}
        """#.utf8))
        #expect(plain.nestedAt == nil)
    }

    /// The warning is drawn from the key the core names — "over 100× your
    /// balance" for a finite cap far above it — and the unlimited sentence
    /// when an older core names none.
    @Test func theWarningIsDrawnFromTheCoresKey() {
        var guardView = GuardViewWire(
            surface: .approvalEditor, detected: nil,
            meta: GuardTokenMetaViewWire(symbol: "USDC", decimals: 6, verified: true, loading: false),
            editor: nil, confirmAllowed: true, rewrittenParamsJson: nil, unlimitedConsented: false,
            unlimitedWarning: true, unlimitedWarningKey: "componentsUi.signing.farAboveBalanceWarning",
            increaseTotal: nil, decimalsUnverified: false, expired: false, batch: nil
        )
        func warnings() -> [String] {
            SigningLive.guardBlocks(guardView, loc: loc).compactMap { block in
                if case .warning(.danger, let text) = block { return text }
                return nil
            }
        }
        #expect(warnings() == [loc.t("componentsUi.signing.farAboveBalanceWarning")])
        #expect(warnings().first?.contains("100×") == true)
        guardView.unlimitedWarningKey = nil
        #expect(warnings() == [loc.t("componentsUi.signing.unlimitedWarning")])
    }
}

// MARK: - C. Sign-in refusal

@MainActor
struct R22SignInRefusalTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private static func hex(_ text: String) -> String {
        "0x" + Data(text.utf8).map { String(format: "%02x", $0) }.joined()
    }

    /// review 1-11: a sign-in for ANOTHER site is refused outright — its own
    /// title and the core's sentence, never the self-call text; no confirm,
    /// nothing signed; the site hears -32603 `sign_in_refused`.
    @Test func aSignInForAnotherSiteIsRefusedInItsOwnWords() async throws {
        let rig = ParentSafeRig(multisigs: [])
        let message = """
        bank.example wants you to sign in with your Ethereum account:
        \(ParentSafeRig.signer)

        Sign in to the bank.

        URI: https://bank.example/login
        Version: 1
        Chain ID: \(ParentSafeRig.chain)
        Nonce: abcdefgh12
        Issued At: 2026-11-08T00:00:00.000Z
        """
        rig.controller.open(SigningController.Incoming(
            id: "siwe-1", method: "personal_sign",
            paramsJson: #"["\#(Self.hex(message))","\#(ParentSafeRig.signer)"]"#,
            origin: "https://evil.example", transportId: "tab-1", chainId: ParentSafeRig.chain
        ))
        let blocked = try #require(rig.controller.sign.blocked, "the sheet opens refused")
        let signIn = try #require(blocked.signIn)
        #expect(blocked.function == "signIn")
        #expect(signIn.key == "componentsUi.signing.signIn.wrongDomain")
        #expect(rig.controller.sign.error?.kind == .signInRefused)
        #expect(!rig.controller.confirmState.enabled)

        let blocks = SigningLive.statusBlocks(sign: rig.controller.sign, loc: loc)
        let texts = blocks.compactMap { block -> String? in
            switch block {
            case .intent(let text, _): text
            case .warning(_, let text): text
            default: nil
            }
        }
        #expect(texts.first == loc.t("componentsUi.signing.signIn.refusedTitle"))
        #expect(texts.contains(loc.t(signIn.key, vars: signIn.params)))
        #expect(texts.contains { $0.contains("bank.example") && $0.contains("evil.example") }, "\(texts)")
        #expect(!texts.contains(loc.t("componentsUi.signing.selfCallBlockedTitle")))

        rig.controller.approve()
        await rig.settle()
        #expect(rig.nothingSigned)
        rig.controller.reject()
        await rig.until { !rig.answers.isEmpty }
        let payload = rig.answers.last?.payload
        #expect((payload?["code"] as? NSNumber)?.intValue == -32603)
        #expect(payload?["kind"] as? String == "sign_in_refused")
    }

    @Test func theKindAndTheBlockDecode() throws {
        let kinds = try CoreJSON.decoder.decode([SignErrorKind].self, from: Data(#"["sign_in_refused"]"#.utf8))
        #expect(kinds == [.signInRefused])
        let blocked = try CoreJSON.decoder.decode(SignBlockedViewWire.self, from: Data(#"""
        {"function":"signIn","selector":"","leg_index":null,"nested":false,
         "sign_in":{"reason":"wrong_chain","key":"componentsUi.signing.signIn.wrongChain",
                    "params":{"chain":"1","connected":"100"}}}
        """#.utf8))
        #expect(blocked.signIn == SignInRefusalWire(
            reason: "wrong_chain", key: "componentsUi.signing.signIn.wrongChain",
            params: ["chain": "1", "connected": "100"]
        ))
        let plain = try CoreJSON.decoder.decode(SignBlockedViewWire.self, from: Data(#"""
        {"function":"execTransaction","selector":"6a761202","leg_index":null,"nested":false}
        """#.utf8))
        #expect(plain.signIn == nil)
    }
}

// MARK: - C. ERC-6492

extension ScriptedRelayPort {
    /// A relay whose chain says the account holds code — a message signature
    /// then passes through `messageSignature` as built. For the tests about
    /// WHERE a message is signed (the page, the key), whose fixture address
    /// is not one their keys make, so no ERC-6492 envelope could deploy it.
    static func deployedAccount() -> ScriptedRelayPort {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080604052")
        return port
    }
}

/// A relay that answers `eth_getCode` as told and records on which chain.
@MainActor
final class CodeReadPort: RelayPort {
    var code: RpcOutcome
    private(set) var codeReads: [(chainId: Int, address: String)] = []
    init(code: RpcOutcome) { self.code = code }

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        if method == "eth_getCode" {
            codeReads.append((chainId, params.first as? String ?? ""))
            return code
        }
        return .failed(rateLimited: false)
    }
    func bundlerBase(chainId: Int) async -> String? { nil }
    func bestRpcUrl(chainId: Int) async -> String? { nil }
    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer { .failed }
}

@MainActor
struct R22MessageSignatureTests {
    private let fixture = TrustedSignerFixture()
    /// ERC-6492's magic suffix.
    private static let magic = String(repeating: "6492", count: 16)
    private let chain = 8453
    /// The account the fixture's key makes — the envelope's factory call
    /// must deploy THIS address, or the core refuses to wrap.
    private var account: String {
        let raw = fixture.key.publicKey.rawRepresentation
        return (try? computeSafeAddress(x: Data(raw.prefix(32)), y: Data(raw.suffix(32))).address) ?? ""
    }

    private func spine(_ port: CodeReadPort, page: ScriptedTrustedSigner? = nil) -> UserOpSpine {
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        if page != nil { accounts.recordJson = fixture.pageRecordJson }
        let spine = UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { P256AssertionSigner(fixture) }
        )
        if let page { spine.trustedSigner = page }
        return spine
    }

    private func sign(_ spine: UserOpSpine) async throws -> String {
        try await spine.signMessage(
            chainId: chain, account: account, originalHash: Data(repeating: 7, count: 32),
            asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
        )
    }

    /// An account with no code on the request's chain signs in ERC-6492's
    /// envelope; one with code, the plain EIP-1271 signature; one whose code
    /// could not be read, the envelope (the core wraps). The code is read on
    /// the REQUEST's chain, for the account itself.
    @Test func theSignatureFollowsTheAccountsCodeOnTheRequestsChain() async throws {
        let undeployed = CodeReadPort(code: .ok("0x"))
        let wrapped = try await sign(spine(undeployed))
        #expect(wrapped.hasSuffix(Self.magic), "an undeployed account's signature is wrapped")
        #expect(undeployed.codeReads.map(\.chainId) == [chain])
        #expect(undeployed.codeReads.first?.address.lowercased() == account.lowercased())

        let deployed = CodeReadPort(code: .ok("0x6080604052"))
        let plain = try await sign(spine(deployed))
        #expect(!plain.hasSuffix(Self.magic), "a deployed account's signature is left as it is")
        #expect(plain.count < wrapped.count)

        let unread = CodeReadPort(code: .failed(rateLimited: false))
        #expect(try await sign(spine(unread)).hasSuffix(Self.magic), "an unread code is wrapped")
    }

    /// The trusted page's answer reaches the same door.
    @Test func theTrustedPagesSignatureIsWrappedToo() async throws {
        let fixture = fixture
        let page = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let undeployed = CodeReadPort(code: .ok("0x"))
        let signature = try await sign(spine(undeployed, page: page))
        #expect(page.asked.count == 1, "the page signed it")
        #expect(signature.hasSuffix(Self.magic))
        #expect(undeployed.codeReads.map(\.chainId) == [chain])

        let deployed = CodeReadPort(code: .ok("0x6080604052"))
        #expect(!(try await sign(spine(deployed, page: page))).hasSuffix(Self.magic))
    }
}

// MARK: - D. The mailbox's words

@MainActor
struct R22MailboxWordsTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    @Test func theDisclosureAndTheWarningAreTheCoresKeys() throws {
        var view = MsProposalsView()
        let words = MsWords(loc: loc, hub: nil, localNames: [:])
        #expect(MultisigHomeCard.nonMembersDisclosure(view, words: words) == nil, "none: no row")
        view.nonMembers = 2
        #expect(MultisigHomeCard.nonMembersDisclosure(view, words: words)
                == loc.t("home.multisig.panel.nonMembersShow", vars: ["total": "2"]))
        #expect(MultisigHomeCard.nonMembersDisclosure(view, words: words)?.contains("(2)") == true)
        view.showingNonMembers = true
        #expect(MultisigHomeCard.nonMembersDisclosure(view, words: words)
                == loc.t("home.multisig.panel.nonMembersHide"))

        let decoded = try JSONDecoder().decode(MsProposalsView.self, from: Data(#"""
        {"non_members":3,"showing_non_members":true,"items":[{"chainId":100,"safeTxHash":"0xab","fromNonMember":true}]}
        """#.utf8))
        #expect(decoded.nonMembers == 3 && decoded.showingNonMembers)
        #expect(decoded.items.first?.fromNonMember == true)
    }

    @Test func ownerNotBoundIsWorded() {
        let words = MsWords(loc: loc, hub: nil, localNames: [:])
        let address = "0x1234567890123456789012345678901234567890"
        let line = words.finding(.object([
            "from": .string("genesis"), "chainId": .number(100),
            "reason": .object(["kind": .string("ownerNotBound"), "address": .string(address)]),
        ]))
        #expect(line == loc.t("home.multisig.authority.ownerNotBound", vars: [
            "network": words.network(100), "address": address,
        ]))
        #expect(line.contains(address))
    }
}

// MARK: - R21-3 (§37): a test build's passkey domain, said as that

@MainActor
struct R213TestBuildPasskeyWordsTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private static let domain = "com.apple.AuthenticationServices.AuthorizationError"
    private static let notAssociated =
        "Application with identifier ABCDE12345.app.getvela.wallet is not associated with domain localhost"

    /// The simulator's test build (RP = localhost) failing 1004 "not
    /// associated" is the test domain not set up — never the release
    /// build's "check your connection… update Vela". A release build's words
    /// are unchanged.
    @Test func aTestBuildsNotAssociatedFailureIsTheTestDomainsSentence() {
        let test = PasskeyErrorFacts(domain: Self.domain, code: 1004, message: Self.notAssociated, testBuild: true)
        #expect(test.words.key == "onboarding.common.passkeyError.testDomainNotAssociated")
        #expect(loc.t(test.words.key) != test.words.key, "the sentence is in the catalog")

        let release = PasskeyErrorFacts(domain: Self.domain, code: 1004, message: Self.notAssociated, testBuild: false)
        #expect(release.words.key == "onboarding.common.passkeyError.appNotAssociated")
        // The same failure either way: only the words differ.
        #expect(test.words.failure == release.words.failure)
    }

    /// The flag is the one this build hands `relying_party_id`, and it rides
    /// on every facts object the shell sends the core (`passkey_failed.error`).
    @Test func theFactsCarryThisBuildsTestFlag() {
        let facts = PasskeyErrorFacts.of(NSError(domain: Self.domain, code: 1004))
        #expect(facts.testBuild == PasskeyRelyingParty.testBuild)
        #expect(PasskeyRelyingParty.testBuild, "the unit tests run a Debug build — a test build")
        #expect(facts.wire["test_build"] as? Bool == PasskeyRelyingParty.testBuild)
        #expect(PasskeyErrorFacts(domain: "", code: nil, message: "x", testBuild: false).wire["test_build"] as? Bool == false)

        // Through the executor's door: the facts it classifies, and the
        // `passkey_failed` the core is answered, say it.
        let error = NSError(domain: Self.domain, code: 1004, userInfo: [NSLocalizedDescriptionKey: Self.notAssociated])
        let classified = PasskeyExecutor.classify(error)
        #expect(classified.facts?.testBuild == true)
        #expect(classified.facts?.words.key == "onboarding.common.passkeyError.testDomainNotAssociated")
        let failed = OnboardingExecutor.failure(for: ["type": "register_passkey"], error: error)
        #expect(failed["type"] as? String == "passkey_failed")
        #expect((failed["error"] as? [String: Any])?["test_build"] as? Bool == true)
    }
}
