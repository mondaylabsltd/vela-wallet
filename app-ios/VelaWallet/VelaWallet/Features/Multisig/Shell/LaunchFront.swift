//
//  LaunchFront.swift
//  VelaWallet
//
//  r16-11: a cold start begins on the account that was in front.
//

import Foundation

/// r16-11: which account a cold start begins on. The multisig in front is
/// the accounts machine's (`vela.activeMultisig`, read from its store once
/// the hub boots); until then `shownMultisig` is nil and the home drew the
/// Vela account's — Parallel Two's home for ~0.5 s before the multisig that
/// was in front. When a multisig WAS in front, the home holds (the page's
/// own ground, nothing of any account) until the hub has read it, at most
/// [holdMs] — never a flash of another account's figures.
enum LaunchFront {
    /// The longest the home holds for the hub (its store read is local).
    static let holdMs = 2_000

    /// The key the core keeps the multisig in front under (`keys::ACTIVE`).
    static let activeKey = "vela.activeMultisig"

    /// The multisig in front when the app last ran, read synchronously from
    /// the store at launch; `nil` when a Vela account was.
    static func storedMultisig(_ store: VelaStore) -> String? {
        guard let raw = store.readString(activeKey) else { return nil }
        let address = raw.trimmingCharacters(in: CharacterSet(charactersIn: "\" \n"))
        return address.isEmpty ? nil : address
    }

    /// The home holds while a multisig was in front and the hub has not read
    /// which yet, until the hold runs out.
    static func holds(storedMultisig: String?, hubLoaded: Bool, expired: Bool) -> Bool {
        storedMultisig != nil && !hubLoaded && !expired
    }
}
