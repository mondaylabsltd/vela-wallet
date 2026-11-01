//
//  LocalMultisigWorld.swift
//  VelaWalletUITests
//
//  The multisig fork harness, without the fork: one loopback HTTP server
//  inside the test runner that plays the genesis service (its health, the
//  index's records, `/api/import`, the proposals mailbox) AND Gnosis's
//  JSON-RPC (Safe's own v1.4.1 code at its own addresses, balances, blocks,
//  a chain where nothing else is deployed). The app is pointed at it with
//  its Debug launch inputs (`VELA_RPC_OVERRIDES`, `VELA_SERVICE_ENDPOINTS`),
//  so a multisig can be created — and a send proposed from it — on a CI
//  simulator with no harness at all. The parallel space signs every passkey.
//
//  What it is NOT: a chain. Nothing executes; balances are a fixed figure.
//  What the app sends is recorded (`requests`, `imports`, `mailbox`) so a
//  test reads what reached the service, not only what the screen says.
//
//  The mailbox merges approvals by safeTxHash, as the service does — the
//  hash computed here (EIP-712 over `SafeTx`, Keccak-256 below), because the
//  app drops a proposal that does not hash as itself.
//
//  Safe's code comes from the core's vectors through `#filePath`: a path on
//  the Mac, which a SIMULATOR's runner can read (a phone's cannot — this
//  world is for simulators).
//

import Foundation
import Network

final class LocalMultisigWorld: @unchecked Sendable {

    static let factory = "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67"
    static let safe141 = "0x41675C099F32341bf84BFc5382aF534df5C7461a"
    static let safeL2141 = "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762"
    static let handler141 = "0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99"
    static let toL2Setup141 = "0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54"
    static let webauthnSigner = "0x94a4F6affBd8975951142c3999aEAB7ecee555c2"

    private let listener: NWListener
    private let queue = DispatchQueue(label: "vela.multisig-world")
    private let lock = NSLock()
    private let codes: [String: String]
    private(set) var port: UInt16 = 0

    /// The genesis service's base URL, and the Gnosis node's.
    var serviceURL: String { "http://127.0.0.1:\(port)" }
    var rpcURL: String { "http://127.0.0.1:\(port)/rpc" }

    // What reached it (read under the lock).
    private var _requests: [String] = []
    private var _imports: [[String: Any]] = []
    private var _records: [String: [String: Any]] = [:]
    private var _mailbox: [[String: Any]] = []

    var requests: [String] { lock.withLock { _requests } }
    var imports: [[String: Any]] { lock.withLock { _imports } }
    var mailbox: [[String: Any]] { lock.withLock { _mailbox } }
    func rpcCalls(_ method: String) -> Int { requests.filter { $0 == "rpc \(method)" }.count }

    init() throws {
        codes = try Self.canonicalCodes()
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = false
        listener = try NWListener(using: parameters, on: .any)
    }

    /// Safe's own v1.4.1 code, from the core's vectors.
    static func canonicalCodes() throws -> [String: String] {
        let dir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("rust/crates/vela-core/tests/vectors/safe-code")
        let named = [
            (factory, "safe-proxy-factory-1.4.1"), (safe141, "safe-1.4.1"), (safeL2141, "safe-l2-1.4.1"),
            (handler141, "compatibility-fallback-handler-1.4.1"), (toL2Setup141, "safe-to-l2-setup-1.4.1"),
        ]
        var out: [String: String] = [:]
        for (address, name) in named {
            let text = try String(contentsOf: dir.appendingPathComponent("\(name).hex"), encoding: .utf8)
            out[address.lowercased()] = text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return out
    }

    /// Listens, and waits (up to 5 s) for the port it was given.
    @discardableResult
    func start() -> Bool {
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { state in
            if case .ready = state { ready.signal() }
            if case .failed = state { ready.signal() }
        }
        listener.newConnectionHandler = { [weak self] connection in
            connection.start(queue: .global())
            self?.read(connection, Data())
        }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
        return port != 0
    }

    func stop() { listener.cancel() }

    // MARK: - HTTP

    /// Read until the headers and `Content-Length` bytes of body are in.
    private func read(_ connection: NWConnection, _ buffer: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 256 * 1024) { [weak self] data, _, complete, error in
            guard let self else { return }
            var buffer = buffer
            if let data { buffer.append(data) }
            if let request = Self.parse(buffer) {
                let (status, body) = self.answer(method: request.method, path: request.path, body: request.body)
                let header = "HTTP/1.1 \(status) \(status < 300 ? "OK" : "Error")\r\n"
                    + "Content-Type: application/json\r\n"
                    + "Content-Length: \(body.count)\r\n"
                    + "Cache-Control: no-store\r\n"
                    + "Connection: close\r\n\r\n"
                var response = Data(header.utf8)
                response.append(body)
                connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
            } else if complete || error != nil {
                connection.cancel()
            } else {
                self.read(connection, buffer)
            }
        }
    }

