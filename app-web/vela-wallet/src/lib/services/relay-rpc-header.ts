/**
 * The RPC the wallet uses for a chain, told to the relay (spec 098 §5).
 *
 * The relay reads every chain through the RPC a request names here first, then
 * its own directory (`vela-relay/src/utils/rpc.rs`). Without it, a network the
 * person added — one the relay's directory cannot reach — could not be probed,
 * quoted or served at all, and the send went on without a word (098 §1).
 *
 * The name is the one the relay reads. Before spec 081 the apps sent
 * `X-Rpc-Url`, which the relay never read; 081 removed it as inert. This is the
 * header that was always meant.
 *
 * The URL can carry a provider API key. That is ruled acceptable (098 §0.1) and
 * said where the person sets an RPC or a provider key, and in the privacy
 * policy. It goes to the RELAY only — never to an RPC provider: the core sets
 * `x_rpc_url` on bundler calls alone.
 */
export const RELAY_RPC_URL_HEADER = 'x-vela-rpc-url';

/** `{ 'x-vela-rpc-url': url }`, or nothing when there is no URL to name. */
export function relayRpcHeader(url: string | null | undefined): Record<string, string> {
	return url ? { [RELAY_RPC_URL_HEADER]: url } : {};
}
