// The Safe 4337 digests — the bytes a transaction signature actually covers.
//
// Transcribed from vela-core (`rust/crates/vela-core/src/user_op.rs`) and
// cross-checked against its compiled wasm in samples/safeop-test.mjs. Two
// independent implementations agreeing on a digest is the only reason to
// believe either of them.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var keccak = ns.keccak;

  // Canonical deployment. A request that names anything else is not a Vela
  // account, and the sheet says so rather than silently signing for it.
  var ENTRY_POINT = '0x0000000071727De22E5E9d8BAf0edAc6f37da032';
  var SAFE_4337_MODULE = '0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226';
  var MULTI_SEND = '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526';

  var SAFE_OP_TYPE = 'SafeOp(address safe,uint256 nonce,bytes initCode,bytes callData,' +
    'uint128 verificationGasLimit,uint128 callGasLimit,uint256 preVerificationGas,' +
    'uint128 maxPriorityFeePerGas,uint128 maxFeePerGas,bytes paymasterAndData,' +
    'uint48 validAfter,uint48 validUntil,address entryPoint)';
  var DOMAIN_TYPE = 'EIP712Domain(uint256 chainId,address verifyingContract)';
  var SAFE_MESSAGE_TYPE = 'SafeMessage(bytes message)';

  var EXECUTE_USER_OP = 'executeUserOp(address,uint256,bytes,uint8)';

  function utf8(text) {
    return new TextEncoder().encode(text);
  }

  function concat(parts) {
    var total = parts.reduce(function (n, p) { return n + p.length; }, 0);
    var out = new Uint8Array(total);
    var at = 0;
    parts.forEach(function (p) { out.set(p, at); at += p.length; });
    return out;
  }

  function fromHex(hex) {
    var body = String(hex || '').replace(/^0x/i, '');
    if (body.length % 2) body = '0' + body;
    var out = new Uint8Array(body.length / 2);
    for (var i = 0; i < out.length; i++) out[i] = parseInt(body.substr(i * 2, 2), 16);
    return out;
  }

  function word(value) {
    var v = typeof value === 'bigint' ? value : BigInt(value);
    return fromHex(v.toString(16).padStart(64, '0'));
  }

  function addressWord(address) {
    return fromHex(String(address).replace(/^0x/i, '').toLowerCase().padStart(64, '0'));
  }

  function domainSeparator(chainId, verifyingContract) {
    return keccak.hash(concat([
      keccak.hash(utf8(DOMAIN_TYPE)),
      word(chainId),
      addressWord(verifyingContract),
    ]));
  }

  function eip712(separator, structHash) {
    return keccak.hash(concat([new Uint8Array([0x19, 0x01]), separator, structHash]));
  }

  /**
   * The SafeOp digest. `validAfter` and `validUntil` are always zero, matching
   * the core — a request that wants a time window would need a protocol change,
   * not a quiet extra field here.
   */
  function safeOpHash(op, chainId, options) {
    options = options || {};
    var entryPoint = options.entryPoint || ENTRY_POINT;
    var module = options.module || SAFE_4337_MODULE;

    var structHash = keccak.hash(concat([
      keccak.hash(utf8(SAFE_OP_TYPE)),
      addressWord(op.sender),
      word(BigInt(op.nonce)),
      keccak.hash(fromHex(op.initCode || '0x')),
      keccak.hash(fromHex(op.callData || '0x')),
      word(BigInt(op.verificationGasLimit)),
      word(BigInt(op.callGasLimit)),
      word(BigInt(op.preVerificationGas)),
      word(BigInt(op.maxPriorityFeePerGas)),
      word(BigInt(op.maxFeePerGas)),
      keccak.hash(fromHex(op.paymasterAndData || '0x')),
      word(0), // validAfter
      word(0), // validUntil
      addressWord(entryPoint),
    ]));
    return eip712(domainSeparator(chainId, module), structHash);
  }

  /**
   * What a Safe actually verifies for an off-chain message: the EIP-191 or
   * EIP-712 hash wrapped in SafeMessage(bytes) under the SAFE's own domain.
   * Signing the unwrapped hash produces a signature EIP-1271 rejects.
   */
  function safeMessageHash(originalHash, chainId, safeAddress) {
    // `SafeMessage(bytes message)` — a dynamic `bytes` member is hashed ONCE
    // in EIP-712 struct encoding. Hashing twice looks equally plausible and
    // produces a signature EIP-1271 rejects.
    var messageHash = keccak.hash(originalHash);
    var structHash = keccak.hash(concat([keccak.hash(utf8(SAFE_MESSAGE_TYPE)), messageHash]));
    return eip712(domainSeparator(chainId, safeAddress), structHash);
  }

  // --- reading the calldata back ---------------------------------------------

  /**
   * Decode `executeUserOp(...)` — and, when it delegatecalls MultiSend, the
   * legs inside it. This is what lets the sheet render the bytes the digest
   * covers rather than the bytes the requester says it covers.
   */
  function decodeCallData(callData) {
    var selector = ns.abi.selectorOf(callData);
    if (selector !== ns.keccak.selector(EXECUTE_USER_OP)) return null;
    var values = ns.abi.decode(EXECUTE_USER_OP, callData);
    if (!values) return null;

    var to = values[0];
    var value = values[1];
    var data = values[2];
    var operation = Number(values[3]);

    if (operation === 1 && to.toLowerCase() === MULTI_SEND.toLowerCase()) {
      var inner = ns.abi.decode('multiSend(bytes)', data);
      if (!inner) return null;
      return { kind: 'multiSend', calls: unpackMultiSend(inner[0]) };
    }
    return {
      kind: 'call',
      calls: [{ to: to, value: '0x' + value.toString(16), data: data }],
    };
  }

  // operation(1) ‖ to(20) ‖ value(32) ‖ dataLen(32) ‖ data
  function unpackMultiSend(packed) {
    var body = String(packed).replace(/^0x/i, '');
    var calls = [];
    var i = 0;
    while (i + 170 <= body.length) {
      var length = Number(BigInt('0x' + body.slice(i + 106, i + 170)));
      calls.push({
        to: '0x' + body.slice(i + 2, i + 42),
        value: '0x' + BigInt('0x' + body.slice(i + 42, i + 106)).toString(16),
        data: '0x' + body.slice(i + 170, i + 170 + length * 2),
      });
      i += 170 + length * 2;
    }
    return calls;
  }

  function sameCall(a, b) {
    var address = function (v) { return String(v || '').toLowerCase(); };
    var amount = function (v) { return BigInt(v || 0).toString(); };
    var bytes = function (v) { return String(v || '0x').toLowerCase().replace(/^0x/, ''); };
    return address(a.to) === address(b.to) &&
      amount(a.value) === amount(b.value) &&
      bytes(a.data) === bytes(b.data);
  }

  ns.safeop = {
    ENTRY_POINT: ENTRY_POINT,
    SAFE_4337_MODULE: SAFE_4337_MODULE,
    MULTI_SEND: MULTI_SEND,
    hash: safeOpHash,
    messageHash: safeMessageHash,
    decodeCallData: decodeCallData,
    sameCall: sameCall,
  };
})(window.VelaCS);
