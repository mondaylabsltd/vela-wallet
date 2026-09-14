//
//  NetworkAdminExecutor.swift
//  VelaWallet
//
//  The only place the `network_admin` core touches the outside world on iOS.
//
//  Ported from `app-web/vela-wallet/src/lib/settings/core/network-admin-executor.ts`
//  (spec 024). Same sixteen operations, same codecs, same probes, same stored
//  bytes.
//
//  ## Why the probes live here rather than waiting for 051's pool
//
//  They look like the RPC pool's job and they are not. The core's add gate is
//  real — `add_confirmed` refuses a candidate whose compatibility was never
//  verified — so a probe-less settings screen could never add a network at all.
//  The pool exists to ROUTE repeated reads of a person's own money, with a ban
//  map and a race; this is one self-contained call whose answer gates one
//  button. Web revised its own D1 to the same conclusion.
//
//  ## No business `if`
//
//  The dedup gate, the candidate assembly, the contract verdict, the
//  chain-mismatch refusal and the clear-key-removes-provider rule are all
//  decided — and tested — in Rust. This file translates shapes and answers
//  questions.
//
//  ## Two alphabets
//
//  The core speaks snake_case (`rpc_url`, `added_at_iso`); the four storage
//  keys hold the camelCase records every other client writes (`rpcURL`,
//  `addedAt`). Capitalised initialisms are load-bearing: `rpcURL`, not `rpcUrl`.
//

import Foundation

@MainActor
final class NetworkAdminExecutor {

    /// Every operation this executor is required to handle
    /// (contracts/shell-operations.md).
    static let operations = [
        "read_store",
        "write_custom_networks",
        "write_network_configs",
        "write_service_endpoints",
        "write_rpc_providers",
        "start_search_debounce",
        "fetch_search_index",
        "fetch_chain_info",
        "probe_rpc",
        "probe_reachable",
        "rpc_get_code",
        "rpc_call_p256",
        "fetch_service_health",
        "fetch_fiat_rates",
        "invalidate_pools",
        "clear_bundler_cache",
    ]

    /// RIP-7212's P256 precompile.
    private static let p256Precompile = "0x0000000000000000000000000000000000000100"

    /// sha256("test") signed with a known P-256 key — the fixture web's
    /// `network-checker.ts` has used since the feature existed. A chain that
    /// returns the expected answer has the precompile; the CORE decides what
    /// the answer means.
    private static let p256Calldata = "0x"
        + "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        + "7bf0e18d07660f15994adce5c3836d7bd6167cdb5726f631098f433ebe0be9c0"
        + "3936edbe5c791477e714e58244afb690b9b88b833ff4acdf0fbd1b28bf0b1182"
        + "3be8cbcb3f590087711ae5ed74b9cd06a88058d0bbe700b5f0ec5a1bfac15592"
        + "f989ef9bfaae0fee03c36625e88eae99806a879d813411f876e7e03a2ffd8314"

    private let store: VelaStore
    /// `vela.serviceEndpoints`' single writer. Reaching that key any other way
    /// is how onboarding's endpoint override gets erased.
    private let accounts: AccountStore
    /// Where the chain index and per-chain data are fetched from. The core owns
    /// the value; this is the last one it reported, so a person who points the
    /// wallet at their own mirror is not ignored.
    private var ethereumDataURL = NetDefaults.ethereumDataURL
    /// The routing pool, so a changed endpoint takes effect now rather than
    /// whenever a cached winner happens to expire. `nil` keeps the
    /// acknowledged no-op the hermetic tests drive.
    private let pool: RpcPool?

