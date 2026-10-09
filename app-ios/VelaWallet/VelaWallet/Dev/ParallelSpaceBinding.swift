//
//  ParallelSpaceBinding.swift
//  VelaWallet
//
//  The parallel space, implemented — Debug configurations only.
//
//  Excluded from Release by `EXCLUDED_SOURCE_FILE_NAMES`, alongside the
//  generated `vela_dev_fixtures.swift` it depends on. A Release build therefore
//  cannot link a fixture symbol even by accident: the reference would not
//  compile.
//
//  ## The door
//
//  `VELA_PARALLEL_SPACE=1` enters and persists; `=0` leaves; unset follows what
//  is persisted. `VELA_PARALLEL_SIGNER=n` picks which fixture key (0-based) the
//  space signs in with, and so which one signs every request after — entering
//  is the space's sign-in, and its record names the key the way a real sign-in
//  does (`signed_in_with`, founder 2026-09-26).
//
//  ## The account is UPSERTED, and that is not a detail
//
//  `DevAccountSeed` replaces the whole account list — correct for a read-only,
//  key-less seed on a simulator, and **forbidden here** (FR-003). This door is
//  opened on a phone that holds the founder's real wallet, and a list
//  replacement there is Android's ANDROID-8 incident with a different serial
//  number. `AccountStore.saveAccount` upserts by id; leaving removes exactly
//  that id.
//
//  Writing the record before the session machine's first event also sidesteps
//  the trap Android hit from the other direction: `session.add_account` stores
//  only the active index, and `set_wallet` would overwrite the real wallet.
//
//  ## The way out goes back to the wallet that was in front
//
//  Leaving used to save active index 0, so whichever wallet the person had
//  been on, they came out on the first one (device pass 2026-10-09, the same
//  finding as Android's). Entering now remembers the address in front beside
//  the flag (`vela.parallelReturnTo`), and leaving lands on the account in
//  front when that is a real one — the person switched to it inside the
//  space — otherwise on the remembered one, found by address because a
//  position moves when a row goes; with neither, the first.
//

#if DEBUG

import Foundation

@MainActor
final class ParallelSpaceBinding: ParallelSpaceProvider {

    /// `vela.parallelSpace` — so the space survives a relaunch the way it does
    /// on every other client, rather than needing the env pin every time.
    static let flagKey = "vela.parallelSpace"
    /// `vela.parallelSigner` — which fixture key signs.
    static let signerKey = "vela.parallelSigner"
    /// `vela.parallelReturnTo` — the address in front when the space was
    /// entered: where leaving goes back to.
    static let returnToKey = "vela.parallelReturnTo"

    private(set) var isActive = false
    private var preferredSigner: UInt32?

    /// Install the space's seam. Called once, from the app's init.
    static func install() {
        ParallelSpaceHook.provider = ParallelSpaceBinding()
    }

    func applyIfRequested(store: VelaStore, accounts: AccountStore) async {
        await apply(environment: ProcessInfo.processInfo.environment, store: store, accounts: accounts)
    }

    /// The same, with the launch environment handed in — what a test drives.
    func apply(environment: [String: String], store: VelaStore, accounts: AccountStore) async {
        let requested = environment["VELA_PARALLEL_SPACE"]
        let persisted = store.readString(Self.flagKey) == "1"

        // An explicit pin outranks the persisted flag in both directions, so a
        // scripted run can both enter AND leave without a human.
        let wanted: Bool
        switch requested {
        case "1", "true", "YES": wanted = true
        case "0", "false", "NO": wanted = false
        default: wanted = persisted
        }

        if let pinned = environment["VELA_PARALLEL_SIGNER"].flatMap(UInt32.init) {
            store.writeString(Self.signerKey, String(pinned))
        }
        preferredSigner = store.readString(Self.signerKey).flatMap(UInt32.init)

        if wanted {
            await enter(store: store, accounts: accounts, alreadyInside: persisted)
        } else if persisted || requested != nil {
            await leave(store: store, accounts: accounts)
        }
    }

    func signer() -> UserOpSigner? {
        isActive ? FixtureUserOpSigner(preferred: preferredSigner) : nil
    }

    // MARK: - Enter and leave

