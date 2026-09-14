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
//  is persisted. `VELA_PARALLEL_SIGNER=n` picks which of the fixture keys
//  signs, as the web's `vela.parallel.signWith(n)` does.
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

#if DEBUG

import Foundation

@MainActor
final class ParallelSpaceBinding: ParallelSpaceProvider {

    /// `vela.parallelSpace` — so the space survives a relaunch the way it does
    /// on every other client, rather than needing the env pin every time.
    static let flagKey = "vela.parallelSpace"
    /// `vela.parallelSigner` — which fixture key signs.
    static let signerKey = "vela.parallelSigner"

    private(set) var isActive = false
    private var preferredSigner: UInt32?

    /// Install the space's seam. Called once, from the app's init.
    static func install() {
        ParallelSpaceHook.provider = ParallelSpaceBinding()
    }

    func applyIfRequested(store: VelaStore, accounts: AccountStore) async {
        let environment = ProcessInfo.processInfo.environment
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
            await enter(store: store, accounts: accounts)
        } else if persisted || requested != nil {
            await leave(store: store, accounts: accounts)
        }
    }

    func signer() -> UserOpSigner? {
        isActive ? FixtureUserOpSigner(preferred: preferredSigner) : nil
    }

    // MARK: - Enter and leave

    private func enter(store: VelaStore, accounts: AccountStore) async {
        guard let fixtures = try? fixtureAccounts(), let first = fixtures.first,
              let address = try? fixtureMultiAddress()
        else {
            print("[vela-wallet] parallel space: the keyset could not be read")
            return
        }

        // The address is a function of EVERY key, so the whole set is written.
        // A record carrying only the first would derive a different Safe — the
        // multi-passkey lesson, applied before it can bite.
        let record: [String: Any] = [
            "id": first.credentialIdHex,
            "name": first.name,
            "address": address,
            "publicKeyHex": first.publicKeyHex,
            "createdAtISO": ISO8601DateFormatter().string(from: Date()),
            "keys": fixtures.map { account in
                [
                    "credentialId": account.credentialIdHex,
                    "publicKeyHex": account.publicKeyHex,
                    "transports": "internal",
                ]
            },
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
        store.writeString(Self.flagKey, nil)
        guard let first = (try? fixtureAccounts())?.first else { return }
        await accounts.removeAccount(id: first.credentialIdHex)
        await accounts.saveActiveIndex(0)
        print("[vela-wallet] parallel space: left")
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