    init(store: VelaStore, accounts: AccountStore, pool: RpcPool? = nil) {
        self.store = store
        self.accounts = accounts
        self.pool = pool
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        // MARK: Storage

        case "read_store":
            let endpoints = await accounts.loadServiceEndpoints()
            // Remember where chain data comes from, so the two fetches below
            // honour a person's override.
            if let override = endpoints["ethereumDataURL"] as? String, !override.isEmpty {
                ethereumDataURL = override
            }
            return CoreJSON.string([
                "type": "store_loaded",
                "custom_networks": store.readList(VelaStore.Key.customNetworks)
                    .map(Self.customNetworkToWire),
                "network_configs": store.readList(VelaStore.Key.networkConfig)
                    .map(Self.networkConfigToWire),
                "endpoints": Self.storedEndpointsToWire(endpoints),
                "provider_keys": Self.providerKeysToWire(store.readObject(VelaStore.Key.rpcProviders)),
            ])

        case "write_custom_networks":
            let networks = operation["networks"] as? [[String: Any]] ?? []
            store.writeList(
                VelaStore.Key.customNetworks,
                networks.map(Self.customNetworkToStored)
            )
            return Self.written

        case "write_network_configs":
            let configs = operation["configs"] as? [[String: Any]] ?? []
            store.writeList(VelaStore.Key.networkConfig, configs.map(Self.networkConfigToStored))
            return Self.written

        case "write_service_endpoints":
            // Through `AccountStore`: same key, same camelCase shape, merged
            // rather than replaced. The core sends the complete record, so all
            // four fields are written — but they are written INTO whatever the
            // blob already holds rather than over it.
            let endpoints = operation["endpoints"] as? [String: Any] ?? [:]
            await accounts.saveServiceEndpoints([
                "ethereumDataURL": endpoints["ethereum_data_url"] as? String,
                "passkeyIndexURL": endpoints["passkey_index_url"] as? String,
                "bundlerServiceURL": endpoints["bundler_service_url"] as? String,
                "fiatRatesURL": endpoints["fiat_rates_url"] as? String,
            ])
            if let updated = endpoints["ethereum_data_url"] as? String, !updated.isEmpty {
                ethereumDataURL = updated
            }
            return Self.written

        case "write_rpc_providers":
            let keys = operation["keys"] as? [String: Any] ?? [:]
            store.writeObject(VelaStore.Key.rpcProviders, Self.providerKeysToStored(keys))
            return Self.written

        // MARK: Search

        case "start_search_debounce":
            let ms = (operation["ms"] as? NSNumber)?.doubleValue ?? 0
            try? await Task.sleep(nanoseconds: UInt64(max(0, ms) * 1_000_000))
            // A superseded debounce is CANCELLED by the driver and never
            // answered — resolving it would push a stale search into a machine
            // that moved on.
            return CoreJSON.string(["type": "debounce_elapsed"])

        case "fetch_search_index":
            let body = await CoreHTTP.getJSON(
                "\(ethereumDataURL)/index/fuse-chains.json",
                timeout: CoreHTTP.Timeout.ethereumData
            )
            // The index is `{ data: [...] }`; anything else reads as no results,
            // which the core renders as its own "nothing found" rather than as
            // an error it has no wording for.
            let rows = (body as? [String: Any])?["data"] as? [Any] ?? []
            return CoreJSON.string([
                "type": "search_index",
                "chains": rows.compactMap(Self.searchEntryToWire),
            ])

        case "fetch_chain_info":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            let body = await CoreHTTP.getJSON(
                "\(ethereumDataURL)/chains/eip155-\(chainId).json",
                timeout: CoreHTTP.Timeout.ethereumData
            )
            return CoreJSON.string([
                "type": "chain_info",
                "chain_id": chainId,
                "data": Self.rawChainDataToWire(body) ?? NSNull(),
            ])

        // MARK: Probes

        case "probe_rpc":
            let url = operation["url"] as? String ?? ""
            return await Self.probeRpc(url)

        case "probe_reachable":
            // An explorer is a website, not a JSON API. Web has to send this
            // `no-cors` and can only report "did not throw", because a browser
            // hides the response. **iOS has no CORS**, so this asks the real
            // question and reads the real status — a strictly better answer
            // than the client this was ported from can give.
            let url = operation["url"] as? String ?? ""
            let probe = await CoreHTTP.probeJSON(url, timeout: CoreHTTP.Timeout.networkCheck)
            return CoreJSON.string([
                "type": "reachable",
                "url": url,
                "ok": probe.status != nil,
                "latency_ms": probe.latencyMs,
            ])

        case "rpc_get_code":
            let url = operation["url"] as? String ?? ""
            let address = operation["address"] as? String ?? ""
            let code = await CoreHTTP.rpc(url, method: "eth_getCode", params: [address, "latest"])
            return CoreJSON.string([
                "type": "code", "url": url, "address": address,
                "code": (code as? String) ?? NSNull(),
            ])

        case "rpc_call_p256":
            let url = operation["url"] as? String ?? ""
            // `gas: 0x100000` is the zkSync-compatibility quirk of the legacy
            // call, carried over verbatim.
            let result = await CoreHTTP.rpc(url, method: "eth_call", params: [
                ["to": Self.p256Precompile, "data": Self.p256Calldata, "gas": "0x100000"],
                "latest",
            ])
            return CoreJSON.string([
                "type": "p256_call", "url": url,
                "result": (result as? String) ?? NSNull(),
            ])

        case "fetch_service_health":
            let field = operation["field"] as? String ?? ""
            let baseURL = operation["base_url"] as? String ?? ""
            let started = Date().timeIntervalSince1970
            let probe = await CoreHTTP.probeJSON(
                "\(baseURL)/api/health?_t=\(Int(started * 1000))",
                timeout: CoreHTTP.Timeout.networkCheck
            )
            return CoreJSON.string([
                "type": "service_health",
                "field": field,
                "body": Self.healthBody(probe),
                "latency_ms": probe.latencyMs,
            ])

        case "fetch_fiat_rates":
            let url = operation["url"] as? String ?? ""
            let probe = await CoreHTTP.probeJSON(url, timeout: CoreHTTP.Timeout.fiatRates)
            return CoreJSON.string([
                "type": "fiat_rates",
                "body": Self.fiatRatesBody(probe),
                "latency_ms": probe.latencyMs,
            ])

        // The routing pool re-reads its config and drops cached winners.
        // Still acknowledged rather than skipped: the core waits for this ack
        // before leaving the write.
        case "invalidate_pools":
            pool?.invalidate(chainId: (operation["chain_id"] as? NSNumber)?.intValue)
            return CoreJSON.string(["type": "invalidated"])

        // MARK: Fail-closed until their infrastructure exists


        // The relay's two caches — the in-band quote (8 s) and the account
        // info (30 s). Before spec 052 there was no bundler client to have a
        // cache, and this answered truthfully that there was nothing to clear.
        case "clear_bundler_cache":
            return CoreJSON.string(["type": "bundler_cache_cleared"])

        default:
            // See `ContactsExecutor`: logged rather than trapped, because a
            // trap is a no-op in Release and a crash in exactly the debug build
            // running on somebody's phone.
            //
            // `store_loaded` with nothing configured is the answer that leaves
            // the core loaded — able to accept writes — rather than stalled
            // before its first render.
            print("[vela-wallet] network_admin: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string([
                "type": "store_loaded",
                "custom_networks": [], "network_configs": [],
                "endpoints": Self.storedEndpointsToWire([:]),
                "provider_keys": Self.providerKeysToWire([:]),
            ])
        }
    }

    private static let written = CoreJSON.string(["type": "written"])

    /// `parseInt(result, 16)`, guarded exactly as web guards it: a chain id of
    /// zero, a non-hex string or a non-string is **no answer at all**, never a
    /// chain id the core would then compare against.
    ///
    /// `nonisolated` because the WebSocket probe reads it from inside a task
    /// group, off the main actor. It is pure arithmetic over its argument, so
    /// there is nothing for the actor to protect.
    nonisolated static func parseChainId(_ result: Any?) -> Int? {
        guard let hex = result as? String else { return nil }
        let digits = hex.hasPrefix("0x") || hex.hasPrefix("0X") ? String(hex.dropFirst(2)) : hex
        guard let value = Int(digits, radix: 16), value > 0 else { return nil }
        return value
    }
}

