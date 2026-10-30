/**
 * The wasm side of the native-parity corpus (F0.2):
 * `crates/vela-core/tests/vectors/native-parity/exports.json`.
 *
 * Every case there names a function by its uniffi name and carries its
 * arguments as the uniffi export takes them (`*_json` keys are JSON TEXT, as a
 * shell sends it). This table calls the web's export of the same rule and
 * reduces the answer to the shape the vector records — JSON text parsed
 * (`""` kept as `""`, `undefined` as `null`), a refusal as `{ refused: code }`,
 * an accepted signature as `{ accepted: {…hex} }` — so the Rust, Kotlin and
 * Swift runners can hold the uniffi exports to the very same values.
 *
 * Used by `verify-web.mjs` (which replays the corpus through the SHIPPED wasm)
 * and by whoever regenerates the corpus from it.
 */

const json = (text) => (text === undefined || text === null ? null : text === '' ? '' : JSON.parse(text));
const unhex = (hex) => String(hex).replace(/^0x/i, '').toLowerCase();
const keysArg = (keys) => keys.map((k) => ({ credentialId: k.credential_id, publicKeyHex: k.public_key_hex }));
const opt = (value) => (value === undefined ? null : value);

/** `{ refused: code }` for a refusal the export threw as its code, else rethrow. */
function refusal(thrown) {
  if (typeof thrown === 'string') return { refused: thrown };
  throw thrown;
}

