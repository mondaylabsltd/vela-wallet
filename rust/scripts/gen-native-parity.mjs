#!/usr/bin/env node
/**
 * Regenerate the F0.2 native-parity corpus,
 * `crates/vela-core/tests/vectors/native-parity/exports.json`, from the SHIPPED
 * wasm (rust/pkg-web + assets/wasm). The inputs are written here; every
 * expectation is the wasm's answer, reduced by `native-parity-wasm.mjs`.
 *
 *   node rust/scripts/gen-native-parity.mjs
 *
 * Run it after adding an export or a case, or after a core change that is meant
 * to move an answer (rebuild rust/pkg-web first: the file records what the web
 * ships). Then `cargo test -p vela-core-uniffi native_parity`, smoke-kotlin.sh
 * and smoke-swift.sh must still agree with it.
 *
 * The trusted-signer cases sign with a P-256 key generated per run and thrown
 * away, so a regeneration rewrites those cases' bytes; that is expected.
 */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash, generateKeyPairSync, sign as cryptoSign } from 'node:crypto';

const RUST = dirname(dirname(fileURLToPath(import.meta.url)));
const { loadShippedCore } = await import('./load-wasm-node.mjs');
const { nativeParityDispatch } = await import('./native-parity-wasm.mjs');
const wasm = await loadShippedCore();
const D = nativeParityDispatch(wasm);

const fx = (name) => readFileSync(join(RUST, 'crates/vela-core/tests/fixtures/spec102', name), 'utf8');
const hex = (b) => Buffer.from(b).toString('hex');
const b64u = (b) => Buffer.from(b).toString('base64url');
const sha256 = (b) => createHash('sha256').update(b).digest();
const keccak = (b) => Buffer.from(wasm.keccak256(Uint8Array.from(b)));
const S = JSON.stringify;

// --- a P-256 key that signs WebAuthn assertions --------------------------------
function key(seedLabel) {
  const { privateKey, publicKey } = generateKeyPairSync('ec', { namedCurve: 'P-256' });
  const jwk = publicKey.export({ format: 'jwk' });
  const x = Buffer.from(jwk.x, 'base64url');
  const y = Buffer.from(jwk.y, 'base64url');
  return { privateKey, x, y, pubHex: '04' + hex(x) + hex(y), credId: hex(sha256(Buffer.from(seedLabel))).slice(0, 32) };
}
function assertion(k, digest, { uv = true } = {}) {
  const authData = Buffer.concat([sha256(Buffer.from('getvela.app')), Buffer.from([uv ? 0x05 : 0x01]), Buffer.from([0, 0, 0, 7])]);
  const clientData = Buffer.from(
    `{"type":"webauthn.get","challenge":"${b64u(digest)}","origin":"https://sign.getvela.app","crossOrigin":false}`,
  );
  const sig = cryptoSign('sha256', Buffer.concat([authData, sha256(clientData)]), { key: k.privateKey, dsaEncoding: 'ieee-p1363' });
  return {
    credentialId: b64u(Buffer.from(k.credId, 'hex')),
    signature: '0x' + hex(sig),
    authenticatorData: '0x' + hex(authData),
    clientDataJSON: '0x' + hex(clientData),
  };
}
const safeOf = (k) => wasm.computeSafeAddressMulti(Uint8Array.from(Buffer.concat([k.x, k.y]))).address;
function word(n) {
  const b = Buffer.alloc(32);
  b.writeBigUInt64BE(BigInt(n), 24);
  return b;
}
function addrWord(a) {
  return Buffer.concat([Buffer.alloc(12), Buffer.from(a.slice(2), 'hex')]);
}
function approvalChallenge(chainId, owner, safeTxHash) {
  const domainType = keccak(Buffer.from('EIP712Domain(uint256 chainId,address verifyingContract)'));
  const domain = keccak(Buffer.concat([domainType, word(chainId), addrWord(owner)]));
  const struct = keccak(Buffer.concat([keccak(Buffer.from('SafeMessage(bytes message)')), Buffer.from(safeTxHash.slice(2), 'hex')]));
  return keccak(Buffer.concat([Buffer.from([0x19, 0x01]), domain, struct]));
}

const cases = [];
const add = (name, fn, input) => cases.push({ name, fn, input });

