// Unit tests for the shared protocol helpers. Run: `node --test test/`
// (Node 22 built-in test runner — no jest/ts-jest friction for ESM extension JS.)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  classifyMethod,
  isSigningMethod,
  INSTANT_READONLY_METHODS,
  BUNDLER_METHODS,
  toHexChainId,
  parseChainId,
  isAddressLike,
  pickSignAddress,
  rpcTargetFor,
  hostLabel,
  ERR,
  signLaunchUrl,
  universalLinkSelfTestUrl,
  UL_SELFTEST_RID,
} from '../src/lib/protocol.js';

test('isSigningMethod mirrors the app predicate', () => {
  for (const m of ['eth_sendTransaction', 'wallet_sendCalls', 'personal_sign', 'eth_sign', 'eth_signTypedData', 'eth_signTypedData_v4']) {
    assert.equal(isSigningMethod(m), true, m);
  }
  for (const m of ['eth_call', 'eth_chainId', 'eth_getBalance', 'eth_accounts']) {
    assert.equal(isSigningMethod(m), false, m);
  }
});

test('signLaunchUrl: scheme unless attested (ulVerified); rid encoded', () => {
  const rid = 'a b/1?x'; // force encoding
  const enc = encodeURIComponent(rid);
  // UNVERIFIED (undefined / false) MUST be the R1-proven scheme — a failed UL would
  // navigate the dApp tab away and lose the pending sign (fund-safety regression).
  assert.equal(signLaunchUrl(rid), `velawallet://sign?rid=${enc}`);
  assert.equal(signLaunchUrl(rid, false), `velawallet://sign?rid=${enc}`);
  // ATTESTED (the app confirmed the association resolves on this device) → the UL.
  assert.equal(signLaunchUrl(rid, true), `https://getvela.app/sign?rid=${enc}`);
  assert.ok(enc.includes('%20') && !signLaunchUrl(rid).includes(' '), 'rid is URL-encoded');
});

test('universalLinkSelfTestUrl targets the /sign path the applinks AASA matches', () => {
  assert.equal(universalLinkSelfTestUrl(), `https://getvela.app/sign?rid=${UL_SELFTEST_RID}`);
});

test('classifyMethod buckets correctly (incl. eth_sign refusal)', () => {
  assert.equal(classifyMethod('eth_sign'), 'unsupported'); // §12.4 security
  assert.equal(classifyMethod('personal_sign'), 'sign');
  assert.equal(classifyMethod('eth_signTypedData_v4'), 'sign');
  assert.equal(classifyMethod('eth_sendTransaction'), 'sign');
  assert.equal(classifyMethod('wallet_sendCalls'), 'sign');
  assert.equal(classifyMethod('eth_requestAccounts'), 'connect');
  assert.equal(classifyMethod('wallet_requestPermissions'), 'connect');
  assert.equal(classifyMethod('eth_accounts'), 'state');
  assert.equal(classifyMethod('eth_chainId'), 'state');
  assert.equal(classifyMethod('net_version'), 'state');
  assert.equal(classifyMethod('wallet_getPermissions'), 'state');
  assert.equal(classifyMethod('wallet_switchEthereumChain'), 'switch');
  assert.equal(classifyMethod('wallet_addEthereumChain'), 'addChain');
  assert.equal(classifyMethod('wallet_watchAsset'), 'addChain');
  // allowlisted reads → proxied
  assert.equal(classifyMethod('eth_call'), 'read');
  assert.equal(classifyMethod('eth_getBalance'), 'read');
  assert.equal(classifyMethod('eth_getTransactionReceipt'), 'read');
  assert.equal(classifyMethod('eth_sendUserOperation'), 'read'); // bundler read
  // NOT allowlisted → refused, never proxied to a public RPC (no fail-open)
  assert.equal(classifyMethod('eth_signTransaction'), 'unsupported');
  assert.equal(classifyMethod('debug_traceTransaction'), 'unsupported');
  assert.equal(classifyMethod('some_random_method'), 'unsupported');
  assert.equal(classifyMethod('eth_getBalanceOf'), 'unsupported'); // near-miss typo
});