/// The core's own defaults, mirrored for the one case the shell needs before it
/// has read the store. Kept next to their consumer and named after their source
/// (`network_admin.rs:157-160`) so a drift is visible.
enum NetDefaults {
    static let ethereumDataURL = "https://ethereum-data.awesometools.dev"
    /// Vela's own relay. The bundler tier has one entry because Vela runs it.
    static let bundlerServiceURL = "https://vela-relay.getvela.app"
}

// MARK: - Probes

private extension NetworkAdminExecutor {

    /// `eth_chainId`, over HTTPS or over a WebSocket.
    ///
    /// The WebSocket half exists because a custom RPC is often `wss://`, and
    /// answering `null` for those would make every WebSocket endpoint read as
    /// incompatible — a chain a person genuinely could add, refused for the
    /// shell's convenience.
    static func probeRpc(_ url: String) async -> String {
        let started = Date()
        let reported: Int?
        if url.hasPrefix("wss://") || url.hasPrefix("ws://") {
            reported = await probeWebSocket(url)
        } else {
            reported = parseChainId(await CoreHTTP.rpc(url, method: "eth_chainId", params: []))
        }
        return CoreJSON.string([
            "type": "probed",
            "url": url,
            "reported_chain_id": reported ?? NSNull(),
            "latency_ms": Date().timeIntervalSince(started) * 1000,
        ])
    }

