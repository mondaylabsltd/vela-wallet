package app.getvela.wallet.feature.browser.core

import org.json.JSONArray
import uniffi.vela_core_uniffi.dappIsSigningMethod

/**
 * Who answers a request `dapp_permissions` forwarded (spec 044 T012).
 *
 * The core routes on PERMISSION — connected or not, this frame or another,
 * secure origin or not — and then forwards everything it does not answer
 * itself. That "everything" is three different things: a signature, a fact
 * about the wallet, and a read of the chain. Deciding between them is a
 * shell job in every client: the extension's service worker does it, the
 * desktop's `executor/dapp_rpc.rs` does it, and this is the same table.
 *
 * **Ported from** `app-web/vela-wallet/extension/lib/protocol.js`
 * (`BUNDLER_METHODS`, `READ_ONLY_RPC_METHODS`, `READ_PROXY_METHODS`). That
 * file ships INSIDE this app already — the page's own provider is built
 * from it — so `DappRpcParityTest` parses it and fails if the two ever
 * disagree.
 *
 * ## Why an allowlist
 *
 * Routing by denylist fails OPEN: `eth_signTransaction` is not caught by
 * any "is this a signing method" test, so a catch-all read bucket would hand
 * it to a public node — and a wallet that proxies arbitrary methods for any
 * site it renders is an open RPC relay wearing a wallet's name.
 */
object DappRpc {
    sealed class Route {
        /** A signature: forwarded to the signing machines. */
        data object Sign : Route()

        /** A fact about the wallet (`eth_chainId`, `net_version`), answered here. */
        data object State : Route()

        /** `wallet_switchEthereumChain`. */
        data object Switch : Route()

        /**
         * `wallet_addEthereumChain` / `wallet_watchAsset`: acknowledged and
         * changed nothing — settings remain the only place a network or a
         * token is added. Saying "no" would break sites that switch after
         * adding; saying "yes" and changing nothing is what the extension ships.
         */
        data object Ack : Route()

        /** A node or bundler read, on the page's current chain. */
        data class Read(val bundler: Boolean) : Route()

        /** Refused. Every method outside the allowlist lands here. */
        data object Unsupported : Route()
    }

    /** The reads the app itself advertises (`READ_ONLY_RPC_METHODS`). */
    val READ_ONLY_RPC_METHODS: List<String> = listOf(
        "eth_call", "eth_estimateGas", "eth_getBalance", "eth_getCode", "eth_getStorageAt",
        "eth_getTransactionCount", "eth_getTransactionByHash", "eth_getTransactionReceipt", "eth_getLogs",
        "eth_blockNumber", "eth_getBlockByNumber", "eth_getBlockByHash", "eth_feeHistory", "eth_gasPrice",
        "eth_maxPriorityFeePerGas", "eth_newFilter", "eth_newBlockFilter", "eth_getFilterChanges",
        "eth_uninstallFilter", "eth_sendRawTransaction", "eth_syncing",
    )

    /** The 4337 reads the pool routes to the bundler. */
    val BUNDLER_METHODS: List<String> = listOf(
        "eth_sendUserOperation", "eth_estimateUserOperationGas", "eth_getUserOperationReceipt",
        "eth_getUserOperationByHash", "pimlico_getUserOperationGasPrice",
    )

    /** The rest of `READ_PROXY_METHODS`: proxied, never advertised. */
    val EXTRA_READ_METHODS: List<String> = listOf(
        "eth_getBlockReceipts", "eth_getProof", "eth_createAccessList", "eth_getFilterLogs",
        "eth_getTransactionByBlockHashAndIndex", "eth_getTransactionByBlockNumberAndIndex",
        "eth_getBlockTransactionCountByHash", "eth_getBlockTransactionCountByNumber", "web3_clientVersion",
    )

    /**
     * `classifyMethod`, with `eth_accounts` / `wallet_getPermissions` /
     * `eth_requestAccounts` absent on purpose: the core answers those, and
     * this is only ever asked about what the core forwarded.
     */
    fun route(method: String): Route {
        // Refused outright, before the signing test that would otherwise catch
        // it: `eth_sign` puts an opaque digest in front of somebody, and the
        // JS refuses it as policy rather than as a missing feature.
        if (method == "eth_sign") return Route.Unsupported
        if (dappIsSigningMethod(method)) return Route.Sign
        if (method == "eth_chainId" || method == "net_version") return Route.State
        if (method == "wallet_switchEthereumChain") return Route.Switch
        if (method == "wallet_addEthereumChain" || method == "wallet_watchAsset") return Route.Ack
        if (method in BUNDLER_METHODS) return Route.Read(bundler = true)
        if (method in READ_ONLY_RPC_METHODS || method in EXTRA_READ_METHODS) return Route.Read(bundler = false)
        return Route.Unsupported
    }

    /** `[{ chainId: "0x64" }]` or `[{ chainId: "100" }]` → 100; anything else → null. */
    fun switchChainParam(paramsJson: String): Int? {
        val params = runCatching { JSONArray(paramsJson) }.getOrNull() ?: return null
        val text = params.optJSONObject(0)?.opt("chainId")?.toString() ?: return null
        val hex = text.removePrefix("0x").removePrefix("0X")
        val value = if (hex.length != text.length) hex.toLongOrNull(16) else text.toLongOrNull()
        return value?.takeIf { it in 0..4_294_967_295L }?.toInt()
    }

    fun hexChainId(chainId: Int): String = "0x" + chainId.toLong().toString(16)
}
