package app.getvela.wallet.feature.send.core

import okhttp3.Request

/**
 * The header the relay reads a chain's RPC from (`vela-relay/src/utils/rpc.rs`).
 *
 * Spec 098 §5. Before spec 081 the app sent `X-Rpc-Url`, a name the relay
 * never read; 081 removed it as inert, and a network the relay's directory
 * cannot reach could then never be probed, quoted or served — the send went on
 * without a word. The URL may carry a provider API key: ruled acceptable
 * (098 §0.1) and said where the person sets an RPC or a provider key.
 *
 * To the relay only. The core sets `x_rpc_url` on bundler calls alone, so an
 * RPC provider is never sent one.
 */
const val RELAY_RPC_URL_HEADER = "x-vela-rpc-url"

/** Names the chain's RPC to the relay when there is one; otherwise leaves the request as it was. */
fun Request.Builder.relayRpcHeader(url: String?): Request.Builder =
    if (url.isNullOrEmpty()) this else header(RELAY_RPC_URL_HEADER, url)
