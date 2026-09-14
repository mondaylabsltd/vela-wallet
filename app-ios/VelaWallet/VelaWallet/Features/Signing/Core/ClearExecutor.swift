//
//  ClearExecutor.swift
//  VelaWallet
//
//  The `clear_signing` machine's five arms: a descriptor, a selector lookup,
//  a routed `eth_call`, a clock and a timer.
//
//  Ported from `app-android/.../feature/signing/core/ClearExecutor.kt` (spec
//  044 T030), which is the desktop's `executor/clear_signing.rs`.
//
//  ## The one distinction the core cannot re-derive
//
//  `rpc_error: true` means the node **answered with an error object** — the
//  contract reverted. `result: null` with `rpc_error: false` means nobody
//  answered at all. The core reads those as different facts and picks a
//  different rung of the degradation ladder for each, so an executor that
//  flattens them makes a reverting contract look like a chain that is down.
//
//  Nothing else here interprets anything. Whether a returned word is a
//  plausible decimals value, whether a descriptor is trustworthy, how long to
//  wait: all the core's.
//

import Foundation

@MainActor
final class ClearExecutor {

    static let operations = ["http_get", "rpc_eth_call", "selector_db_lookup", "timer", "now"]

    /// The chain-data base the settings machine names. A blank base answers
    /// "no descriptor", which is a rung, not a failure.
    private let dataBase: () -> String
    private let pool: RpcPool

    /// Selector candidates, cached for the session. The lookup is two public
    /// databases and a signing sheet asks for the same selector every time a
    /// person swaps on the same router.
    private var selectorCache: [String: [String]] = [:]

    init(dataBase: @escaping () -> String, pool: RpcPool) {
        self.dataBase = dataBase
        self.pool = pool
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "http_get":
            let path = operation["path"] as? String ?? ""
            return CoreJSON.string([
                "type": "descriptor_fetched",
                "path": path,
                "json": await descriptor(path: path) as Any? ?? NSNull(),
            ])

        case "rpc_eth_call":
            let probe = operation["probe"] as? String ?? ""
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            let to = operation["to"] as? String ?? ""
            let outcome = await pool.call(
                chainId: chainId,
                method: "eth_call",
                params: [["to": to, "data": operation["data"] as? String ?? "0x"], "latest"]
            )
            var result: Any = NSNull()
            var reverted = false
            switch outcome {
            case .ok(let body):
                if let hex = body as? String { result = hex }
            case .rpcError:
                // The node answered, and its answer was "this reverted".
                reverted = true
            default:
                break
            }
            return CoreJSON.string([
                "type": "rpc_answer",
                "probe": probe,
                "chain_id": chainId,
                "to": to,
                "result": result,
                "rpc_error": reverted,
            ])

        case "selector_db_lookup":
            let selector = operation["selector"] as? String ?? ""
            return CoreJSON.string([
                "type": "selector_candidates",
                "sigs": await lookup(selector: selector),
            ])

        case "timer":
            let ms = (operation["ms"] as? NSNumber)?.intValue ?? 0
            let token = (operation["token"] as? NSNumber)?.intValue ?? 0
            try? await Task.sleep(nanoseconds: UInt64(max(0, ms)) * 1_000_000)
            return CoreJSON.string(["type": "timed_out", "token": token])

        case "now":
            return CoreJSON.string(["type": "clock", "now_ms": Date().timeIntervalSince1970 * 1000])

        default:
            print("[vela-wallet] clear_signing: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "http_get":
            return CoreJSON.string([
                "type": "descriptor_fetched",
                "path": operation["path"] as? String ?? "",
                "json": NSNull(),
            ])
        case "rpc_eth_call":
            return CoreJSON.string([
                "type": "rpc_answer",
                "probe": operation["probe"] as? String ?? "",
                "chain_id": (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
                "to": operation["to"] as? String ?? "",
                "result": NSNull(),
                "rpc_error": false,
            ])
        case "selector_db_lookup":
            return CoreJSON.string(["type": "selector_candidates", "sigs": []])
        case "timer":
            return CoreJSON.string(["type": "timed_out", "token": (operation["token"] as? NSNumber)?.intValue ?? 0])
        default:
            return CoreJSON.string(["type": "clock", "now_ms": Date().timeIntervalSince1970 * 1000])
        }
    }

    // MARK: - The two networks this machine reaches

    private func descriptor(path: String) async -> String? {
        let base = dataBase().trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !base.isEmpty, !path.isEmpty else { return nil }
        let url = base + "/" + path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        return await Self.text(url: url, timeout: Self.descriptorTimeout)
    }

    /// Eight hex digits, or nothing worth asking about.
    static func normalise(selector raw: String) -> String? {
        let hex = (raw.hasPrefix("0x") ? String(raw.dropFirst(2)) : raw).lowercased()
        guard hex.count == 8, hex.allSatisfy({ $0.isHexDigit }) else { return nil }
        return hex
    }

    private func lookup(selector raw: String) async -> [String] {
        guard let selector = Self.normalise(selector: raw) else { return [] }
        if let cached = selectorCache[selector] { return cached }

        var found = await Self.fromOpenchain(selector: selector)
        if found.isEmpty { found = await Self.fromFourByte(selector: selector) }
        if !found.isEmpty { selectorCache[selector] = found }
        return found
    }

    static let descriptorTimeout: TimeInterval = 6
    static let selectorTimeout: TimeInterval = 5

    private static func text(url: String, timeout: TimeInterval) async -> String? {
        guard let parsed = URL(string: url), parsed.scheme?.lowercased() == "https" else { return nil }
        var request = URLRequest(url: parsed)
        request.timeoutInterval = timeout
        request.setValue("application/json", forHTTPHeaderField: "accept")
        guard let (data, response) = try? await URLSession.shared.data(for: request),
              let status = (response as? HTTPURLResponse)?.statusCode, status == 200
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private static func json(url: String) async -> [String: Any]? {
        guard let body = await text(url: url, timeout: selectorTimeout),
              let data = body.data(using: .utf8)
        else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    static func fromOpenchain(selector: String) async -> [String] {
        let url = "https://api.openchain.xyz/signature-database/v1/lookup"
            + "?function=0x\(selector)&filter=true"
        guard let object = await json(url: url),
              let result = object["result"] as? [String: Any],
              let function = result["function"] as? [String: Any],
              let list = function["0x\(selector)"] as? [[String: Any]]
        else { return [] }
        return list.compactMap { ($0["name"] as? String)?.nilIfBlank }
    }

    static func fromFourByte(selector: String) async -> [String] {
        let url = "https://www.4byte.directory/api/v1/signatures/?hex_signature=0x\(selector)"
        guard let object = await json(url: url),
              let results = object["results"] as? [[String: Any]]
        else { return [] }
        // Shortest first: the 4byte database is full of collisions whose
        // longer entries are deliberate lookalikes.
        return results
            .compactMap { ($0["text_signature"] as? String)?.nilIfBlank }
            .sorted { $0.count < $1.count }
    }
}

private extension String {
    var nilIfBlank: String? {
        trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : self
    }
}