    static func parse(_ buffer: Data) -> (method: String, path: String, body: Data)? {
        guard let end = buffer.range(of: Data("\r\n\r\n".utf8)) else { return nil }
        let head = String(decoding: buffer[buffer.startIndex..<end.lowerBound], as: UTF8.self)
        let lines = head.components(separatedBy: "\r\n")
        let first = lines.first?.split(separator: " ").map(String.init) ?? []
        guard first.count >= 2 else { return nil }
        let length = lines.dropFirst().compactMap { line -> Int? in
            let parts = line.split(separator: ":", maxSplits: 1)
            guard parts.count == 2, parts[0].lowercased() == "content-length" else { return nil }
            return Int(parts[1].trimmingCharacters(in: .whitespaces))
        }.first ?? 0
        let body = buffer[end.upperBound...]
        guard body.count >= length else { return nil }
        return (first[0], first[1], Data(body.prefix(length)))
    }

    private func answer(method: String, path: String, body: Data) -> (Int, Data) {
        lock.lock()
        defer { lock.unlock() }
        if path.hasPrefix("/api/") {
            _requests.append("\(method) \(path)")
            let (status, object) = service(method: method, path: path, body: body)
            return (status, Self.json(object))
        }
        // JSON-RPC: one call or a batch.
        let parsed = try? JSONSerialization.jsonObject(with: body)
        if let batch = parsed as? [[String: Any]] {
            return (200, Self.json(batch.map(rpc)))
        }
        return (200, Self.json(rpc(parsed as? [String: Any] ?? [:])))
    }

    // MARK: - The genesis service

