//
//  VelaWalletApp.swift
//  VelaWallet
//

import SwiftUI

@main
struct VelaWalletApp: App {
    private let loc = Loc()

    init() {
        // The parallel space's seam, installed before any view exists. The
        // binding — and the keyset it reaches — are excluded from Release
        // builds by the build configuration, so this call has nothing to
        // install there and the symbols are not in the binary at all.
        #if DEBUG
        ParallelSpaceBinding.install()
        #endif
    }

    var body: some Scene {
        WindowGroup {
            RootView(loc: loc)
        }
    }
}
