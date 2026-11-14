//
//  ProductionWriteGuard.swift
//  VelaWallet
//
//  §28 (docs/multisig-machines.md): a test build never writes to production.
//
//  Round 17: a Debug simulator build creating a throwaway passkey account
//  wrote it to the production key index (`p256-index-v2.getvela.app`),
//  because nobody had pointed `passkeyIndexURL` at a local stand-in. A
//  checklist did not stop it; the build does now.
//
//  ## The rule is the core's
//
//  `service_defaults::write_verdict`: in a TEST build (every Debug build —
//  the simulator's, the parallel space's, the "Vela Test" device-test build,
//  `….devicetest`), a write (any method but GET/HEAD/OPTIONS; a JSON-RPC
//  POST asking only reads is a read) to a production host (`*.getvela.app`
//  and the defaults' hosts) is refused before it leaves the device, logged,
//  and said honestly ("This test build won't write to the live service;
//  point it at a local one."). Reads stay allowed.
//
//  ## Where it stands
//
//  A URL protocol: registered with the URL loading system (so
//  `URLSession.shared`, which `velaShared` is) and put first in every
//  session `URLSession.vela(_:)` makes — every session this app opens. A
//  request it may not judge on its method and host alone (a POST to a
//  production host) it takes, reads the body, asks the core, and either
//  fails it or sends it on unchanged.
//
//  `VELA_ALLOW_PRODUCTION_WRITES=1` in the launch environment is the opt-in,
//  off by default on the simulator; a Debug build on a real device is opted
//  in (`onDevice`, owner 2026-11-14).
//
//  Excluded from Release by `EXCLUDED_SOURCE_FILE_NAMES` and called only
//  under `#if DEBUG`: a store binary holds neither this code nor the opt-in's
//  name (`app-ios/scripts/check-ios-store-binary.sh`).
//

#if DEBUG

import Foundation
import VelaCore