    static func probeWebSocket(_ url: String) async -> Int? {
        guard let parsed = URL(string: url), parsed.scheme?.lowercased() == "wss" else { return nil }
        let task = URLSession.shared.webSocketTask(with: parsed)
        task.resume()
        defer { task.cancel(with: .goingAway, reason: nil) }

        let request = #"{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}"#
        do {
            try await task.send(.string(request))
            // The budget is the probe's, not the socket's: a server that opens
            // the connection and then says nothing must not hold the wizard.
            return try await withThrowingTaskGroup(of: Int?.self) { group in
                group.addTask {
                    let message = try await task.receive()
                    guard case .string(let text) = message,
                          let data = text.data(using: .utf8),
                          let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
                    else { return nil }
                    return parseChainId(object["result"])
                }
                group.addTask {
                    try await Task.sleep(nanoseconds: UInt64(CoreHTTP.Timeout.networkCheck * 1_000_000_000))
                    return nil
                }
                let first = try await group.next() ?? nil
                group.cancelAll()
                return first
            }
        } catch {
            return nil
        }
    }

    /// The `/api/health` body, shaped for the core. **The verdict is the
    /// core's** — this only says what came back.
    static func healthBody(_ probe: CoreHTTP.Probe) -> [String: Any] {
        guard let status = probe.status else { return ["type": "failed"] }
        guard (200..<300).contains(status) else {
            return ["type": "http_error", "status": status]
        }
        let object = probe.body as? [String: Any] ?? [:]
        return [
            "type": "identity",
            "service": (object["service"] as? String) ?? NSNull(),
            "status": (object["status"] as? String) ?? NSNull(),
        ]
    }
}

// MARK: - Stored ⇄ wire

// Internal rather than private: these codecs ARE the cross-client contract,
// and a contract nothing can test is a contract nobody keeps.
extension NetworkAdminExecutor {

    /// The fiat endpoint's reply, shaped for the core.
    ///
    /// The COUNT is the shell's, because "how many rates are in this body" is a
    /// question about the two response shapes a swappable provider may send.
    /// Whether a count of zero means the endpoint is unusable is the core's,
    /// and it is not answered here.
    static func fiatRatesBody(_ probe: CoreHTTP.Probe) -> [String: Any] {
        guard let status = probe.status else { return ["type": "failed"] }
        guard (200..<300).contains(status) else {
            return ["type": "http_error", "status": status]
        }
        let count: Int
        if let rows = probe.body as? [Any] {
            count = rows.count
        } else if let rates = (probe.body as? [String: Any])?["rates"] as? [String: Any] {
            count = rates.count
        } else {
            count = 0
        }
        return ["type": "rates", "rate_count": count]
    }