    private func service(method: String, path: String, body: Data) -> (Int, Any) {
        let (route, query) = path.split(separator: "?", maxSplits: 1).map(String.init).splitFirst()
        if route == "/api/health" {
            return (200, [
                "service": "safe-genesis-registry", "ok": true, "homeChainId": 100,
                "chains": [["chainId": 100, "registry": true]],
            ])
        }
        if route.hasPrefix("/api/genesis/") {
            let safe = String(route.dropFirst("/api/genesis/".count)).lowercased()
            if let record = _records[safe] { return (200, record) }
            return (404, ["found": false])
        }
        let request = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any] ?? [:]
        if route == "/api/import", method == "POST" {
            _imports.append(request)
            if let safe = request["safe"] as? String, let genesis = request["genesis"] {
                _records[safe.lowercased()] = [
                    "found": true, "pending": false, "genesis": genesis, "recordedOn": [100], "readFrom": 100,
                ]
            }
            return (200, ["status": "recorded", "recordedOn": [100]])
        }
        if route == "/api/proposals", method == "POST" {
            return (201, take(request))
        }
        if route.hasPrefix("/api/proposals/") {
            let safe = String(route.dropFirst("/api/proposals/".count))
            let chain = (query ?? "").split(separator: "&")
                .compactMap { $0.hasPrefix("chainId=") ? Int($0.dropFirst(8)) : nil }.first
            let mine = _mailbox.filter {
                ($0["safe"] as? String)?.caseInsensitiveCompare(safe) == .orderedSame
                    && (chain == nil || ($0["chainId"] as? Int) == chain)
            }
            if chain != nil {
                return (200, [
                    "members": ["deployed": false, "owners": [], "threshold": "1", "nonce": "0"],
                    "proposals": mine,
                ])
            }
            var counts: [Int: Int] = [:]
            for p in mine { counts[p["chainId"] as? Int ?? 0, default: 0] += 1 }
            return (200, [
                "total": mine.count,
                "chains": counts.keys.sorted().map { ["chainId": $0, "count": counts[$0]!] },
                "proposals": mine,
            ])
        }
        return (404, ["error": "no route"])
    }

    /// The mailbox takes an approval: merged into its proposal by safeTxHash,
    /// or a new proposal.
    private func take(_ request: [String: Any]) -> [String: Any] {
        let chain = (request["chainId"] as? NSNumber)?.intValue ?? 0
        let safe = request["safe"] as? String ?? ""
        let tx = request["safeTx"] as? [String: Any] ?? [:]
        let hash = SafeTxHash.of(chainId: chain, safe: safe, tx: tx)
        let now = Int(Date().timeIntervalSince1970 * 1000)
        var approval = request["approval"] as? [String: Any] ?? [:]
        approval["createdMs"] = now
        let owner = (approval["owner"] as? String ?? "").lowercased()
        let same: ([String: Any]) -> Bool = {
            ($0["chainId"] as? Int) == chain && ($0["safeTxHash"] as? String) == hash
        }
        let status: String
        if let i = _mailbox.firstIndex(where: same) {
            var approvals = _mailbox[i]["approvals"] as? [[String: Any]] ?? []
            if approvals.contains(where: { ($0["owner"] as? String)?.lowercased() == owner }) {
                status = "unchanged"
            } else {
                approvals.append(approval)
                status = "approved"
            }
            _mailbox[i]["approvals"] = approvals
            _mailbox[i]["updatedMs"] = now
        } else {
            _mailbox.append([
                "chainId": chain, "safe": safe, "safeTxHash": hash, "safeTx": tx,
                "proposer": approval["owner"] ?? "", "approvals": [approval],
                "createdMs": now, "updatedMs": now,
            ])
            status = "created"
        }
        return ["status": status, "proposal": _mailbox.first(where: same) ?? [:]]
    }

    // MARK: - Gnosis

    private func rpc(_ call: [String: Any]) -> [String: Any] {
        let id = call["id"] ?? 1
        let method = call["method"] as? String ?? ""
        let params = call["params"] as? [Any] ?? []
        _requests.append("rpc \(method)")
        func ok(_ result: Any) -> [String: Any] { ["jsonrpc": "2.0", "id": id, "result": result] }
        let zeroWord = "0x" + String(repeating: "0", count: 64)
        switch method {
        case "eth_chainId": return ok("0x64")
        case "net_version": return ok("100")
        case "eth_blockNumber": return ok("0x2625a00")
        case "eth_getBlockByNumber", "eth_getBlockByHash":
            return ok([
                "number": "0x2625a00", "hash": zeroWord, "parentHash": zeroWord,
                "timestamp": "0x" + String(Int(Date().timeIntervalSince1970), radix: 16),
                "baseFeePerGas": "0x3b9aca00", "gasLimit": "0x1c9c380", "gasUsed": "0x0",
                "transactions": [],
            ])
        case "eth_gasPrice", "eth_maxPriorityFeePerGas": return ok("0x3b9aca00")
        case "eth_feeHistory":
            return ok([
                "oldestBlock": "0x2625a00", "baseFeePerGas": ["0x3b9aca00", "0x3b9aca00"],
                "gasUsedRatio": [0.5], "reward": [["0x3b9aca00", "0x3b9aca00", "0x3b9aca00"]],
            ])
        case "eth_getBalance": return ok("0x56bc75e2d63100000")  // 100 xDAI, whoever asks
        case "eth_getTransactionCount": return ok("0x0")
        case "eth_getCode":
            return ok(codes[(params.first as? String ?? "").lowercased()] ?? "0x")
        case "eth_getStorageAt": return ok(zeroWord)
        case "eth_estimateGas": return ok("0x30d40")
        case "eth_getLogs": return ok([Any]())
        case "eth_getTransactionReceipt", "eth_getTransactionByHash": return ok(NSNull())
        case "eth_call":
            let request = params.first as? [String: Any] ?? [:]
            let to = (request["to"] as? String ?? "").lowercased()
            let data = (request["data"] as? String ?? (request["input"] as? String ?? "0x")).lowercased()
            // `getConfiguration(address)` on Vela's shared passkey signer: nobody configured.
            if to == Self.webauthnSigner.lowercased(), data.hasPrefix("0xc44b11f7") {
                return ok("0x" + String(repeating: "0", count: 192))
            }
            return ok("0x")
        default:
            return ["jsonrpc": "2.0", "id": id, "error": ["code": -32601, "message": "\(method) is not served here"]]
        }
    }

    static func json(_ object: Any) -> Data {
        (try? JSONSerialization.data(withJSONObject: object, options: [.fragmentsAllowed])) ?? Data("null".utf8)
    }
}

private extension Array where Element == String {
    func splitFirst() -> (String, String?) { (first ?? "", count > 1 ? self[1] : nil) }
}

// MARK: - SafeTx hash (EIP-712), as the service and the core compute it

enum SafeTxHash {
    static let domainTypehash = "47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a79469218"
    static let safeTxTypehash = "bb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8"