// --- signing_route / multisig_approval_route / signing_venue_block -------------
const accounts = {
  official: fx('official-page-account.json'),
  app_only: fx('app-only-account.json'),
  custom: fx('custom-origin-account.json'),
  before_page: fx('before-sign-in-key-page.json'),
};
for (const [label, account] of Object.entries(accounts)) {
  for (const method of ['', 'eth_sendTransaction', 'personal_sign', 'eth_signTypedData_v4', 'wallet_sendCalls', 'vela_multisigExecute', 'vela_multisigApprove', 'eth_sign']) {
    add(`signing_route/${label}/${method || 'own_send'}`, 'signing_route', { account_json: account, method });
  }
  add(`multisig_approval_route/${label}`, 'multisig_approval_route', { account_json: account });
}
add('signing_route/unreadable', 'signing_route', { account_json: '{"id":1}', method: 'personal_sign' });
add('multisig_approval_route/unreadable', 'multisig_approval_route', { account_json: 'not json' });
for (const [domain, venue] of [
  ['getvela.app', { type: 'in_vela' }],
  ['getvela.app', { type: 'page', url: 'https://sign.getvela.app/' }],
  ['getvela.app', { type: 'page', url: 'http://localhost:8140/' }],
  ['localhost', { type: 'page', url: 'https://sign.getvela.app/' }],
  ['localhost', { type: 'in_vela' }],
  ['localhost', { type: 'page', url: 'http://localhost:8140/' }],
]) {
  add(`signing_venue_block/${domain}/${venue.type}/${venue.url ?? ''}`, 'signing_venue_block', { domain, venue_json: S(venue) });
}
add('signing_venue_block/unreadable', 'signing_venue_block', { domain: 'getvela.app', venue_json: '{"type":"elsewhere"}' });

// --- the trusted signing page: a message, accepted and refused -----------------
const CHAIN = 100;
const signer = key('vela-native-parity-signer');
const other = key('vela-native-parity-other');
const account = safeOf(signer);
const keys = [{ credential_id: signer.credId, public_key_hex: signer.pubHex }];
const params = S(['0x48656c6c6f2c2056656c61', account]);
const requestInput = {
  method: 'personal_sign',
  params_json: params,
  origin: 'https://app.example',
  origin_seen_by_browser: true,
  chain_id: CHAIN,
  chain_name: 'Gnosis',
  native_symbol: 'xDAI',
  account,
  account_name: 'Savings',
  credential_ids_hex: [signer.credId],
  key_route_json: S({ credential_id: signer.credId, method: 'trusted_signer', transports: 'internal,hybrid', hints: ['client-device'] }),
};
add('trusted_signer_signature_request/personal_sign', 'trusted_signer_signature_request', requestInput);
add('trusted_signer_signature_request/no_route', 'trusted_signer_signature_request', { ...requestInput, key_route_json: null, origin_seen_by_browser: false });
const request = S(D.trusted_signer_signature_request(requestInput));
const original = Buffer.from(wasm.signMessageHash('personal_sign', params));
const digest = Buffer.from(wasm.attestSafeMessageHash(Uint8Array.from(original), BigInt(CHAIN), account));
const good = assertion(signer, digest);
add('trusted_signer_signature_accept/accepted', 'trusted_signer_signature_accept', {
  request_json: request, result_json: S(good), keys, digest_hex: '0x' + hex(digest),
});
add('trusted_signer_signature_accept/shell_signs_other_bytes', 'trusted_signer_signature_accept', {
  request_json: request, result_json: S(good), keys, digest_hex: '0x' + hex(sha256(Buffer.from('other'))),
});
add('trusted_signer_signature_accept/foreign_key', 'trusted_signer_signature_accept', {
  request_json: request, result_json: S(assertion(other, digest)), keys, digest_hex: '0x' + hex(digest),
});
add('trusted_signer_signature_accept/not_user_verified', 'trusted_signer_signature_accept', {
  request_json: request, result_json: S(assertion(signer, digest, { uv: false })), keys, digest_hex: '0x' + hex(digest),
});
{
  const tampered = { ...good, signature: good.signature.slice(0, -2) + (good.signature.endsWith('00') ? '01' : '00') };
  add('trusted_signer_signature_accept/bad_signature', 'trusted_signer_signature_accept', {
    request_json: request, result_json: S(tampered), keys, digest_hex: '0x' + hex(digest),
  });
}
add('trusted_signer_signature_accept/signed_another_challenge', 'trusted_signer_signature_accept', {
  request_json: request, result_json: S(assertion(signer, sha256(Buffer.from('x')))), keys, digest_hex: '0x' + hex(digest),
});

