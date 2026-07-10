import Foundation
import React

/// Reads/writes the shared App Group container and bridges Darwin notifications
/// (cross-process nudges) to React Native. Mirrors the vela-cloud-sync template
/// (RCTEventEmitter subclass) because we need native -> JS events (onDarwin).
///
/// App Group: group.app.getvela.wallet — already declared in the app entitlements
/// by plugins/with-native-modules.js -> withIOSEntitlements, and in the Safari
/// extension by targets/safari/generated.entitlements.
///
/// Error codes:
///   - APPGROUP_UNAVAILABLE — container URL nil (entitlement missing / not provisioned)
///   - APPGROUP_FAILED      — generic IO failure
@objc(VelaAppGroup)
class VelaAppGroupModule: RCTEventEmitter {

    static let appGroupID = "group.app.getvela.wallet"

    private var hasListeners = false

    /// Darwin observer callbacks are process-wide C functions that cannot capture
    /// `self`, so the active instance is parked here for the trampoline to reach.
    fileprivate static weak var shared: VelaAppGroupModule?

    /// Darwin names this instance is currently observing (so we can tear them down).
    private var observedDarwinNames = Set<String>()

    override init() {
        super.init()
        Self.shared = self
    }

    deinit {
        removeAllDarwinObservers()
    }

    // MARK: - RCTEventEmitter

    @objc override static func requiresMainQueueSetup() -> Bool { true }

    override func supportedEvents() -> [String]! {
        // Single event channel; the Darwin name rides in the body.
        return ["VelaAppGroup_darwin"]
    }

    override func startObserving() { hasListeners = true }
    override func stopObserving()  { hasListeners = false }

    // MARK: - container

    private func containerURL() -> URL? {
        FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: Self.appGroupID
        )
    }

    @objc func isSupported(
        _ resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        resolve(containerURL() != nil)
    }

    // MARK: - writeFile (atomic + protected)

    /// Writes `json` to <container>/<name> atomically (temp file + rename), with
    /// NSFileProtectionCompleteUntilFirstUserAuthentication so a background
    /// extension can read it once the device has been unlocked since boot.
    @objc func writeFile(
        _ name: String,
        json: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let base = containerURL() else {
            reject("APPGROUP_UNAVAILABLE", "App Group container not available", nil)
            return
        }
        let dest = base.appendingPathComponent(name)
        do {
            try FileManager.default.createDirectory(
                at: dest.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let data = Data(json.utf8)
            try data.write(to: dest, options: [
                .atomic,
                .completeFileProtectionUntilFirstUserAuthentication,
            ])
            resolve(nil)
        } catch {
            reject("APPGROUP_FAILED", error.localizedDescription, error)
        }
    }

    // MARK: - readFile

    @objc func readFile(
        _ name: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let base = containerURL() else {
            reject("APPGROUP_UNAVAILABLE", "App Group container not available", nil)
            return
        }
        let src = base.appendingPathComponent(name)
        guard FileManager.default.fileExists(atPath: src.path) else {
            resolve(nil)  // absent -> JS receives null
            return
        }
        do {
            let data = try Data(contentsOf: src)
            resolve(String(decoding: data, as: UTF8.self))
        } catch {
            reject("APPGROUP_FAILED", error.localizedDescription, error)
        }
    }

    // MARK: - list (subdir)

    @objc func list(
        _ dir: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let base = containerURL() else {
            reject("APPGROUP_UNAVAILABLE", "App Group container not available", nil)
            return
        }
        let target = dir.isEmpty ? base : base.appendingPathComponent(dir)
        do {
            let names = try FileManager.default.contentsOfDirectory(atPath: target.path)
            resolve(names)
        } catch {
            // Missing dir -> empty list rather than a reject (matches read's soft-miss)
            resolve([])
        }
    }

    // MARK: - remove

    @objc func remove(
        _ name: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let base = containerURL() else {
            reject("APPGROUP_UNAVAILABLE", "App Group container not available", nil)
            return
        }
        let target = base.appendingPathComponent(name)
        do {
            if FileManager.default.fileExists(atPath: target.path) {
                try FileManager.default.removeItem(at: target)
            }
            resolve(nil)
        } catch {
            reject("APPGROUP_FAILED", error.localizedDescription, error)
        }
    }

    // MARK: - Darwin notifications

    @objc func postDarwin(
        _ name: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        CFNotificationCenterPostNotification(
            center,
            CFNotificationName(name as CFString),
            nil,   // Darwin notifications are payload-less
            nil,
            true   // deliverImmediately
        )
        resolve(nil)
    }

    @objc func observeDarwin(
        _ name: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard !observedDarwinNames.contains(name) else {
            resolve(nil); return
        }
        observedDarwinNames.insert(name)

        let center = CFNotificationCenterGetDarwinNotifyCenter()
        // The callback is a bare C function pointer — it may not capture context,
        // so it reaches back through VelaAppGroupModule.shared.
        let callback: CFNotificationCallback = { _, _, cfName, _, _ in
            guard let cfName = cfName else { return }
            let eventName = cfName.rawValue as String
            DispatchQueue.main.async {
                VelaAppGroupModule.shared?.emitDarwin(name: eventName)
            }
        }
        CFNotificationCenterAddObserver(
            center,
            Unmanaged.passUnretained(self).toOpaque(),  // observer token
            callback,
            name as CFString,
            nil,
            .deliverImmediately
        )
        resolve(nil)
    }

    @objc func unobserveDarwin(
        _ name: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        CFNotificationCenterRemoveObserver(
            center,
            Unmanaged.passUnretained(self).toOpaque(),
            CFNotificationName(name as CFString),
            nil
        )
        observedDarwinNames.remove(name)
        resolve(nil)
    }

    fileprivate func emitDarwin(name: String) {
        guard hasListeners else { return }
        sendEvent(withName: "VelaAppGroup_darwin", body: ["name": name])
    }

    private func removeAllDarwinObservers() {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        CFNotificationCenterRemoveEveryObserver(
            center,
            Unmanaged.passUnretained(self).toOpaque()
        )
        observedDarwinNames.removeAll()
    }
}