    static func of(chainId: Int, safe: String, tx: [String: Any]) -> String {
        let domain = Keccak.hash(bytes(domainTypehash) + word(chainId) + addressWord(safe))
        let data = bytes(tx["data"] as? String ?? "0x")
        let body = Keccak.hash(
            bytes(safeTxTypehash) + addressWord(tx["to"]) + word(tx["value"]) + Keccak.hash(data)
                + word(tx["operation"]) + word(tx["safeTxGas"]) + word(tx["baseGas"]) + word(tx["gasPrice"])
                + addressWord(tx["gasToken"]) + addressWord(tx["refundReceiver"]) + word(tx["nonce"])
        )
        return "0x" + hex(Keccak.hash([0x19, 0x01] + domain + body))
    }

    static func hex(_ bytes: [UInt8]) -> String { bytes.map { String(format: "%02x", $0) }.joined() }

    static func bytes(_ hex: String) -> [UInt8] {
        var body = Array(hex.hasPrefix("0x") ? hex.dropFirst(2) : Substring(hex))
        if body.count % 2 == 1 { body.insert("0", at: 0) }
        return stride(from: 0, to: body.count, by: 2).compactMap { UInt8(String(body[$0...$0 + 1]), radix: 16) }
    }

    static func addressWord(_ value: Any?) -> [UInt8] {
        let raw = bytes(value as? String ?? "0x")
        return [UInt8](repeating: 0, count: max(0, 32 - raw.count)) + raw.suffix(32)
    }

    /// A uint256: a number, a `0x` quantity, or decimal text.
    static func word(_ value: Any?) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: 32)
        switch value {
        case let n as NSNumber:
            var v = n.uint64Value
            for i in stride(from: 31, through: 24, by: -1) { out[i] = UInt8(v & 0xff); v >>= 8 }
        case let s as String where s.hasPrefix("0x"):
            let raw = bytes(s)
            out = [UInt8](repeating: 0, count: max(0, 32 - raw.count)) + raw.suffix(32)
        case let s as String:
            for digit in s.compactMap({ $0.wholeNumberValue }) {
                var carry = digit
                for i in stride(from: 31, through: 0, by: -1) {
                    let v = Int(out[i]) * 10 + carry
                    out[i] = UInt8(v & 0xff)
                    carry = v >> 8
                }
            }
        default:
            break
        }
        return out
    }

    static func word(_ n: Int) -> [UInt8] { word(NSNumber(value: n)) }
}

/// Keccak-256 (the pre-FIPS padding Ethereum uses), from FIPS 202's permutation.
enum Keccak {
    private static let rc: [UInt64] = [
        0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
        0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
        0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
        0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
        0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
        0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
    ]
    private static let rotations = [1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44]
    private static let lanes = [10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1]

    private static func rotl(_ x: UInt64, _ n: Int) -> UInt64 { (x << UInt64(n)) | (x >> UInt64(64 - n)) }

    private static func permute(_ s: inout [UInt64]) {
        var c = [UInt64](repeating: 0, count: 5)
        for round in 0..<24 {
            for i in 0..<5 { c[i] = s[i] ^ s[i + 5] ^ s[i + 10] ^ s[i + 15] ^ s[i + 20] }
            for i in 0..<5 {
                let t = c[(i + 4) % 5] ^ rotl(c[(i + 1) % 5], 1)
                for j in stride(from: 0, to: 25, by: 5) { s[j + i] ^= t }
            }
            var t = s[1]
            for i in 0..<24 {
                let j = lanes[i]
                let next = s[j]
                s[j] = rotl(t, rotations[i])
                t = next
            }
            for j in stride(from: 0, to: 25, by: 5) {
                for i in 0..<5 { c[i] = s[j + i] }
                for i in 0..<5 { s[j + i] ^= ~c[(i + 1) % 5] & c[(i + 2) % 5] }
            }
            s[0] ^= rc[round]
        }
    }

    static func hash(_ data: [UInt8]) -> [UInt8] {
        let rate = 136
        var message = data
        message.append(0x01)
        while message.count % rate != 0 { message.append(0) }
        message[message.count - 1] |= 0x80
        var state = [UInt64](repeating: 0, count: 25)
        for block in stride(from: 0, to: message.count, by: rate) {
            for i in 0..<(rate / 8) {
                var lane: UInt64 = 0
                for b in 0..<8 { lane |= UInt64(message[block + i * 8 + b]) << UInt64(8 * b) }
                state[i] ^= lane
            }
            permute(&state)
        }
        var out: [UInt8] = []
        for i in 0..<4 { for b in 0..<8 { out.append(UInt8((state[i] >> UInt64(8 * b)) & 0xff)) } }
        return out
    }
}
