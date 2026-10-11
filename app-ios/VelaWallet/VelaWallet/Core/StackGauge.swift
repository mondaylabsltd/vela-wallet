//
//  StackGauge.swift
//  VelaWallet
//
//  How much of the main thread's stack the router has used by the time it
//  builds a pushed screen.
//
//  An iPhone gives the main thread ONE megabyte (about 1,008 KB usable). A
//  simulator gives it eight. And an unoptimised build keeps every temporary
//  of a SwiftUI body on the stack: each modifier applied to a view makes a
//  new value that holds the whole view under it, and `RootView`'s router is
//  four such bodies called one inside the other — `body`, `appBody`,
//  `content(router:)`, `signedInOrWelcome` — before `flowScreen` asks
//  `flowModel` for a screen's model.
//
//  On the owner's iPhone 11 (2026-10-11, a Debug build) opening Send died
//  there: EXC_BAD_ACCESS at the stack guard, seventy frames deep, no
//  recursion, in `Loc.t` under `WalletFlowFixtures.sendForm`. Measured on a
//  simulator, where the same frames are the same size and merely have room:
//
//      at entry to …        before      after erasing three views
//      appBody              119.6 KB     75.0 KB
//      content(router:)     410.3 KB    225.6 KB
//      signedInOrWelcome    739.9 KB    488.5 KB
//      flowModel            946.0 KB    694.5 KB
//      sendForm           1,009.1 KB    757.7 KB    ← the phone's limit is ~1,008
//
//  It had been within a few kilobytes of the limit for some time; a handful
//  of new modifiers at the root took it over. The three `AnyView`s in
//  `RootView` (around the router's content, the stack's root and a pushed
//  flow screen) each replace a value the modifiers above it would copy with
//  a reference. Each wraps ONE function's result, so the erased type never
//  changes and no identity is lost.
//
//  Nothing on a simulator can crash this way, so nothing there used to
//  notice. This gauge is how a simulator notices: with `VELA_STACK_GAUGE`
//  naming a file (Debug builds only), the depth at each `note` is appended
//  to it, and `StackDepthUITests` holds the router to a budget.
//

import Foundation

enum StackGauge {
    #if DEBUG
    /// The file the depths are written to, when a test asked for them.
    private static let path: String? = {
        let path = ProcessInfo.processInfo.environment["VELA_STACK_GAUGE"] ?? ""
        return path.isEmpty ? nil : path
    }()
    #endif

    /// Record how deep the stack is here. Free unless a test asked.
    @inline(never)
    static func note(_ what: String) {
        #if DEBUG
        guard let path else { return }
        let line = "\(what) \(used())\n"
        guard let data = line.data(using: .utf8) else { return }
        if let handle = FileHandle(forWritingAtPath: path) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: data)
        } else {
            FileManager.default.createFile(atPath: path, contents: data)
        }
        #endif
    }

    /// Bytes of this thread's stack in use at the call.
    @inline(never)
    static func used() -> Int {
        var here = 0
        let top = UInt(bitPattern: pthread_get_stackaddr_np(pthread_self()))
        return withUnsafePointer(to: &here) { Int(top &- UInt(bitPattern: $0)) }
    }
}
