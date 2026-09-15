//
//  BuildInfo.swift
//  VelaWallet
//
//  What build this is — for 关于, and for a bug report that names it.
//
//  Android reads `BuildConfig.VERSION_NAME` / `GIT_COMMIT`. iOS reads its own
//  bundle, and the commit is a build setting rather than a `git` call, because
//  `ENABLE_USER_SCRIPT_SANDBOXING = YES` forbids a build phase from reading
//  `.git` (see `Info.plist`). A release passes it:
//
//      xcodebuild archive … VELA_GIT_COMMIT=$(git rev-parse --short HEAD)
//
//  With no commit the page says `build 42` — which is true, checkable against
//  the App Store record, and not a hash nobody can look up.
//

import Foundation

enum BuildInfo {
    /// `CFBundleShortVersionString` — the marketing version, "1.0".
    static var version: String {
        string("CFBundleShortVersionString") ?? "0.0"
    }

    /// `CFBundleVersion` — the build number.
    static var build: String {
        string("CFBundleVersion") ?? "0"
    }

    /// The short commit, or `build <n>`.
    static var commit: String {
        let stamped = string("VelaGitCommit")
        if let stamped, stamped != "unknown", !stamped.hasPrefix("$(") {
            return stamped
        }
        return "build \(build)"
    }

    private static func string(_ key: String) -> String? {
        guard let value = Bundle.main.object(forInfoDictionaryKey: key) as? String,
              !value.isEmpty
        else { return nil }
        return value
    }
}