    static func customNetworkToWire(_ stored: [String: Any]) -> [String: Any] {
        [
            "id": string(stored["id"]),
            "display_name": string(stored["displayName"]),
            "chain_id": integer(stored["chainId"]),
            "icon_label": string(stored["iconLabel"]),
            "icon_color": string(stored["iconColor"]),
            "icon_bg": string(stored["iconBg"]),
            "logo_url": string(stored["logoURL"]),
            "is_l2": stored["isL2"] as? Bool ?? false,
            "rpc_url": string(stored["rpcURL"]),
            "explorer_url": string(stored["explorerURL"]),
            "bundler_url": string(stored["bundlerURL"]),
            "native_symbol": string(stored["nativeSymbol"]),
            "added_at_iso": string(stored["addedAt"]),
        ]
    }

    static func customNetworkToStored(_ wire: [String: Any]) -> [String: Any] {
        [
            "id": string(wire["id"]),
            "displayName": string(wire["display_name"]),
            "chainId": integer(wire["chain_id"]),
            "iconLabel": string(wire["icon_label"]),
            "iconColor": string(wire["icon_color"]),
            "iconBg": string(wire["icon_bg"]),
            "logoURL": string(wire["logo_url"]),
            "isL2": wire["is_l2"] as? Bool ?? false,
            "rpcURL": string(wire["rpc_url"]),
            "explorerURL": string(wire["explorer_url"]),
            "bundlerURL": string(wire["bundler_url"]),
            "nativeSymbol": string(wire["native_symbol"]),
            "addedAt": string(wire["added_at_iso"]),
        ]
    }

    static func networkConfigToWire(_ stored: [String: Any]) -> [String: Any] {
        [
            "chain_id": integer(stored["chainId"]),
            "rpc_url": string(stored["rpcURL"]),
            "explorer_url": string(stored["explorerURL"]),
            "bundler_url": string(stored["bundlerURL"]),
        ]
    }

    static func networkConfigToStored(_ wire: [String: Any]) -> [String: Any] {
        [
            "chainId": integer(wire["chain_id"]),
            "rpcURL": string(wire["rpc_url"]),
            "explorerURL": string(wire["explorer_url"]),
            "bundlerURL": string(wire["bundler_url"]),
        ]
    }

    /// The RAW blob: absent fields stay **absent**, so the core applies its own
    /// defaults merge rather than being handed empty strings it would take for
    /// a person's choice.
    static func storedEndpointsToWire(_ stored: [String: Any]) -> [String: Any] {
        [
            "ethereum_data_url": optionalString(stored["ethereumDataURL"]),
            "passkey_index_url": optionalString(stored["passkeyIndexURL"]),
            "bundler_service_url": optionalString(stored["bundlerServiceURL"]),
            "fiat_rates_url": optionalString(stored["fiatRatesURL"]),
        ]
    }

    static func providerKeysToWire(_ stored: [String: Any]) -> [String: Any] {
        [
            "alchemy": optionalString(stored["alchemy"]),
            "drpc": optionalString(stored["drpc"]),
            "ankr": optionalString(stored["ankr"]),
        ]
    }

    /// A cleared key is **removed**, not stored as an empty string — the core's
    /// invariant ⑦ and the shape the Expo saver writes.
    static func providerKeysToStored(_ wire: [String: Any]) -> [String: Any] {
        var out: [String: Any] = [:]
        for provider in ["alchemy", "drpc", "ankr"] {
            if let key = wire[provider] as? String, !key.isEmpty { out[provider] = key }
        }
        return out
    }

