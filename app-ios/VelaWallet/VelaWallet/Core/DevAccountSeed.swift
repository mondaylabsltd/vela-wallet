//
//  DevAccountSeed.swift
//  VelaWallet
//
//  An address to read for, without a passkey ceremony (spec 051 research D3).
//
//  `VELA_ACCOUNT=0x…` writes a **key-less** account record so the read path has
//  somebody's holdings to fetch. That is legitimate for reads and would not be
//  for signing, and the difference is structural rather than a promise:
//
//  - reading a balance needs an **address** and nothing else;
//  - signing needs a **key**, and the whole point of this wallet is that the key
//    lives in the Secure Enclave and cannot be seeded.
//
//  So the record carries an empty `public_key_hex`. It can fund every read in
//  spec 051, and if spec 052 ever leaned on it the failure is a **refusal** —
//  no key, no signature — rather than a wrong signature. FR-010 exists so this
//  is never inherited as a way to skip a ceremony.
//
//  DEBUG-only and env-gated, like every other `VELA_*` pin. It is not production
//  navigation and there is no code path to it from the app.
//

import Foundation

enum DevAccountSeed {

    /// The one id this seed ever writes, so it can recognise — and remove —
    /// its own record without touching a real one.
    static let devId = "dev-read-only"

    /// Seed the account list if `VELA_ACCOUNT` names an address.
    ///
    /// Called before the session machine boots, because the session reads
    /// `vela.accounts` on its first event and a record written after that would
    /// not be seen until a relaunch.
    static func applyIfRequested(store: VelaStore) {
        #if DEBUG
        let raw = ProcessInfo.processInfo.environment["VELA_ACCOUNT"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)

        // **The pin's account lives and dies with the pin.**
        //
        // It used to only ever be written, and that is a trap rather than a
        // convenience: the record persists, so the NEXT launch without the pin
        // is still signed in as a key-less account. The XCUITest suite found it
        // immediately — two onboarding tests failed because the app booted
        // into a wallet nobody had asked for — and the same thing would happen
        // to a person who ran the pin once on their own device.
        //
        // FR-010 says this must never be inherited as a way to skip a
        // ceremony. Removing it here is what makes that structural instead of
        // a promise.
        guard let address = raw, address.hasPrefix("0x"), address.count == 42 else {
            let existing = store.readList(VelaStore.Key.accounts)
            if existing.contains(where: { ($0["id"] as? String) == devId }) {
                store.remove(VelaStore.Key.accounts)
                store.remove(VelaStore.Key.activeIndex)
                print("[vela-wallet] VELA_ACCOUNT is unset — the seeded read-only account was removed")
            }
            return
        }

        store.writeList(VelaStore.Key.accounts, [[
            "id": devId,
            "name": "Dev (read-only)",
            "address": address,
            // Empty on purpose. See the file comment: this is what makes the
            // record unable to sign rather than merely unlikely to.
            "public_key_hex": "",
            "created_at_iso": "1970-01-01T00:00:00Z",
            "keys": [],
        ]])
        store.writeString(VelaStore.Key.activeIndex, nil)
        print("[vela-wallet] VELA_ACCOUNT seeded a READ-ONLY account: \(address)")
        #endif
    }
}