// --- a member's approval -------------------------------------------------------
const MULTISIG = '0x4c3FF6Bc7d1a4b7f4e0e93B4C61b8b1a7e5dF2A9';
const tx = {
  to: '0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69', value: '0xde0b6b3a7640000', data: '0x', operation: 0,
  safeTxGas: '0x0', baseGas: '0x0', gasPrice: '0x0', gasToken: '0x0000000000000000000000000000000000000000',
  refundReceiver: '0x0000000000000000000000000000000000000000', nonce: '0x3',
};
const approvalInput = {
  chain_id: CHAIN, chain_name: 'Gnosis', native_symbol: 'xDAI', multisig: MULTISIG, multisig_name: 'Team vault',
  tx_json: S(tx), member: account, member_name: 'Savings', credential_ids_hex: [signer.credId],
  key_route_json: S({ credential_id: signer.credId, method: 'trusted_signer', transports: 'internal', hints: [] }),
};
add('trusted_signer_approval_request/named', 'trusted_signer_approval_request', approvalInput);
add('trusted_signer_approval_request/unnamed_no_route', 'trusted_signer_approval_request', {
  ...approvalInput, multisig_name: null, member_name: null, key_route_json: null, chain_name: null, native_symbol: null,
});
const approvalRequest = D.trusted_signer_approval_request(approvalInput);
const safeTxHash = JSON.stringify(approvalRequest).match(/"safeTxHash":"(0x[0-9a-fA-F]{64})"/)[1];
const challenge = approvalChallenge(CHAIN, account, safeTxHash);
add('trusted_signer_approval_accept/accepted', 'trusted_signer_approval_accept', {
  result_json: S(assertion(signer, challenge)), chain_id: CHAIN, member: account, safe_tx_hash: safeTxHash, keys,
});
add('trusted_signer_approval_accept/another_tx', 'trusted_signer_approval_accept', {
  result_json: S(assertion(signer, challenge)), chain_id: CHAIN, member: account, safe_tx_hash: '0x' + 'ab'.repeat(32), keys,
});
add('trusted_signer_approval_accept/foreign_key', 'trusted_signer_approval_accept', {
  result_json: S(assertion(other, challenge)), chain_id: CHAIN, member: account, safe_tx_hash: safeTxHash, keys,
});
add('trusted_signer_approval_accept/not_user_verified', 'trusted_signer_approval_accept', {
  result_json: S(assertion(signer, challenge, { uv: false })), chain_id: CHAIN, member: account, safe_tx_hash: safeTxHash, keys,
});

// --- a dApp request's answer ----------------------------------------------------
for (const kind of ['user_rejected', 'wallet_switched_chains', 'unsupported_chain', 'unauthorized_account', 'invalid_params',
  'unsupported_capability', 'unlimited_approval', 'self_call_blocked', 'funding_cancelled', 'submit_failed', 'stale_fee_quote', 'no_such_kind']) {
  add(`sign_error_words/${kind}`, 'sign_error_words', { kind, detail: null });
}
add('sign_error_words/submit_failed_detail', 'sign_error_words', { kind: 'submit_failed', detail: 'bundler said no' });
add('sign_error_words/invalid_params_detail', 'sign_error_words', { kind: 'invalid_params', detail: 'missing to' });
add('sign_not_confirmed_detail', 'sign_not_confirmed_detail', { user_op_hash: '0x' + '12'.repeat(32) });
for (const [label, payload] of [
  ['hash', { type: 'ok', result: '0x' + '34'.repeat(32) }],
  ['batch_object', { type: 'ok', result: { id: '0x' + '56'.repeat(32) } }],
  ['null', { type: 'ok', result: null }],
  ['error', { type: 'err', code: 4001, kind: 'user_rejected', message: 'User rejected the request.' }],
]) add(`sign_answered/${label}`, 'sign_answered', { payload_json: S(payload) });
add('sign_answered/unreadable', 'sign_answered', { payload_json: '{' });
add('sign_request_ttl_ms', 'sign_request_ttl_ms', {});

