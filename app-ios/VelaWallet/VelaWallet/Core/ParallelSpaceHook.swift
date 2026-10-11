//
//  ParallelSpaceHook.swift
//  VelaWallet
//
//  The seam between the app and a test environment that does not ship.
//
//  The parallel space is the **real app with one substitution**: where a
//  passkey would sign, `vela-core`'s fixed keyset signs. Same chains, same
//  relay, same storage, same screens. It exists because every feature from spec
//  052 onward has to be verified on the founder's own iPhone, and a send ends
//  in a biometric prompt — which needs a finger, and a finger cannot be
//  scripted.
//
//  ## Why this file is always compiled and the implementation is not
//
//  A Release build must contain neither the keys nor the door (FR-001). That is
//  enforced by the build configuration: `Dev/ParallelSpaceBinding.swift` and
//  the generated `Dev/vela_dev_fixtures.swift` are in
//  `EXCLUDED_SOURCE_FILE_NAMES` for Release, and the fixtures archive is linked
//  only by Debug. So the Release binary cannot reference a fixture symbol —
//  **it would not compile if anything outside this seam tried**, which is a
//  stronger guarantee than a runtime flag.
//
//  This protocol, and the `nil` provider that is all Release ever sees, are the
//  shape that makes that possible.
//

import SwiftUI

@MainActor
protocol ParallelSpaceProvider {
    /// Whether the app is inside the space right now.
    var isActive: Bool { get }

    /// Read the launch environment and the persisted flag, and enter or leave.
    ///
    /// Called **before the session machine boots**, so the record is on disk by
    /// the time anything reads the account list.
    func applyIfRequested(store: VelaStore, accounts: AccountStore) async

    /// The signer to use while the space is active.
    func signer() -> UserOpSigner?

    /// Whether `address` is the space's own test wallet — asked of the
    /// ACCOUNT, never of `isActive` (`ParallelSpaceHook.isSpaceAccount`).
    func isSpaceAccount(_ address: String) -> Bool
}

enum ParallelSpaceHook {
    /// Installed by the Debug binding at launch. `nil` in every Release build,
    /// and in any Debug build that has not opted in.
    @MainActor static var provider: ParallelSpaceProvider?

    @MainActor static var isActive: Bool { provider?.isActive ?? false }

    @MainActor
    static func applyIfRequested(store: VelaStore, accounts: AccountStore) async {
        await provider?.applyIfRequested(store: store, accounts: accounts)
    }

    /// Whether `address` is the parallel space's test wallet.
    ///
    /// What the in-app browser tells the core about the account in front
    /// (`account_switched.space`): the core then keeps the person's connected
    /// sites apart from the space's — not re-pinned to the test wallet, not
    /// dropped when its record goes, not written at all while it is in front.
    /// A walk through the space on the owner's phone cost them a real
    /// connection (Arc Orbs, 2026-10-11) because the test wallet coming to
    /// the front was told as an ordinary account switch.
    ///
    /// Asked of the ACCOUNT, not of `isActive`. The flag says the space was
    /// entered at this launch; it does not say whose the account in front is.
    /// Inside the space the person can switch to one of their own wallets —
    /// their sites must follow them there — and back to the test wallet,
    /// which must set them aside again. The address is a fact about the
    /// build's keyset; it does not move when a preference does.
    ///
    /// `false` in every Release build: no provider, no keyset, no such account.
    @MainActor
    static func isSpaceAccount(_ address: String?) -> Bool {
        guard let address, !address.isEmpty else { return false }
        return provider?.isSpaceAccount(address) ?? false
    }

    /// The signer for a user operation: the fixture's while the space is
    /// active, otherwise the person's own passkey.
    ///
    /// Written as a fallback rather than a branch inside the send path so there
    /// is exactly ONE signing call site, and the substitution is the only
    /// difference between the two environments (FR-002).
    @MainActor
    static func signer(passkey: PasskeyExecutor) -> UserOpSigner {
        provider?.signer() ?? PasskeyUserOpSigner(passkey: passkey)
    }
}

/// The marker that answers to neither gate.
///
/// Deliberately **not a corpus string**: it is a developer marker, it must be
/// conspicuous rather than localised, and it never ships. The desktop and
/// Android made the same call.
///
/// `#if DEBUG` around the view rather than only around its caller, so the
/// literal itself is absent from a Release binary. A dormant "PARALLEL SPACE"
/// string in a shipped wallet is not key material and not a door — but it is
/// the first thing a reviewer would ask about, and the answer should be that
/// it is not there.
#if DEBUG
struct ParallelSpaceBadge: View {
    var body: some View {
        Text(verbatim: "PARALLEL SPACE")
            .font(.system(size: 10, weight: .heavy))
            .tracking(1.5)
            .foregroundStyle(.black)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 3)
            .background(Color(red: 1, green: 0.84, blue: 0))
            .allowsHitTesting(false)
    }
}
#endif