    /// `alreadyInside`: the flag was persisted — a relaunch inside the space,
    /// which enters again.
    private func enter(store: VelaStore, accounts: AccountStore, alreadyInside: Bool) async {
        guard let fixtures = try? fixtureAccounts(), let first = fixtures.first,
              let address = try? fixtureMultiAddress()
        else {
            print("[vela-wallet] parallel space: the keyset could not be read")
            return
        }

        // The wallet in front before the space, for the way out. On the way
        // in it is whatever was in front (none, when the space's own record
        // is); on a relaunch inside, the space's record is in front unless
        // the person switched to a real wallet before it — and that wallet is
        // where they asked to be, so it is the one to go back to.
        let inFront = Self.realAccount(
            in: await accounts.loadAccounts(), at: await accounts.loadActiveIndex(), fixtureId: first.credentialIdHex
        )
        if !alreadyInside || inFront != nil {
            store.writeString(Self.returnToKey, inFront)
        }

        // The address is a function of EVERY key, so the whole set is written.
        // A record carrying only the first would derive a different Safe — the
        // multi-passkey lesson, applied before it can bite.
        //
        // **snake_case, and that is the contract rather than a style.** The
        // core's hand-written reader accepts `publicKeyHex` / `createdAt` too,
        // but only to read a list the retired Expo client wrote; what every
        // client WRITES is snake_case (`app/mod.rs:160`). A first attempt here
        // used `createdAtISO` — which matches neither spelling — and the whole
        // account list failed to deserialize, so the app opened on Welcome with
        // the record sitting on disk. The failure is silent by design: a list
        // the core cannot read is refused rather than half-adopted.
        let signedInWith = fixtures.first { $0.index == preferredSigner } ?? first
        let record: [String: Any] = [
            "id": first.credentialIdHex,
            "name": first.name,
            "address": address,
            "public_key_hex": first.publicKeyHex,
            "created_at_iso": ISO8601DateFormatter().string(from: Date()),
            "keys": fixtures.map { account in
                [
                    "credential_id": account.credentialIdHex,
                    "public_key_hex": account.publicKeyHex,
                    "name": account.name,
                    "transports": "internal",
                ]
            },
            "signed_in_with": ["credential_id": signedInWith.credentialIdHex, "method": "platform"],
        ]
        await accounts.saveAccount(record)
        let list = await accounts.loadAccounts()
        if let index = list.firstIndex(where: { ($0["id"] as? String) == first.credentialIdHex }) {
            await accounts.saveActiveIndex(index)
        }
        store.writeString(Self.flagKey, "1")
        isActive = true
        print("[vela-wallet] parallel space: entered as \(address)")
    }

    private func leave(store: VelaStore, accounts: AccountStore) async {
        isActive = false
        let returnTo = store.readString(Self.returnToKey)
        store.writeString(Self.flagKey, nil)
        store.writeString(Self.returnToKey, nil)
        guard let first = (try? fixtureAccounts())?.first else { return }
        // A real wallet in front was chosen inside the space; otherwise the
        // one that was in front when it was entered.
        let inFront = Self.realAccount(
            in: await accounts.loadAccounts(), at: await accounts.loadActiveIndex(), fixtureId: first.credentialIdHex
        )
        await accounts.removeAccount(id: first.credentialIdHex)
        let index = Self.index(of: inFront ?? returnTo, in: await accounts.loadAccounts())
        await accounts.saveActiveIndex(index)
        print("[vela-wallet] parallel space: left, active index \(index)")
    }

    // MARK: - Which wallet is in front

    /// The address of the record at `index` when it is a real wallet — not
    /// the space's own record (`fixtureId`) — or `nil`.
    static func realAccount(in accounts: [[String: Any]], at index: Int, fixtureId: String) -> String? {
        guard accounts.indices.contains(index), (accounts[index]["id"] as? String) != fixtureId,
              let address = accounts[index]["address"] as? String, !address.isEmpty
        else { return nil }
        return address
    }

    /// The position of the record at `address`, whatever its case; 0 when it
    /// is not there, or none was given.
    static func index(of address: String?, in accounts: [[String: Any]]) -> Int {
        guard let address, !address.isEmpty else { return 0 }
        return accounts.firstIndex {
            ($0["address"] as? String)?.caseInsensitiveCompare(address) == .orderedSame
        } ?? 0
    }
}

/// The fixed keyset, signing where a passkey would.
///
/// It builds a **genuine** WebAuthn assertion — a real ECDSA P-256 signature
/// over `sha256(authenticatorData ‖ sha256(clientDataJSON))` — so the same
/// bytes verify against the Safe's on-chain P-256 verifier. A real user
/// operation from a fixture Safe settles on chain; that is the whole point.
@MainActor
struct FixtureUserOpSigner: UserOpSigner {
    let preferred: UInt32?

    func sign(
        challenge: Data,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod
    ) async throws -> Assertion {
        let signed = try fixtureAssert(
            challenge: challenge,
            allowCredentialIds: credentialIdHex.map { [$0] } ?? [],
            preferred: preferred
        )
        return Assertion(
            credentialIdHex: signed.credentialIdHex,
            signatureDerHex: signed.signatureDerHex,
            authenticatorDataHex: signed.authenticatorDataHex,
            clientDataJsonHex: signed.clientDataJsonHex,
            userIdHex: nil,
            authenticatorAttachment: "platform"
        )
    }
}

#endif
