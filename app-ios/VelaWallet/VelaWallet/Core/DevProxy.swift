//
//  DevProxy.swift
//  VelaWallet
//
//  Every connection this app makes, through one fault proxy (spec 082).
//
//  `VELA_DEV_PROXY=<host>:<port>` sends the dApp browser's pages AND the
//  wallet's own traffic — RPC, relay, registry, prices, logos, probes — through
//  that HTTP CONNECT proxy (`scripts/device/chaos-proxy.py`), so a bad-network
//  row hits this app and nothing else. The phone's own proxy setting is never
//  touched: that would put every other app on the phone through the fault too.
//
//  ## Nothing falls back to a direct connection
//
//  `ProxyConfiguration` fails over only when asked to, and it is not asked: a
//  proxy that is down is a network that is down, which is the point. A value
//  that is not `host:port` stops the launch — a fault row that quietly went
//  direct would pass for the wrong reason.
//
//  ## Why the sessions are made here
//
//  `URLSession.vela(_:)` and `URLSession.velaShared` are every session the app
//  opens. In Release they are `URLSession(configuration:)` and
//  `URLSession.shared`, exactly: the proxy is `#if DEBUG`, so a distribution
//  build has nothing to reach — not even the `Network` import.
//
//  ## What cannot be routed
//
//  The trusted signer's `SFSafariViewController` is Safari's process and
//  follows only the device's proxy; passkey ceremonies run in system daemons.
//

import Foundation
#if DEBUG
import Network
import WebKit
#endif

extension URLSession {

    /// `URLSession(configuration:delegate:delegateQueue:)` — through
    /// `VELA_DEV_PROXY` in a Debug build that sets it.
    nonisolated static func vela(
        _ configuration: URLSessionConfiguration,
        delegate: URLSessionDelegate? = nil,
        delegateQueue: OperationQueue? = nil
    ) -> URLSession {
        #if DEBUG
        if let proxy = DevProxy.configuration { configuration.proxyConfigurations = [proxy] }
        #endif
        return URLSession(configuration: configuration, delegate: delegate, delegateQueue: delegateQueue)
    }

    /// `URLSession.shared`. Its configuration cannot be changed, so a Debug
    /// build with `VELA_DEV_PROXY` set answers with a session of its own.
    nonisolated static var velaShared: URLSession {
        #if DEBUG
        if let proxied = DevProxy.sharedSession { return proxied }
        #endif
        return .shared
    }
}

#if DEBUG
nonisolated enum DevProxy {

    /// `VELA_DEV_PROXY` as given, or `nil` when it is unset or empty.
    static let address: String? = {
        let raw = ProcessInfo.processInfo.environment["VELA_DEV_PROXY"]?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return raw.isEmpty ? nil : raw
    }()

    static let configuration: ProxyConfiguration? = {
        guard let address else { return nil }
        guard let colon = address.lastIndex(of: ":"),
              let number = UInt16(address[address.index(after: colon)...]), number > 0,
              let port = NWEndpoint.Port(rawValue: number)
        else { fatalError("[vela-wallet] VELA_DEV_PROXY=\(address) is not <host>:<port>") }
        // `[::1]:8899` — the brackets are the URL's, not the host's.
        let host = address[..<colon].trimmingCharacters(in: CharacterSet(charactersIn: "[]"))
        guard !host.isEmpty else { fatalError("[vela-wallet] VELA_DEV_PROXY=\(address) has no host") }

        var proxy = ProxyConfiguration(httpCONNECTProxy: .hostPort(host: NWEndpoint.Host(host), port: port))
        proxy.allowFailover = false
        return proxy
    }()

    /// What `URLSession.velaShared` answers while the proxy is set.
    static let sharedSession: URLSession? = configuration.map { _ in URLSession.vela(.default) }

    /// Called once from the app's init, before any web view exists. Every tab
    /// uses `WKWebsiteDataStore.default()`, so this one store covers them all.
    @MainActor static func install() {
        guard let configuration, let address else { return }
        WKWebsiteDataStore.default().proxyConfigurations = [configuration]
        print("[vela-wallet] dev proxy: all traffic via \(address)")
    }
}
#endif
