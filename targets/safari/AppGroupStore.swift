// AppGroupStore.swift — self-contained App Group helpers for the EXTENSION target.
//
// The extension links no React Native, so it cannot use modules/vela-app-group's
// RN bridge; it gets its own tiny, dependency-free copy here. This file is
// auto-compiled into the extension because targets/safari/ is a synchronized
// root group (any .swift in it is a build member — no pbxproj edit needed).
//
// The two processes share ONLY: the app-group id string + the on-disk JSON
// schema (the echo-from-* files). There is no linked Swift type across targets.
import Foundation
import CoreFoundation   // CFNotificationCenter (re-exported by Foundation; explicit for clarity)

enum AppGroupStore {

    static let appGroupID = "group.app.getvela.wallet"

    // MARK: - Container

    static var containerURL: URL? {
        FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: appGroupID)
    }

    // MARK: - Atomic JSON write (with file protection)

    /// Writes `object` as JSON to `<container>/<name>`. Atomic + protected with
    /// completeUntilFirstUserAuthentication so the app can read it in the
    /// background after first unlock since boot.
    @discardableResult
    static func writeJSON(_ object: [String: Any], to name: String) -> URL? {
        guard let dir = containerURL else { return nil }
        let url = dir.appendingPathComponent(name)
        do {
            let data = try JSONSerialization.data(
                withJSONObject: object, options: [.sortedKeys])
            try data.write(to: url, options: [
                .atomic,
                .completeFileProtectionUntilFirstUserAuthentication,
            ])
            return url
        } catch {
            return nil
        }
    }

    // MARK: - JSON read

    static func readJSON(url: URL) -> [String: Any]? {
        guard let data = try? Data(contentsOf: url),
              let obj = try? JSONSerialization.jsonObject(with: data),
              let dict = obj as? [String: Any] else { return nil }
        return dict
    }

    // MARK: - Newest file matching a prefix

    /// Newest (by modification date) file whose name starts with `prefix`.
    static func newestFile(withPrefix prefix: String) -> URL? {
        guard let dir = containerURL else { return nil }
        let fm = FileManager.default
        let items = (try? fm.contentsOfDirectory(
            at: dir,
            includingPropertiesForKeys: [.contentModificationDateKey],
            options: [.skipsHiddenFiles])) ?? []
        return items
            .filter { $0.lastPathComponent.hasPrefix(prefix) }
            .max { a, b in
                let da = (try? a.resourceValues(forKeys: [.contentModificationDateKey]))?
                    .contentModificationDate ?? .distantPast
                let db = (try? b.resourceValues(forKeys: [.contentModificationDateKey]))?
                    .contentModificationDate ?? .distantPast
                return da < db
            }
    }

    // MARK: - Darwin notifications (name-only, cross-sandbox)

    static func postDarwin(_ name: String) {
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            CFNotificationName(name as CFString),
            nil, nil, true)
    }
}