    /// One row of `/index/fuse-chains.json`. A row without a usable chain id is
    /// dropped rather than coerced to zero — chain 0 is not a chain.
    ///
    /// **The `u32` guard is not paranoia — it is a live defect this cut found.**
    /// The production index contains chain ids above `u32::MAX` (7078815900 was
    /// the first one), the core's `chain_id` is a `u32`, and serde refuses the
    /// WHOLE `search_index` result over one bad row. The observable symptom is
    /// a wizard stuck on 搜索中 forever with no error anywhere, because the
    /// result never reaches the machine.
    ///
    /// Dropping the row is hygiene, not policy: a chain id the core cannot
    /// represent is a chain it can never be asked about, so offering it would
    /// be offering a dead end.
    static func searchEntryToWire(_ raw: Any) -> [String: Any]? {
        guard let row = raw as? [String: Any],
              let number = row["chainId"] as? NSNumber,
              let chainId = uint32(number)
        else { return nil }
        return [
            "chain_id": chainId,
            "name": string(row["name"]),
            "short_name": string(row["shortName"]),
            "native_currency_symbol": string(row["nativeCurrencySymbol"]),
            "has_logo": row["hasLogo"] as? Bool ?? false,
        ]
    }

    /// `/chains/eip155-{id}.json` as the core wants it: **raw and unfiltered**.
    ///
    /// Only the explorer entries are flattened to their `url`, because the
    /// core's field is a string list. Every parsing DECISION — the defaults, the
    /// HTTPS filter, the key-placeholder rejection — belongs to
    /// `parse_chain_data` in Rust, and doing any of it here would be a second
    /// opinion about which chains exist.
    static func rawChainDataToWire(_ body: Any?) -> [String: Any]? {
        guard let data = body as? [String: Any] else { return nil }
        let native = data["nativeCurrency"] as? [String: Any]
        return [
            // Same `u32` guard as the index: an id the core cannot hold arrives
            // as "no id", which its parser already knows how to refuse.
            "chain_id": (data["chainId"] as? NSNumber).flatMap(uint32) ?? NSNull(),
            "name": optionalString(data["name"]),
            "short_name": optionalString(data["shortName"]),
            "native_currency_name": optionalString(native?["name"]),
            "native_currency_symbol": optionalString(native?["symbol"]),
            "native_currency_decimals": (native?["decimals"] as? NSNumber)?.intValue ?? NSNull(),
            "rpc": (data["rpc"] as? [Any] ?? []).compactMap { $0 as? String },
            "explorers": (data["explorers"] as? [Any] ?? []).map {
                ($0 as? [String: Any])?["url"] as? String ?? ""
            },
            "testnet": data["testnet"] as? Bool ?? false,
        ]
    }

    // MARK: Coercion — deserialization hygiene, never policy

    static func string(_ value: Any?) -> String { value as? String ?? "" }

    static func optionalString(_ value: Any?) -> Any {
        guard let text = value as? String, !text.isEmpty else { return NSNull() }
        return text
    }

    /// A missing or non-finite chain id becomes 0, which serde accepts and the
    /// core discards — the alternative is a rejected `store_loaded`, which
    /// strands the machine unloaded forever.
    static func integer(_ value: Any?) -> Int {
        guard let number = value as? NSNumber, number.doubleValue.isFinite else { return 0 }
        return number.intValue
    }

    /// A chain id the core's `u32` can actually hold, or nothing.
    ///
    /// `nil` for a negative, a fractional, or an out-of-range value. Every wire
    /// field this feeds is either optional or a row that can be dropped, which
    /// is deliberate: one unrepresentable id must never cost the whole answer.
    static func uint32(_ number: NSNumber) -> Int? {
        let value = number.doubleValue
        guard value.isFinite, value >= 0, value <= Double(UInt32.max),
              value == value.rounded(.towardZero)
        else { return nil }
        return Int(value)
    }
}