// --- the agent ------------------------------------------------------------------
const ME = '0x1111111111111111111111111111111111111111';
const world = (allowed) => ({ session: { address: ME, accounts: [{ account: { address: ME } }] }, agent_wallets: allowed ? [ME] : [] });
add('intent_agent_reads/review_allowed', 'intent_agent_reads', { world_json: S(world(true)), operation: null });
add('intent_agent_reads/review_not_allowed', 'intent_agent_reads', { world_json: S(world(false)), operation: null });
add('intent_agent_reads/wait_unknown_operation', 'intent_agent_reads', { world_json: S(world(true)), operation: 'op-1' });
add('intent_agent_reads/unreadable_world', 'intent_agent_reads', { world_json: 'nope', operation: null });
const fee = (raw) => ({ state: 'quoted', amount: '0.001', amount_raw: raw, decimals: 18, symbol: 'xDAI', token: null });
const advice = (current, args, allowed = true, currentFee = null) => S({ world: world(allowed), current_digest: current, current_fee: currentFee, args });
add('intent_advice/sanitized', 'intent_advice', { input_json: advice('0xabc', {
  review_digest: '0xabc', verdict: 'danger', summary: '‮Drains​ your\n\nUSDC',
  points: ['one', '', 'two\u0007', 'three', 'four', 'five', 'six'],
  sources: ['https://user:pw@Etherscan.io:443/address/0x1?a=b', 'javascript:alert(1)', 'not a url'],
}) });
add('intent_advice/earlier_version', 'intent_advice', { input_json: advice('0xdef', { review_digest: '0xabc', verdict: 'looks_fine', summary: 'ok' }) });
add('intent_advice/with_fee', 'intent_advice', { input_json: advice('0xabc', { review_digest: '0xabc', verdict: 'caution', summary: 'Check it' }, true, fee('1000000000000000')) });
add('intent_advice/not_allowed', 'intent_advice', { input_json: advice('0xabc', { review_digest: '0xabc', verdict: 'caution', summary: 'Check it' }, false) });
add('intent_advice/no_review', 'intent_advice', { input_json: advice(null, { review_digest: '0xabc', verdict: 'caution', summary: 'Check it' }) });
add('intent_advice/bad_verdict', 'intent_advice', { input_json: advice('0xabc', { review_digest: '0xabc', verdict: 'approve_now', summary: 'x' }) });
add('intent_advice/gate_only', 'intent_advice', { input_json: S({ world: world(true) }) });
const given = D.intent_advice({ input_json: advice('0xabc', { review_digest: '0xabc', verdict: 'looks_fine', summary: 'ok' }, true, fee('1000000000000000')) }).advice;
for (const [label, digest, raw] of [['current', '0xabc', '1200000000000000'], ['fee_changed', '0xabc', '2000000000000000'], ['earlier_version', '0xdef', '1000000000000000']]) {
  add(`intent_advice_standing/${label}`, 'intent_advice_standing', { input_json: S({ advice: given, current_digest: digest, current_fee: fee(raw) }) });
}
add('intent_advice_standing/unreadable', 'intent_advice_standing', { input_json: '[]' });

// --- the read path ----------------------------------------------------------------
const seeds = [
  { url: 'https://rpc.mine.example', source: 'user' },
  { url: 'https://rpc.gnosischain.com', source: 'default' },
  { url: 'https://gnosis.public.example', source: 'public' },
];
add('rpc_own_only_pool/on', 'rpc_own_only_pool', { seeds_json: S(seeds), own_only: true });
add('rpc_own_only_pool/off', 'rpc_own_only_pool', { seeds_json: S(seeds), own_only: false });
add('rpc_own_only_pool/on_without_own', 'rpc_own_only_pool', { seeds_json: S(seeds.slice(1)), own_only: true });
add('rpc_own_only_pool/unreadable', 'rpc_own_only_pool', { seeds_json: '{', own_only: true });
add('rpc_hedge_after_ms', 'rpc_hedge_after_ms', {});
add('rpc_read_timeout_ms', 'rpc_read_timeout_ms', {});
for (const [kind, method] of [['rpc', 'eth_call'], ['rpc', 'eth_getBalance'], ['rpc', 'eth_sendRawTransaction'], ['bundler', 'eth_estimateUserOperationGas'], ['other', 'eth_call']]) {
  add(`rpc_is_hedged_read/${kind}/${method}`, 'rpc_is_hedged_read', { kind, method });
}
for (const [label, kind, method, outcome] of [
  ['answered', 'rpc', 'eth_call', { type: 'response', error: null }],
  ['reverted', 'rpc', 'eth_call', { type: 'response', error: { code: 3, message: 'execution reverted' } }],
  ['other_error', 'rpc', 'eth_call', { type: 'response', error: { code: -32000, message: 'header not found' } }],
  ['rate_limited', 'rpc', 'eth_call', { type: 'http_error', status: 429 }],
  ['timeout', 'rpc', 'eth_getBalance', { type: 'timeout' }],
  ['bundler', 'bundler', 'eth_getUserOperationReceipt', { type: 'response', error: null }],
  ['unknown_kind', 'ws', 'eth_call', { type: 'response', error: null }],
]) add(`rpc_early_verdict/${label}`, 'rpc_early_verdict', { kind, method, url: 'https://rpc.example', outcome_json: S(outcome) });
add('rpc_early_verdict/unreadable_outcome', 'rpc_early_verdict', { kind: 'rpc', method: 'eth_call', url: 'https://rpc.example', outcome_json: '{"type":"weird"}' });
for (const n of [0, 1, 2, 3, 4, 5, 10, 40]) add(`rpc_cooldown_ms/${n}`, 'rpc_cooldown_ms', { consecutive_failures: n });