nonisolated final class ProductionWriteGuard: URLProtocol, @unchecked Sendable {

    /// What the guard judges by. The app's is [thisBuild]; a test sets its own.
    struct Policy: Sendable, Equatable {
        var testBuild: Bool
        var allowProductionWrites: Bool
    }

    static let optInVar = "VELA_ALLOW_PRODUCTION_WRITES"

    /// This build: Debug, and the device-test build by its id — a test build
    /// by the core's `is_test_build`. The opt-in from the environment.
    static let thisBuild = Policy(
        testBuild: isTestBuild(
            devFixtures: true,
            deviceTest: Bundle.main.bundleIdentifier?.hasSuffix(".devicetest") ?? false,
            debug: true
        ),
        allowProductionWrites: onDevice
            || productionWritesOptIn(value: ProcessInfo.processInfo.environment[optInVar])
    )

    /// A Debug build on a real iPhone or iPad writes to production like the
    /// store app (owner, 2026-11-14): only the team can sign one, its
    /// passkeys are getvela.app's (§35, revised the same day), and creating
    /// and signing in must work as they will ship — publishing to the key
    /// index included. The simulator's builds — dev fixtures, throwaway keys,
    /// the Round 17 key-index write — keep refusing without the opt-in.
    static var onDevice: Bool {
        #if targetEnvironment(simulator)
        false
        #else
        true
        #endif
    }

    private static let lock = NSLock()
    nonisolated(unsafe) private static var current = thisBuild

    /// The policy in force.
    static var policy: Policy {
        get { lock.withLock { current } }
        set { lock.withLock { current = newValue } }
    }

    /// Register with the URL loading system. Once, from the app's init.
    static func install() {
        guard policy.testBuild else { return }
        URLProtocol.registerClass(ProductionWriteGuard.self)
        if policy.allowProductionWrites {
            print("[vela-wallet] guard: production writes allowed (\(optInVar)=1)")
        }
    }

    /// `configuration` with the guard first among its protocols.
    static func guarding(_ configuration: URLSessionConfiguration) {
        guard policy.testBuild else { return }
        let others = (configuration.protocolClasses ?? []).filter { $0 != ProductionWriteGuard.self }
        configuration.protocolClasses = [ProductionWriteGuard.self] + others
    }

    /// Marks a request the guard already let through.
    private static let passedKey = "vela.productionWriteGuard.passed"

    /// The production host `request` is refused for, or `nil`. [body] is
    /// read only when the method and host leave the answer open.
    static func refusal(_ request: URLRequest, policy: Policy, body: () -> Data?) -> String? {
        guard policy.testBuild, !policy.allowProductionWrites, let url = request.url?.absoluteString
        else { return nil }
        let method = request.httpMethod ?? "GET"
        let text = mayNeedBody(request) ? body().flatMap { String(data: $0, encoding: .utf8) } : nil
        return productionWriteRefusal(
            testBuild: policy.testBuild,
            allowProductionWrites: policy.allowProductionWrites,
            method: method,
            url: url,
            body: text
        )
    }

    /// A write to a production host: what the guard must take to judge.
    private static func claims(_ request: URLRequest, policy: Policy) -> Bool {
        guard policy.testBuild, !policy.allowProductionWrites,
              URLProtocol.property(forKey: passedKey, in: request) == nil,
              let host = request.url?.host, isProductionHost(host: host)
        else { return false }
        let method = (request.httpMethod ?? "GET").uppercased()
        return !["GET", "HEAD", "OPTIONS"].contains(method)
    }

    private static func mayNeedBody(_ request: URLRequest) -> Bool {
        (request.httpMethod ?? "GET").uppercased() == "POST"
    }

    // MARK: URLProtocol

    override class func canInit(with request: URLRequest) -> Bool {
        claims(request, policy: policy)
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    private var forwarded: URLSessionDataTask?

    override func startLoading() {
        let body = Self.bodyData(request)
        if let host = Self.refusal(request, policy: Self.policy, body: { body }) {
            let error = ProductionWriteRefused.error(host: host)
            print("[vela-wallet] guard: refused \(request.httpMethod ?? "?") \(request.url?.absoluteString ?? "?") — \(error.localizedDescription)")
            client?.urlProtocol(self, didFailWithError: error)
            return
        }
        // A read after all (a JSON-RPC read): sent on, unchanged.
        guard let passed = (request as NSURLRequest).mutableCopy() as? NSMutableURLRequest else {
            client?.urlProtocol(self, didFailWithError: URLError(.unknown))
            return
        }
        passed.httpBodyStream = nil
        passed.httpBody = body
        URLProtocol.setProperty(true, forKey: Self.passedKey, in: passed)
        let task = Self.forwarding.dataTask(with: passed as URLRequest) { [weak self] data, response, error in
            guard let self else { return }
            if let error {
                self.client?.urlProtocol(self, didFailWithError: error)
                return
            }
            if let response {
                self.client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            }
            if let data { self.client?.urlProtocol(self, didLoad: data) }
            self.client?.urlProtocolDidFinishLoading(self)
        }
        forwarded = task
        task.resume()
    }

    override func stopLoading() {
        forwarded?.cancel()
    }

    /// Where a read the guard took goes out: the app's own session shape (a
    /// test points it at its loopback stand-in).
    nonisolated(unsafe) private static var forwardingSession: URLSession?
    static var forwarding: URLSession {
        get {
            if let session = lock.withLock({ forwardingSession }) { return session }
            // Made outside the lock: `vela(_:)` reads the policy, under it.
            let made = URLSession.vela(.ephemeral)
            return lock.withLock {
                if let session = forwardingSession { return session }
                forwardingSession = made
                return made
            }
        }
        set { lock.withLock { forwardingSession = newValue } }
    }

    /// The request's body, whether it came as data or as a stream.
    static func bodyData(_ request: URLRequest) -> Data? {
        if let body = request.httpBody { return body }
        guard let stream = request.httpBodyStream else { return nil }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 16_384)
        while stream.hasBytesAvailable {
            let read = stream.read(&buffer, maxLength: buffer.count)
            if read <= 0 { break }
            data.append(buffer, count: read)
        }
        return data
    }
}

/// A test build's write to a production service, refused before it left.
nonisolated enum ProductionWriteRefused {
    static let domain = "app.getvela.VelaWallet.ProductionWriteRefused"

    /// The error the transports read as "never sent": `.cannotConnectToHost`
    /// (the core's not-sent class), carrying the honest sentence and the host.
    static func error(host: String) -> URLError {
        URLError(.cannotConnectToHost, userInfo: [
            NSLocalizedDescriptionKey: "\(productionWriteRefusedEnglish()) (\(host))",
            "host": host,
            "productionWriteRefused": true,
        ])
    }

    /// Is `error` the guard's refusal.
    static func matches(_ error: Error) -> Bool {
        (error as? URLError)?.errorUserInfo["productionWriteRefused"] as? Bool == true
            || (error as NSError).userInfo["productionWriteRefused"] as? Bool == true
    }
}

#endif
