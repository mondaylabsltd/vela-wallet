//
//  DevLaunchInputs.swift
//  VelaWallet
//
//  Three launch inputs that point a Debug build at local services — the web
//  harness's `useHarness` (e2e/onchain/harness.ts), for a simulator or an
//  XCUITest (native parity F0.5). Debug configurations only.
//
//  Excluded from Release by `EXCLUDED_SOURCE_FILE_NAMES`, beside the parallel
//  space's binding, and called only under `#if DEBUG`: a Release binary holds
//  neither the code nor the three names (`app-ios/scripts/check-ios-store-binary.sh`
//  reads the archived binary to prove it).
//
//  ## The inputs
//
//  - `VELA_SERVICE_ENDPOINTS` — a JSON object merged into
//    `vela.serviceEndpoints` (its stored camelCase names, or the core's
//    snake_case ones): `{"bundlerServiceURL":"http://127.0.0.1:9090",
//    "genesisServiceURL":"http://127.0.0.1:9087"}`. A `null` or `""` value
//    removes that endpoint (the default is used again).
//  - `VELA_RPC_OVERRIDES` — a JSON object, chain id → RPC URL, or chain id →
//    `{ "rpcURL", "onlyMine"?, "explorerURL"?, "bundlerURL"? }`, upserted into
//    `vela.networkConfig` as the person's own node for that network:
//    `{"100":"http://127.0.0.1:9045","8453":"http://127.0.0.1:9046"}`. A
//    `null` removes that chain's override.
//  - `VELA_PEERCONNECT_RELAY` — the PeerConnect relay an outside signer's
//    session goes through, stored at `vela.peerconnectRelay` as the web keeps
//    it. Nothing on iOS opens a PeerConnect session yet (multisig is the
//    web's until F0.4): it is stored so the first one that does finds it.
//
//  Applied in the app's init, before the object graph — and so before any
//  machine has read a store. Persisted, like the parallel space: a relaunch
//  without the inputs keeps them, and the Settings screens show (and reset)
//  them like any other value. Malformed JSON stops the launch with the
//  reason: a harness pointed nowhere must not run on the real services.
//

#if DEBUG

import Foundation

@MainActor
enum DevLaunchInputs {

    static let serviceEndpointsVar = "VELA_SERVICE_ENDPOINTS"
    static let rpcOverridesVar = "VELA_RPC_OVERRIDES"
    static let peerconnectRelayVar = "VELA_PEERCONNECT_RELAY"
    /// The web's key for the relay (harness.ts `vela.peerconnectRelay`).
    static let peerconnectRelayKey = "vela.peerconnectRelay"
    /// Native parity N-I3: a link the app is "opened with" at launch, through
    /// the very handler a universal link reaches (`openLink`). A simulator
    /// build is unsigned, so iOS never hands it a universal link of its own;
    /// this is how a UI test opens one. Debug only, like every input here.
    static let openLinkVar = "VELA_OPEN_LINK"

    /// The launch link, when one was given and it is a URL.
    static var launchLink: URL? {
        nonEmpty(ProcessInfo.processInfo.environment[openLinkVar]).flatMap(URL.init(string:))
    }

    /// Read the launch environment and write what it names. Once, from the
    /// app's init.
    static func install() {
        do {
            try apply(environment: ProcessInfo.processInfo.environment, defaults: .standard)
        } catch {
            fatalError("[vela-wallet] \(error)")
        }
    }

    struct InputError: Error, CustomStringConvertible {
        let description: String
    }

    /// What a test drives: the environment and the shelf handed in.
    static func apply(environment: [String: String], defaults: UserDefaults) throws {
        if let raw = nonEmpty(environment[serviceEndpointsVar]) {
            try applyServiceEndpoints(raw, defaults: defaults)
            print("[vela-wallet] dev: service endpoints from \(serviceEndpointsVar)")
        }
        if let raw = nonEmpty(environment[rpcOverridesVar]) {
            try applyRpcOverrides(raw, store: VelaStore(defaults: defaults))
            print("[vela-wallet] dev: RPC overrides from \(rpcOverridesVar)")
        }
        if let relay = nonEmpty(environment[peerconnectRelayVar]) {
            guard URL(string: relay)?.scheme != nil else {
                throw InputError(description: "\(peerconnectRelayVar)=\(relay) is not a URL")
            }
            defaults.set(relay, forKey: peerconnectRelayKey)
            print("[vela-wallet] dev: PeerConnect relay from \(peerconnectRelayVar)")
        }
    }