// --- fees and gas -----------------------------------------------------------------
for (const chain of [1, 100, 8453, 5042002, 4217, 999999]) {
  add(`min_gas_price_wei/${chain}`, 'min_gas_price_wei', { chain_id: chain });
  add(`typical_inclusion_seconds/${chain}`, 'typical_inclusion_seconds', { chain_id: chain });
}
add('fee_quote_deadline_ms', 'fee_quote_deadline_ms', {});
for (const [label, chain, deployed, sub, settlement, inner] of [
  ['deployed', 100, true, 2, null, null],
  ['undeployed_no_settlement', 100, false, 2, null, null],
  ['undeployed_settlement', 100, false, 2, '150000', null],
  ['inner_floor_raises', 8453, true, 1, null, '900000'],
  ['inner_floor_below', 8453, true, 1, null, '1000'],
  ['tempo', 4217, true, 3, null, null],
]) {
  add(`user_op_gas_limits/${label}`, 'user_op_gas_limits', {
    chain_id: chain, deployed, sub_calls: sub, verification_gas_limit: '120000', call_gas_limit: '200000',
    pre_verification_gas: '55000', settlement_gas: settlement, inner_floor: inner,
  });
}

// --- dApp RPC -----------------------------------------------------------------------
for (const method of ['eth_call', 'eth_chainId', 'eth_accounts', 'eth_requestAccounts', 'eth_sendTransaction', 'personal_sign',
  'eth_signTypedData_v4', 'wallet_sendCalls', 'wallet_getCallsStatus', 'wallet_getCapabilities', 'wallet_switchEthereumChain',
  'wallet_addEthereumChain', 'eth_getUserOperationReceipt', 'eth_sign', 'eth_subscribe', 'made_up_method']) {
  add(`dapp_rpc_classify/${method}`, 'dapp_rpc_classify', { method });
}
const granted = S([ME]);
const chains = S([100, 8453, 1]);
add('dapp_rpc_capabilities/granted', 'dapp_rpc_capabilities', { params_json: S([ME]), granted_json: granted, chains_json: chains });
add('dapp_rpc_capabilities/chain_filter', 'dapp_rpc_capabilities', { params_json: S([ME, ['0x64']]), granted_json: granted, chains_json: chains });
add('dapp_rpc_capabilities/not_granted', 'dapp_rpc_capabilities', { params_json: S(['0x2222222222222222222222222222222222222222']), granted_json: granted, chains_json: chains });
add('dapp_rpc_capabilities/no_params', 'dapp_rpc_capabilities', { params_json: '[]', granted_json: granted, chains_json: chains });
const ID = '0x' + '9a'.repeat(32);
const receipt = (success) => ({
  userOpHash: ID, success, actualGasCost: '0x1', actualGasUsed: '0x5208',
  logs: [{ address: ME, topics: ['0x' + '01'.repeat(32)], data: '0x' }],
  receipt: { transactionHash: '0x' + '7b'.repeat(32), blockHash: '0x' + '8c'.repeat(32), blockNumber: '0x10', gasUsed: '0x5208', status: success ? '0x1' : '0x0', logs: [] },
});
add('dapp_rpc_calls_status/landed', 'dapp_rpc_calls_status', { id: ID, chain_id: 100, receipt_json: S(receipt(true)), relay_status_json: null });
add('dapp_rpc_calls_status/reverted', 'dapp_rpc_calls_status', { id: ID, chain_id: 100, receipt_json: S(receipt(false)), relay_status_json: null });
add('dapp_rpc_calls_status/pending', 'dapp_rpc_calls_status', { id: ID, chain_id: 8453, receipt_json: null, relay_status_json: null });
add('dapp_rpc_calls_status/no_receipt_null', 'dapp_rpc_calls_status', { id: ID, chain_id: 8453, receipt_json: 'null', relay_status_json: S({ status: 'not_found' }) });
add('dapp_rpc_calls_status/relay_rejected', 'dapp_rpc_calls_status', { id: ID, chain_id: 8453, receipt_json: null, relay_status_json: S({ status: 'rejected' }) });
add('dapp_rpc_calls_status/relay_failed', 'dapp_rpc_calls_status', { id: ID, chain_id: 8453, receipt_json: null, relay_status_json: S({ status: 'failed' }) });