export function nativeParityDispatch(wasm) {
  return {
    // --- where a signature goes ---
    signing_route: (i) => json(wasm.signingRoute(i.account_json, i.method)),
    signing_venue_block: (i) => json(wasm.signingVenueBlock(i.domain, i.venue_json)),
    multisig_approval_route: (i) => json(wasm.multisigApprovalRoute(i.account_json)),

    // --- the trusted signing page ---
    trusted_signer_signature_request: (i) =>
      json(
        wasm.trustedSignerSignatureRequest(
          JSON.stringify({
            method: i.method,
            params: i.method ? JSON.parse(i.params_json) : null,
            origin: i.origin,
            originSeenByBrowser: i.origin_seen_by_browser,
            chainId: i.chain_id,
            chainName: opt(i.chain_name),
            nativeSymbol: opt(i.native_symbol),
            account: i.account,
            accountName: opt(i.account_name),
            credentialIdsHex: i.credential_ids_hex,
            keyRoute: i.key_route_json ? JSON.parse(i.key_route_json) : null,
            calls: [],
          }),
        ),
      ),
    trusted_signer_signature_accept: (i) => {
      try {
        const out = JSON.parse(
          wasm.trustedSignerSignatureAccept(
            JSON.stringify({
              request: JSON.parse(i.request_json),
              result: JSON.parse(i.result_json),
              keys: keysArg(i.keys),
              digestHex: i.digest_hex,
            }),
          ),
        );
        return {
          accepted: {
            credential_id_hex: unhex(out.credentialId),
            signature_der_hex: unhex(out.signatureHex),
            authenticator_data_hex: unhex(out.authenticatorDataHex),
            client_data_json_hex: unhex(out.clientDataJSONHex),
          },
        };
      } catch (thrown) {
        return refusal(thrown);
      }
    },
    trusted_signer_approval_request: (i) =>
      json(
        wasm.trustedSignerApprovalRequest(
          JSON.stringify({
            chainId: i.chain_id,
            chainName: opt(i.chain_name),
            nativeSymbol: opt(i.native_symbol),
            multisig: i.multisig,
            multisigName: opt(i.multisig_name),
            tx: JSON.parse(i.tx_json),
            member: i.member,
            memberName: opt(i.member_name),
            credentialIdsHex: i.credential_ids_hex,
            keyRoute: i.key_route_json ? JSON.parse(i.key_route_json) : null,
          }),
        ),
      ),
    trusted_signer_approval_accept: (i) => {
      try {
        return {
          accepted: JSON.parse(
            wasm.trustedSignerApprovalAccept(
              JSON.stringify({
                result: JSON.parse(i.result_json),
                chainId: i.chain_id,
                member: i.member,
                safeTxHash: i.safe_tx_hash,
                keys: keysArg(i.keys),
              }),
            ),
          ),
        };
      } catch (thrown) {
        return refusal(thrown);
      }
    },

    // --- how a dApp request is answered ---
    sign_error_words: (i) => opt(wasm.signErrorWords(i.kind, opt(i.detail))),
    sign_not_confirmed_detail: (i) => wasm.signNotConfirmedDetail(i.user_op_hash),
    sign_answered: (i) => opt(wasm.signAnswered(i.payload_json)),
    sign_request_ttl_ms: () => wasm.signRequestTtlMs(),

    // --- the agent ---
    intent_agent_reads: (i) => json(wasm.intentAgentReads(i.world_json, opt(i.operation))),
    intent_advice: (i) => json(wasm.intentAdvice(i.input_json)),
    intent_advice_standing: (i) => json(wasm.intentAdviceStanding(i.input_json)),

    // --- the read path ---
    rpc_own_only_pool: (i) => json(wasm.rpcOwnOnlyPool(i.seeds_json, i.own_only)),
    rpc_hedge_after_ms: () => wasm.rpcHedgeAfterMs(),
    rpc_is_hedged_read: (i) => wasm.rpcIsHedgedRead(i.kind, i.method),
    rpc_early_verdict: (i) => json(wasm.rpcEarlyVerdict(i.kind, i.method, i.url, i.outcome_json)),
    rpc_read_timeout_ms: () => wasm.rpcReadTimeoutMs(),
    rpc_cooldown_ms: (i) => wasm.rpcCooldownMs(i.consecutive_failures),

    // --- fees and gas ---
    min_gas_price_wei: (i) => wasm.minGasPriceWei(i.chain_id),
    fee_quote_deadline_ms: () => wasm.feeQuoteDeadlineMs(),
    typical_inclusion_seconds: (i) => wasm.typicalInclusionSeconds(i.chain_id),
    user_op_gas_limits: (i) =>
      json(
        wasm.userOpGasLimits(
          i.chain_id,
          i.deployed,
          i.sub_calls,
          i.verification_gas_limit,
          i.call_gas_limit,
          i.pre_verification_gas,
          opt(i.settlement_gas),
          opt(i.inner_floor),
        ),
      ),

    // --- dApp RPC ---
    dapp_rpc_classify: (i) => json(wasm.dappRpcClassify(i.method)),
    dapp_rpc_capabilities: (i) => json(wasm.dappRpcCapabilities(i.params_json, i.granted_json, i.chains_json)),
    dapp_rpc_calls_status: (i) =>
      json(wasm.dappRpcCallsStatus(i.id, i.chain_id, opt(i.receipt_json), opt(i.relay_status_json))),

    // --- tokens, sites, names, currencies ---
    token_provenance: (i) => json(wasm.tokenProvenance(i.chain_id, i.token, i.symbol, i.native_symbol)),
    site_start_chain: (i) => wasm.siteStartChain(Uint32Array.from(i.space_chain_ids), opt(i.last_used)),
    resolved_name_step: (i) => json(wasm.resolvedNameStep(i.chain_id, i.registry, i.name, i.answers_json)),
    currency_fraction_digits: (i) => wasm.currencyFractionDigits(i.code),

    // --- core asks 1 ---
    // The `RelayRejection` wire JSON: "nonce_used", { other: … }, …
    classify_relay_rejection: (i) => json(wasm.classifyRelayRejection(i.message)),
    is_localhost_http: (i) => wasm.isLocalhostHttp(i.url),
    passkey_ceremony_end: (i) => wasm.passkeyCeremonyEnd(i.kind, i.elapsed_ms),
    passkey_sign_timeout_ms: () => wasm.passkeySignTimeoutMs(),
    rpc_node_standing: (i) => wasm.rpcNodeStanding(i.view_json, i.chain_id),
    // The verdict record, as uniffi's `SimOutcomeRecord` carries it (its
    // `deltas_json` parsed as `deltas`).
    sim_outcome: (i) => json(wasm.simOutcome(i.user, i.reply_json)),
  };
}
