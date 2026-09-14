//
//  RequestRouter.swift
//  VelaWallet
//
//  What happens to a request the permissions machine forwarded but does not
//  answer itself: the reads, the state questions, the chain switch, the
//  acknowledgements, and the refusals.
//
//  Ported from `app-android/.../feature/browser/core/RequestRouter.kt`
//  (spec 044), which is the desktop's `executor/dapp_rpc.rs`.
//
//  ## `net_version` is decimal and `eth_chainId` is hex
//
//  Not a nicety. A hex answer to `net_version` is a string comparison every
//  dApp fails, and the failure is silent: the site simply believes it is on a
//  different network than the wallet.
//
//  ## A user-op hash is not a transaction hash, and a page must not learn that
//
//  `eth_sendTransaction` is answered with a **transaction** hash. A page that
//  then polls `eth_getTransactionReceipt` for a hash this wallet minted as a
//  user operation gets the receipt for the transaction that carried it — the
//  translation spec 028 found was needed when a real dApp's swap "submitted
//  but never confirmed".
//

import Foundation

@MainActor
final class RequestRouter {

    enum Receipt {
        case pending
        case landed(txHash: String)
    }

    struct Ports {
        var browserChain: () -> Int = { 100 }
        var knownChains: () -> [Int] = { [] }
        var switchChain: (Int) -> Void = { _ in }
        /// One call through the wallet's own pool. `nil` = no endpoint
        /// answered at all, which is different from a node that refused.
        var poolCall: (_ chainId: Int, _ method: String, _ params: [Any], _ bundler: Bool) async -> [String: Any]?
        = { _, _, _, _ in nil }
        var respond: (_ id: String, _ json: [String: Any]) -> Void = { _, _ in }
        var sign: (_ id: String, _ method: String, _ paramsJson: String, _ origin: String) -> Void
        = { _, _, _, _ in }
        /// `nil` when this hash is not one of ours.
        var receiptFor: (_ userOpHash: String) async -> Receipt? = { _ in nil }
    }

    var ports: Ports

    init(ports: Ports = Ports()) {
        self.ports = ports
    }

    func route(id: String, method: String, paramsJson: String, origin: String) async {
        let route = DappRpc.route(method)

        switch route {
        case .sign:
            ports.sign(id, method, paramsJson, origin)

        case .state:
            let chainId = ports.browserChain()
            let answer: Any = method == "net_version"
                ? String(chainId)
                : DappRpc.hexChainId(chainId)
            ports.respond(id, BrowserExecutor.resultJson(id: id, result: answer))

        case .switchChain:
            guard let wanted = DappRpc.switchChainParam(paramsJson) else {
                ports.respond(id, BrowserExecutor.errorJson(id: id, code: -32602, message: "Invalid chainId"))
                return
            }
            guard ports.knownChains().contains(wanted) else {
                // 4902 is the standard's "add it first", and adding a network
                // by a page's request is 055's path, not this one's.
                ports.respond(id, BrowserExecutor.errorJson(id: id, code: 4902, message: "Unrecognized chain ID"))
                return
            }
            ports.switchChain(wanted)
            ports.respond(id, BrowserExecutor.resultJson(id: id, result: nil))

        case .ack:
            // Acknowledged, and nothing changed. Saying "no" to
            // `wallet_addEthereumChain` breaks dApps that call it defensively
            // before every request; acting on it would let a page add a
            // network nobody vetted.
            ports.respond(id, BrowserExecutor.resultJson(id: id, result: nil))

        case .read(let bundler):
            await read(id: id, method: method, paramsJson: paramsJson, bundler: bundler)

        case .unsupported:
            // **4900, not 4001.** The person did not decline.
            ports.respond(id, BrowserExecutor.errorJson(
                id: id, code: 4900, message: "Vela cannot answer \(method) yet"
            ))
        }
    }

    private func read(id: String, method: String, paramsJson: String, bundler: Bool) async {
        var params = Self.array(paramsJson)

        if method == "eth_getTransactionReceipt" || method == "eth_getTransactionByHash",
           let asked = params.first as? String, asked.hasPrefix("0x") {
            switch await ports.receiptFor(asked) {
            case .pending:
                // Ours, and not landed. `null` is what a node says about a
                // transaction it has not seen, which is exactly true here.
                ports.respond(id, BrowserExecutor.resultJson(id: id, result: nil))
                return
            case .landed(let txHash):
                params = [txHash]
            case nil:
                break
            }
        }

        let body = await ports.poolCall(ports.browserChain(), method, params, bundler)
        guard let body else {
            ports.respond(id, BrowserExecutor.errorJson(
                id: id, code: -32603, message: "No endpoint answered"
            ))
            return
        }
        if let error = body["error"] as? [String: Any] {
            let message = (error["message"] as? String).flatMap { $0.isEmpty ? nil : $0 }
                ?? "The node refused this call"
            ports.respond(id, BrowserExecutor.errorJson(
                id: id,
                code: (error["code"] as? NSNumber)?.intValue ?? -32603,
                message: message
            ))
            return
        }
        ports.respond(id, BrowserExecutor.resultJson(id: id, result: body["result"]))
    }

    static func array(_ json: String) -> [Any] {
        guard let data = json.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any]
        else { return [] }
        return params
    }
}