// --- tokens, sites, names, currencies ---------------------------------------------------
for (const [label, chain, token, symbol, native] of [
  ['native', 100, '', 'xDAI', 'xDAI'],
  ['base_usdc', 8453, '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913', 'USDC', 'ETH'],
  ['imitation_usdc', 8453, '0x00000000000000000000000000000000deadbeef', 'USDC', 'ETH'],
  ['arc_usdc_coin', 5042002, '', 'USDC', 'USDC'],
  ['arc_usdc_contract', 5042002, '0x00000000000000000000000000000000c0ffee00', 'USDC', 'USDC'],
  ['unknown_token', 1, '0x00000000000000000000000000000000000000aa', 'PEPE', 'ETH'],
  ['imitates_native', 8453, '0x00000000000000000000000000000000000000bb', 'ETH', 'ETH'],
]) add(`token_provenance/${label}`, 'token_provenance', { chain_id: chain, token, symbol, native_symbol: native });
for (const [label, ids, last] of [['first', [100, 8453, 1], null], ['last_used', [100, 8453, 1], 8453], ['last_used_gone', [100, 8453], 1], ['empty', [], null], ['empty_last', [], 10]]) {
  add(`site_start_chain/${label}`, 'site_start_chain', { space_chain_ids: ids, last_used: last });
}
const ENS = '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e';
add('resolved_name_step/first_question', 'resolved_name_step', { chain_id: 1, registry: ENS, name: 'alice.eth', answers_json: '[]' });
add('resolved_name_step/not_a_name', 'resolved_name_step', { chain_id: 1, registry: ENS, name: 'not a name', answers_json: '[]' });
add('resolved_name_step/unreadable_answers', 'resolved_name_step', { chain_id: 1, registry: ENS, name: 'alice.eth', answers_json: '{' });
for (const code of ['USD', 'EUR', 'JPY', 'KRW', 'IDR', 'VND', 'KWD', 'BHD', 'CNY', 'usd', 'XYZ', '']) {
  add(`currency_fraction_digits/${code || 'empty'}`, 'currency_fraction_digits', { code });
}

