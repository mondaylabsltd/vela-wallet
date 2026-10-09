//
//  DeviceSafety.swift
//  VelaWalletUITests
//
//  What a UI test may do to the phone it runs on.
//
//  `VELA_ACCOUNT` seeds a key-less read-only account by REPLACING the whole
//  account list, and the next launch without it removes that list. On a
//  simulator that is a fixture; on the owner's iPhone it is their wallets. The
//  app honours the pin on a simulator only (`DevAccountSeed`), so a test that
//  needs it is skipped on a device rather than run against an account it never
//  got — and it says why.
//

import XCTest

enum DeviceSafety {
    /// The test runs on a simulator — never somebody's phone.
    static var onSimulator: Bool {
        #if targetEnvironment(simulator)
        return true
        #else
        return false
        #endif
    }
}

extension XCTestCase {
    /// Skip this test on a device: it seeds `VELA_ACCOUNT`, which a device
    /// build ignores and which would replace the phone's own accounts.
    func skipOnDeviceForSeededAccount() throws {
        try XCTSkipUnless(
            DeviceSafety.onSimulator,
            "seeds VELA_ACCOUNT, which replaces the account list: simulator only"
        )
    }
}