    // MARK: - Service endpoints

    /// The core's wire names, to the stored ones every client reads.
    static let endpointNames: [String: String] = [
        "ethereum_data_url": "ethereumDataURL",
        "passkey_index_url": "passkeyIndexURL",
        "bundler_service_url": "bundlerServiceURL",
        "fiat_rates_url": "fiatRatesURL",
        "genesis_service_url": "genesisServiceURL",
    ]

    static func applyServiceEndpoints(_ raw: String, defaults: UserDefaults) throws {
        let given = try object(raw, name: serviceEndpointsVar)
        let known = Set(endpointNames.values)
        var stored: [String: Any] = [:]
        if let text = defaults.string(forKey: VelaStore.Key.serviceEndpoints),
           let parsed = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] {
            stored = parsed
        }
        for (key, value) in given {
            let name = endpointNames[key] ?? key
            guard known.contains(name) else {
                throw InputError(description: "\(serviceEndpointsVar): unknown endpoint `\(key)`")
            }
            if let url = value as? String, !url.isEmpty {
                guard URL(string: url)?.scheme != nil else {
                    throw InputError(description: "\(serviceEndpointsVar): `\(key)` is not a URL")
                }
                stored[name] = url
            } else {
                stored.removeValue(forKey: name)
            }
        }
        let data = try JSONSerialization.data(withJSONObject: stored)
        defaults.set(String(decoding: data, as: UTF8.self), forKey: VelaStore.Key.serviceEndpoints)
    }

    // MARK: - RPC overrides

    static func applyRpcOverrides(_ raw: String, store: VelaStore) throws {
        let given = try object(raw, name: rpcOverridesVar)
        var configs = store.readList(VelaStore.Key.networkConfig)
        for (key, value) in given {
            guard let chainId = Int(key), chainId > 0 else {
                throw InputError(description: "\(rpcOverridesVar): `\(key)` is not a chain id")
            }
            let at = configs.firstIndex { ($0["chainId"] as? NSNumber)?.intValue == chainId }
            if value is NSNull {
                if let at { configs.remove(at: at) }
                continue
            }
            var record: [String: Any] = at.map { configs[$0] } ?? [
                "chainId": chainId, "rpcURL": "", "explorerURL": "", "bundlerURL": "",
            ]
            switch value {
            case let url as String:
                record["rpcURL"] = url
            case let fields as [String: Any]:
                for name in ["rpcURL", "explorerURL", "bundlerURL"] {
                    if let text = fields[name] as? String { record[name] = text }
                }
                if let only = fields["onlyMine"] as? Bool {
                    if only { record["onlyMine"] = true } else { record.removeValue(forKey: "onlyMine") }
                }
            default:
                throw InputError(description: "\(rpcOverridesVar): chain \(key) is neither a URL nor an object")
            }
            guard let url = record["rpcURL"] as? String, URL(string: url)?.scheme != nil else {
                throw InputError(description: "\(rpcOverridesVar): chain \(key) names no RPC URL")
            }
            if let at { configs[at] = record } else { configs.append(record) }
        }
        store.writeList(VelaStore.Key.networkConfig, configs)
    }

    // MARK: - Parsing

    private static func nonEmpty(_ value: String?) -> String? {
        guard let value = value?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty
        else { return nil }
        return value
    }

    private static func object(_ raw: String, name: String) throws -> [String: Any] {
        guard let parsed = try? JSONSerialization.jsonObject(with: Data(raw.utf8)) as? [String: Any] else {
            throw InputError(description: "\(name) is not a JSON object")
        }
        return parsed
    }
}

#endif