// --- core asks 1 (the native lanes' requests) ------------------------------------------
// The relay's sentence, read once for every shell — a spent nonce among it (send-15).
for (const [label, message] of [
  ['relayer_unavailable', 'The gas relayer is unavailable right now.'],
  ['underfunded', 'dedicated bundler gas account is short. Deposit to: 0x1111111111111111111111111111111111111111 required: 1'],
  ['aa25', 'AA25 invalid account nonce'],
  ['nonce_mismatch', 'Transaction nonce mismatch. Please try again.'],
  ['nonce_too_low', 'nonce too low'],
  ['aa250_is_not_aa25', 'AA250 something'],
  ['other', 'Transaction reverted during simulation. Check recipient address and amount.'],
]) add(`classify_relay_rejection/${label}`, 'classify_relay_rejection', { message });
// Plain http only to this computer, anchored on the whole host.
for (const [label, url] of [
  ['localhost_port_path', 'http://localhost:3000/api'],
  ['loopback_v4', 'http://127.0.0.1:8080'],
  ['loopback_v6', 'http://[::1]:8545/'],
  ['https_localhost', 'https://localhost'],
  ['suffix_attack', 'http://127.0.0.1.evil.com'],
  ['bad_port', 'http://localhost:abc'],
]) add(`is_localhost_http/${label}`, 'is_localhost_http', { url });
// A passkey prompt's end: a cancel at the deadline is a timeout (send-11).
for (const [label, kind, elapsed] of [
  ['cancelled_early', 'cancelled', 3000],
  ['cancelled_at_deadline', 'cancelled', 118000],
  ['cancelled_just_before', 'cancelled', 117999],
  ['failure_late', 'other', 130000],
  ['not_supported', 'not_supported', 0],
  ['unknown_kind', 'shrug', 130000],
]) add(`passkey_ceremony_end/${label}`, 'passkey_ceremony_end', { kind, elapsed_ms: elapsed });
add('passkey_sign_timeout_ms', 'passkey_sign_timeout_ms', {});
// A passkey failure, said in the person's language (round 10, real-device S3):
// every case of the core's own corpus, through the shipped wasm.
for (const c of JSON.parse(readFileSync(join(RUST, 'crates/vela-core/tests/vectors/rules/passkey-errors.json'), 'utf8')).cases) {
  add(`passkey_error_words/${c.name}`, 'passkey_error_words', {
    platform: c.platform,
    domain: c.domain,
    code: c.code,
    message: c.message,
  });
}
// Where a chain's reads stand in the pool's view (own node, fallback).
const poolView = {
  failed_chains: [1],
  rate_limited_chains: [],
  banned: [],
  unreached_chains: [],
  fallback_chains: [100],
  own_only_chains: [1, 10],
  own_node_unreached_chains: [1],
  pending_urls: {},
};
for (const [label, chain] of [['own_node_unreached', 1], ['own_only', 10], ['falling_back', 100], ['pooled', 8453]]) {
  add(`rpc_node_standing/${label}`, 'rpc_node_standing', { view_json: S(poolView), chain_id: chain });
}
add('rpc_node_standing/unreadable', 'rpc_node_standing', { view_json: '{', chain_id: 1 });
// A simulation's outcome, "Check again" with it (dapp-15).
const simUser = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
for (const [label, reply] of [
  ['unreachable', { unreachable: true }],
  ['not_offered', { error: { code: -32601, message: 'the method eth_simulateV1 does not exist' } }],
  ['nothing_moves', { result: [{ calls: [{ status: '0x1', logs: [] }] }] }],
]) add(`sim_outcome/${label}`, 'sim_outcome', { user: simUser, reply_json: S(reply) });
// A built-in network's name, without a core instance (the Android lane's ask).
for (const chain of [1, 100, 8453, 5042002, 999999]) {
  add(`builtin_network_display_name/${chain}`, 'builtin_network_display_name', { chain_id: chain });
}
add('builtin_network_ids', 'builtin_network_ids', {});

// --- run through the shipped wasm ------------------------------------------------------
const out = cases.map((c) => ({ ...c, expect: { value: D[c.fn](c.input) } }));
const accepted = out.filter((c) => c.name.endsWith('/accepted'));
for (const c of accepted) if (!c.expect.value.accepted) throw new Error(`${c.name} did not accept: ${S(c.expect.value)}`);
const doc = {
  suite: 'native-parity',
  note:
    'F0.2: the web exports the native apps need over uniffi. Generated by rust/scripts/gen-native-parity.mjs — ' +
    'do not edit by hand. Inputs are as the uniffi export takes them ' +
    '(`*_json` keys are JSON TEXT); every expectation is what the SHIPPED wasm answered (rust/scripts/native-parity-wasm.mjs ' +
    'reduces it: JSON text parsed, "" kept, undefined as null, a refusal as {refused: code}, an accepted assertion as hex). ' +
    'verify-web.mjs replays it through the wasm; vela-core-uniffi tests, smoke-kotlin.sh and smoke-swift.sh through uniffi. ' +
    'The P-256 keys behind the trusted-signer cases were generated once for this file and thrown away.',
  cases: out,
};
mkdirSync(join(RUST, 'crates/vela-core/tests/vectors/native-parity'), { recursive: true });
writeFileSync(join(RUST, 'crates/vela-core/tests/vectors/native-parity/exports.json'), JSON.stringify(doc, null, 1) + '\n');
const exports = new Set(out.map((c) => c.fn));
console.log(`gen-native-parity: wrote ${out.length} cases over ${exports.size} exports`);
