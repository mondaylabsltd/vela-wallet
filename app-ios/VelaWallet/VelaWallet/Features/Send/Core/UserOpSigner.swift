//
//  UserOpSigner.swift
//  VelaWallet
//
//  The one seam the core cannot have: a challenge in, an assertion out.
//
//  Ported from `app-android/.../feature/send/core/UserOpSigner.kt` (spec 043
//  research D9). The send path hands over the **SafeOp hash** — never a
//  WebAuthn signing hash. The core packs the assertion into the operation's
//  signature and the Safe verifies the client data it finds there; a shell that
//  pre-hashed would sign the wrong bytes and learn about it from a reverted
//  transaction.
//
//  Outside the parallel space the answer comes from the person's passkey
//  through the ceremony onboarding already owns. Inside it, from the core's
//  fixed keyset.
//
//  **Exactly one call per attempt.** The executor cancels the signing task on
//  `cancel_passkey_sign`; a second prompt after a cancel is the defect spec 052
//  User Story 3 names, and the test for it counts calls on this protocol rather
//  than reading the code and hoping.
//

import Foundation

@MainActor
protocol UserOpSigner {
    /// - Parameters:
    ///   - challenge: the 32-byte SafeOp hash.
    ///   - credentialIdHex: the pinned credential — the wallet's first key — or
    ///     `nil` for a discoverable ceremony.
    ///   - transports: the pinned key's stored transports (`"internal"`,
    ///     `"hybrid,internal"`, `"usb,nfc"`…), load-bearing for routing.
    ///   - method: outranks the transport hints. A caBLE credential carries the
    ///     wide hint set, and without this a person who had just signed in by
    ///     scanning a QR was told to plug in a security key they never owned.
    func sign(
        challenge: Data,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod
    ) async throws -> Assertion
}

/// The real thing: the same ceremony sign-in uses, with the same rpId.
@MainActor
struct PasskeyUserOpSigner: UserOpSigner {
    let passkey: PasskeyExecutor

    func sign(
        challenge: Data,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod
    ) async throws -> Assertion {
        try await passkey.assert(
            challenge: challenge,
            credentialIdHex: credentialIdHex,
            transports: transports,
            method: method
        )
    }
}
