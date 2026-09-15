//
//  ExploreExecutor.swift
//  VelaWallet
//
//  Two executors, five operations, one store each.
//
//  `explore_sites` keeps one JSON document under `vela.explore`; the
//  `browser_history` keeps a JSON array under `vela.browserHistory`. Both are
//  the documents the other three clients read, so the key names and the shapes
//  are not this file's to choose.
//
//  Neither machine gets a judgement from here. An unreadable document is
//  answered as **absent**, not as a failure a person has to dismiss, because
//  both cores treat "nothing stored" as a legitimate starting state and a
//  browser that refused to open because its favourites file was corrupt would
//  be worse than one that starts empty.
//

import Foundation

@MainActor
final class ExploreExecutor {

    static let operations = ["read_explore", "write_explore"]

    static let key = "vela.explore"

    private let store: VelaStore

    init(store: VelaStore) {
        self.store = store
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "read_explore":
            let doc = store.readObject(Self.key)
            // An empty object is not a document. The core reads `null` as
            // "never written" and builds its own default; handing it `{}`
            // would make it deserialise a document with no fields.
            return CoreJSON.string([
                "type": "loaded",
                "doc": doc.isEmpty ? NSNull() : doc as Any,
            ])

        case "write_explore":
            if let doc = operation["doc"] as? [String: Any] {
                store.writeObject(Self.key, doc)
            }
            return CoreJSON.string(["type": "written"])

        default:
            print("[vela-wallet] explore_sites: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "read_explore": return CoreJSON.string(["type": "loaded", "doc": NSNull()])
        default: return CoreJSON.string(["type": "written"])
        }
    }
}

/// The browser's recents.
///
/// ## `onLoaded` is not a convenience
///
/// `browser_history` publishes no `ready` flag, and a visit dispatched before
/// `read_history` has answered is **dropped silently**. So the executor tells
/// its owner when the load lands, and the owner records nothing before then.
/// Android found this on a device, in its first phase, with a browser that
/// remembered nothing about the first page of every session.
@MainActor
final class BhistExecutor {

    static let operations = ["read_history", "write_history", "remove_history"]

    static let key = "vela.browserHistory"

    private let store: VelaStore

    /// Fired once, when the mirror is live.
    var onLoaded: () -> Void = {}
    private var announced = false

    init(store: VelaStore) {
        self.store = store
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "read_history":
            // A corrupt or absent list answers `[]` — the TS `catch { [] }`,
            // ported. Distinct from `explore`'s `null` because the core's own
            // doc says so for each.
            let entries = store.readList(Self.key)
            defer { announce() }
            return CoreJSON.string(["type": "loaded", "entries": entries])

        case "write_history":
            store.writeList(Self.key, operation["entries"] as? [[String: Any]] ?? [])
            return CoreJSON.string(["type": "written"])

        case "remove_history":
            // **Deletes the key**, rather than writing `[]`. Ported verbatim
            // from `browser-history.ts`, and the other three clients do the
            // same: "cleared" and "empty" are different facts on disk.
            store.remove(Self.key)
            return CoreJSON.string(["type": "written"])

        default:
            print("[vela-wallet] browser_history: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    /// Even a failed read makes the mirror live — the core is `Ready` either
    /// way, and an owner waiting for a load that errored would wait forever.
    ///
    /// **Deferred by one main-actor turn, on purpose.** This runs while the
    /// answer is still on its way back to the core: announcing synchronously
    /// would let the owner record a visit into a machine that has not yet
    /// accepted the load, which is the exact drop this whole mechanism exists
    /// to prevent.
    private func announce() {
        guard !announced else { return }
        announced = true
        Task { @MainActor [onLoaded] in onLoaded() }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "read_history": return CoreJSON.string(["type": "loaded", "entries": []])
        default: return CoreJSON.string(["type": "written"])
        }
    }
}