test('the app allowlists are represented verbatim', () => {
  for (const m of ['eth_accounts', 'eth_requestAccounts', 'eth_chainId', 'net_version', 'wallet_getPermissions', 'wallet_requestPermissions', 'wallet_addEthereumChain']) {
    assert.equal(INSTANT_READONLY_METHODS.has(m), true, m);
  }
  for (const m of ['eth_sendUserOperation', 'eth_estimateUserOperationGas', 'eth_getUserOperationReceipt', 'eth_getUserOperationByHash', 'pimlico_getUserOperationGasPrice']) {
    assert.equal(BUNDLER_METHODS.has(m), true, m);
  }
});

test('toHexChainId is minimal lowercase hex', () => {
  assert.equal(toHexChainId(1), '0x1');
  assert.equal(toHexChainId(56), '0x38');
  assert.equal(toHexChainId(8453), '0x2105');
  assert.equal(toHexChainId('0x38'), '0x38');
  assert.equal(toHexChainId('137'), '0x89');
  assert.equal(toHexChainId(0), '0x1'); // guard
  assert.equal(toHexChainId(NaN), '0x1');
});

test('parseChainId accepts number | hex | decimal string', () => {
  assert.equal(parseChainId(42161), 42161);
  assert.equal(parseChainId('0xa4b1'), 42161);
  assert.equal(parseChainId('42161'), 42161);
  assert.equal(parseChainId('nope'), 0);
  assert.equal(parseChainId(undefined), 0);
});

test('isAddressLike is shape-based', () => {
  assert.equal(isAddressLike('0x' + 'a'.repeat(40)), true);
  assert.equal(isAddressLike('0x' + 'A'.repeat(40)), true);
  assert.equal(isAddressLike('0x' + 'a'.repeat(39)), false);
  assert.equal(isAddressLike('hello'), false);
  assert.equal(isAddressLike(42), false);
});

test('pickSignAddress finds the address by shape, not position', () => {
  const A = '0x' + '1'.repeat(40);
  // personal_sign = [message, address]
  assert.equal(pickSignAddress('personal_sign', ['0xdeadbeef', A]), A.toLowerCase());
  // typed data = [address, typedData] — address FIRST (opposite order)
  assert.equal(pickSignAddress('eth_signTypedData_v4', [A, '{"types":{}}']), A.toLowerCase());
  assert.equal(pickSignAddress('personal_sign', ['0xdeadbeef']), null);
});

test('rpcTargetFor routes bundler methods to the bundler URL', () => {
  const chain = { rpcUrl: 'https://node.example', bundlerUrl: 'https://bundler.example/1' };
  assert.deepEqual(rpcTargetFor('eth_call', chain), { url: 'https://node.example', kind: 'node' });
  assert.deepEqual(rpcTargetFor('eth_sendUserOperation', chain), { url: 'https://bundler.example/1', kind: 'bundler' });
  assert.equal(rpcTargetFor('eth_call', null), null);
  assert.equal(rpcTargetFor('eth_sendUserOperation', { rpcUrl: 'x' }), null); // no bundlerUrl
});

test('hostLabel extracts a friendly host', () => {
  assert.equal(hostLabel('https://biubiu.tools'), 'biubiu.tools');
  assert.equal(hostLabel('https://app.uniswap.org:443'), 'app.uniswap.org');
});

test('error codes match the design contract', () => {
  assert.equal(ERR.USER_REJECTED, 4001);
  assert.equal(ERR.UNAUTHORIZED, 4100);
  assert.equal(ERR.UNSUPPORTED_METHOD, 4200);
  assert.equal(ERR.UNKNOWN_PENDING, 4900); // §12.1.3 — never 4001 on timeout
  assert.equal(ERR.CHAIN_NOT_ADDED, 4902);
});
