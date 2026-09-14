//
//  DappRpc.swift
//  VelaWallet
//
//  Which of six answers a method gets.
//
//  A Swift port of `app-android/.../feature/browser/core/DappRpc.kt`, which is
//  a port of the desktop's `executor/dapp_rpc.rs`, which is a port of
//  `app-web/vela-wallet/extension/lib/protocol.js`'s `classifyMethod`.
//  `DappRpcParityTest` reads that script and fails when the three sets differ
//  — which is the only reason this table may be duplicated at all.
//
//  ## An allowlist, and the reason it is not a denylist
//
//  A method outside the list is refused, never forwarded. Denylist routing
//  fails OPEN, and `eth_signTransaction` is exactly the method it fails open
//  on: it is not caught by `dappIsSigningMethod`, so a catch-all "read" bucket
//  would proxy it to a public node and turn this browser into an open RPC
//  relay for any site that asks.
//
//  ## Three methods are deliberately absent
//
//  `eth_accounts`, `eth_requestAccounts` and `wallet_getPermissions` never
//  reach here: the permissions machine answers those from its own grant
//  mirror, and this table is only ever asked about what it forwarded.
//

import Foundation
import VelaCore

enum DappRoute: Equatable {
    /// The signing sheet.
    case sign
    /// The wallet's own answer about the chain.
    case state
    case switchChain
    /// Acknowledged; nothing changes.
    case ack
    /// A node or bundler read.
    case read(bundler: Bool)
    /// Refused. **4900, not 4001** — the person did not decline.
    case unsupported
}

enum DappRpc {

    /// The node reads the wallet itself advertises.
    static let readOnlyRpcMethods: Set<String> = [
        "eth_call", "eth_estimateGas", "eth_getBalance", "eth_getCode", "eth_getStorageAt",
        "eth_getTransactionCount", "eth_getTransactionByHash", "eth_getTransactionReceipt",
        "eth_getLogs", "eth_blockNumber", "eth_getBlockByNumber", "eth_getBlockByHash",
        "eth_feeHistory", "eth_gasPrice", "eth_maxPriorityFeePerGas", "eth_newFilter",
        "eth_newBlockFilter", "eth_getFilterChanges", "eth_uninstallFilter",
        "eth_sendRawTransaction", "eth_syncing",
    ]

    /// Routed to the ERC-4337 bundler, not the node.
    static let bundlerMethods: Set<String> = [
        "eth_sendUserOperation", "eth_estimateUserOperationGas", "eth_getUserOperationReceipt",
        "eth_getUserOperationByHash", "pimlico_getUserOperationGasPrice",
    ]

    /// Reads the wallet does not use itself but will proxy.
    static let extraReadMethods: Set<String> = [
        "eth_getBlockReceipts", "eth_getProof", "eth_createAccessList", "eth_getFilterLogs",
        "eth_getTransactionByBlockHashAndIndex", "eth_getTransactionByBlockNumberAndIndex",
        "eth_getBlockTransactionCountByHash", "eth_getBlockTransactionCountByNumber",
        "web3_clientVersion",
    ]

    static func route(_ method: String) -> DappRoute {
        // Refused outright, **before** the signing test that would otherwise
        // catch it. `eth_sign` puts an opaque digest in front of somebody, and
        // this wallet refuses it as policy rather than as a missing feature.
        if method == "eth_sign" { return .unsupported }
        if dappIsSigningMethod(method: method) { return .sign }
        if method == "eth_chainId" || method == "net_version" { return .state }
        if method == "wallet_switchEthereumChain" { return .switchChain }
        if method == "wallet_addEthereumChain" || method == "wallet_watchAsset" { return .ack }
        if bundlerMethods.contains(method) { return .read(bundler: true) }
        if readOnlyRpcMethods.contains(method) || extraReadMethods.contains(method) {
            return .read(bundler: false)
        }
        return .unsupported
    }

    /// `[{ chainId: "0x64" }]` or `[{ chainId: "100" }]` → 100; anything else
    /// → `nil`.
    static func switchChainParam(_ paramsJson: String) -> Int? {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any],
              let first = params.first as? [String: Any],
              let raw = first["chainId"]
        else { return nil }

        let text: String
        if let string = raw as? String { text = string }
        else if let number = raw as? NSNumber { text = number.stringValue }
        else { return nil }

        let stripped = text.hasPrefix("0x") || text.hasPrefix("0X")
            ? String(text.dropFirst(2)) : text
        let value = stripped.count != text.count
            ? Int(stripped, radix: 16)
            : Int(text)
        guard let value, value >= 0, value <= 4_294_967_295 else { return nil }
        return value
    }

    static func hexChainId(_ chainId: Int) -> String { "0x" + String(chainId, radix: 16) }
}
