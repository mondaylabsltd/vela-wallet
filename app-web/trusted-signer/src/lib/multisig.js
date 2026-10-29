// A multisig member's approval (`vela_multisigApprove`): what a member of a
// Vela multisig (a Safe v1.4.1) signs to approve one of its transactions.
//
// The app hands this page the raw SafeTx, the multisig's address and the
// chain, and says which hash it thinks they make. This page trusts none of it
// as a conclusion: it recomputes the safeTxHash from the fields itself
// (EIP-712, the Safe's own domain), refuses when the app's hash differs, reads
// what the transaction does from its bytes, and signs only the approval bytes
// it derived — never a hash the app supplied.
//
// Transcribed from vela-core (`rust/crates/vela-core/src/multisig.rs`:
// `safe_tx_hashes`, `approval_challenge`, `review`) and cross-checked against
// its compiled wasm in samples/multisig-test.mjs, case by case: two
// independent readings agreeing is the only reason to believe either. The
// rules are the core's, on purpose — the page must never offer to sign what
// Vela itself refuses, nor read a transaction differently from it.
//
// What the member's passkey signs (`approvalChallenge`): a v1.4.1 Safe asks
// each owner `isValidSignature(txHashData, sig)` (EIP-1271, legacy form); a
// Vela account answers through its CompatibilityFallbackHandler, which checks
// `SafeMessage(keccak256(txHashData))` — i.e. `SafeMessage(safeTxHash)` —
// under the MEMBER's own domain. Not lib/safeop.js's `messageHash`, whose
// extra keccak is the dApp path: a dApp signature can never pass for an
// approval, and an approval can never pass for a dApp signature.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var keccak = ns.keccak;

  /** Safe v1.4.1 `MultiSendCallOnly`: the one delegatecall target Vela signs. */
  var MULTI_SEND_CALL_ONLY = '0x9641d764fc13c8B624c04430C7356C1C7C8102e2';
  /** The Safe Genesis Registry (v4): where a multisig renames its record. */
  var GENESIS_REGISTRY = '0xF412388bF908E510066d3Be5c704bd9A327f945d';
  var ZERO = '0x0000000000000000000000000000000000000000';

  var SAFE_TX_TYPE = 'SafeTx(address to,uint256 value,bytes data,uint8 operation,' +
    'uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,' +
    'address refundReceiver,uint256 nonce)';
  var DOMAIN_TYPE = 'EIP712Domain(uint256 chainId,address verifyingContract)';
  var SAFE_MESSAGE_TYPE = 'SafeMessage(bytes message)';

  /**
   * The most calldata this page reads, in bytes. Every byte of a SafeTx is
   * drawn (the details hold its calldata in full), and a page that froze on a
   * crafted megabyte would be a page nobody can refuse on. A real batch of
   * sixty transfers is about ten kilobytes.
   */
  var MAX_DATA_BYTES = 64 * 1024;
  /** The most legs of a batch this page draws. */
  var MAX_LEGS = 64;
  /** The registry's `MAX_NAME_LENGTH`, in bytes (vela-core `MAX_NAME_BYTES`). */
  var MAX_NAME_BYTES = 64;
  var MAX_UINT256 = (1n << 256n) - 1n;

  var FIELDS = ['to', 'value', 'data', 'operation', 'safeTxGas', 'baseGas', 'gasPrice',
    'gasToken', 'refundReceiver', 'nonce'];

  // --- bytes ----------------------------------------------------------------

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
    var body = String(hex).replace(/^0x/i, '');
    var out = new Uint8Array(body.length / 2);
    for (var i = 0; i < out.length; i++) out[i] = parseInt(body.substr(i * 2, 2), 16);
    return out;
  }

  function toHex(bytes) {
    var s = '0x';
    for (var i = 0; i < bytes.length; i++) s += bytes[i].toString(16).padStart(2, '0');
    return s;
  }

  function word(value) {
    return fromHex(BigInt(value).toString(16).padStart(64, '0'));
  }

  function addressWord(address) {
    return fromHex(address.slice(2).toLowerCase().padStart(64, '0'));
  }

  function wordAt(bytes, at) {
    if (at + 32 > bytes.length) return null;
    var hex = '';
    for (var i = at; i < at + 32; i++) hex += bytes[i].toString(16).padStart(2, '0');
    return BigInt('0x' + hex);
  }

  function addressOf(bytes, at) {
    return checksum(toHex(bytes.subarray(at, at + 20)));
  }

  /** EIP-55 casing: how the core (and every block explorer) writes an address. */
  function checksum(address) {
    var body = String(address).replace(/^0x/i, '').toLowerCase();
    var hash = keccak.hex(utf8(body)).slice(2);
    var out = '0x';
    for (var i = 0; i < body.length; i++) {
      out += parseInt(hash[i], 16) >= 8 ? body[i].toUpperCase() : body[i];
    }
    return out;
  }

  function same(a, b) {
    return String(a).toLowerCase() === String(b).toLowerCase();
  }

  function selector(signature) {
    return keccak.selector(signature);
  }

  function selectorOf(bytes) {
    return bytes.length >= 4 ? toHex(bytes.subarray(0, 4)) : null;
  }

  // --- the SafeTx as the app sent it ---------------------------------------

  var ADDRESS = /^0x[0-9a-fA-F]{40}$/;
  var HEX_BYTES = /^0x(?:[0-9a-fA-F]{2})*$/;

  /**
   * A quantity as the core reads one (`multisig::quantity`): `0x` hex (`0x`
   * alone is zero) or decimal digits, surrounding spaces ignored, at most
   * 2^256 − 1. Anything else — a number, a sign, a fraction — is not one.
   */
  function quantity(value) {
    if (typeof value !== 'string') return null;
    var text = value.trim();
    var parsed;
    if (/^0x[0-9a-fA-F]*$/.test(text)) parsed = text.length === 2 ? 0n : BigInt(text);
    else if (/^[0-9]+$/.test(text)) parsed = BigInt(text);
    else return null;
    return parsed <= MAX_UINT256 ? parsed : null;
  }

  /**
   * The SafeTx fields as values this page can hash, or why not (an i18n key).
   * Strict on purpose: a field this page reads differently from the core
   * would make a hash that differs from the one approved.
   */
  function parseTx(raw) {
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return { error: 'refuse.msShape' };
    for (var i = 0; i < FIELDS.length; i++) {
      if (raw[FIELDS[i]] === undefined || raw[FIELDS[i]] === null) return { error: 'refuse.msShape' };
    }
    if (!ADDRESS.test(raw.to) || !ADDRESS.test(raw.gasToken) || !ADDRESS.test(raw.refundReceiver)) {
      return { error: 'refuse.msShape' };
    }
    if (typeof raw.data !== 'string' || !HEX_BYTES.test(raw.data)) return { error: 'refuse.msShape' };
    if ((raw.data.length - 2) / 2 > MAX_DATA_BYTES) return { error: 'refuse.msTooLarge' };
    var operation = raw.operation;
    if (typeof operation !== 'number' || !Number.isInteger(operation) || operation < 0 || operation > 255) {
      return { error: 'refuse.msShape' };
    }
    var tx = {
      to: checksum(raw.to),
      data: '0x' + raw.data.slice(2).toLowerCase(),
      operation: operation,
      gasToken: checksum(raw.gasToken),
      refundReceiver: checksum(raw.refundReceiver),
    };
    var quantities = ['value', 'safeTxGas', 'baseGas', 'gasPrice', 'nonce'];
    for (var q = 0; q < quantities.length; q++) {
      var parsed = quantity(raw[quantities[q]]);
      if (parsed === null) return { error: 'refuse.msShape' };
      tx[quantities[q]] = parsed;
    }
    return { tx: tx };
  }

  // --- hashes ---------------------------------------------------------------

  function domainSeparator(chainId, verifyingContract) {
    return keccak.hash(concat([
      keccak.hash(utf8(DOMAIN_TYPE)),
      word(chainId),
      addressWord(verifyingContract),
    ]));
  }

  function eip712(domain, struct) {
    return keccak.hash(concat([new Uint8Array([0x19, 0x01]), domain, struct]));
  }

  function structHash(tx) {
    return keccak.hash(concat([
      keccak.hash(utf8(SAFE_TX_TYPE)),
      addressWord(tx.to),
      word(tx.value),
      keccak.hash(fromHex(tx.data)),
      word(tx.operation),
      word(tx.safeTxGas),
      word(tx.baseGas),
      word(tx.gasPrice),
      addressWord(tx.gasToken),
      addressWord(tx.refundReceiver),
      word(tx.nonce),
    ]));
  }

  /**
   * The three hashes a hardware wallet shows for a SafeTx (vela-core
   * `safe_tx_hashes`): the domain hash (`EIP712Domain{chainId,
   * verifyingContract: safe}`), the message hash (the SafeTx struct hash) and
   * the safeTxHash they make. Hex, `0x`, lowercase.
   */
  function hashes(chainId, safe, tx) {
    var domain = domainSeparator(chainId, safe);
    var message = structHash(tx);
    return {
      domainHash: toHex(domain),
      messageHash: toHex(message),
      safeTxHash: toHex(eip712(domain, message)),
    };
  }

  /**
   * What the member's passkey signs to approve `safeTxHash` (32 bytes, hex)
   * on `chainId` (vela-core `approval_challenge`): `SafeMessage(safeTxHash)`
   * under the member's own domain. The bytes, for the WebAuthn challenge.
   */
  function approvalChallenge(chainId, member, safeTxHash) {
    var inner = keccak.hash(concat([keccak.hash(utf8(SAFE_MESSAGE_TYPE)), fromHex(safeTxHash)]));
    return eip712(domainSeparator(chainId, member), inner);
  }

  // --- what it does (vela-core `review`) -----------------------------------

  var OWNER_MANAGEMENT = {};
  OWNER_MANAGEMENT[selector('addOwnerWithThreshold(address,uint256)')] = 'addOwner';
  OWNER_MANAGEMENT[selector('removeOwner(address,address,uint256)')] = 'removeOwner';
  OWNER_MANAGEMENT[selector('swapOwner(address,address,address)')] = 'swapOwner';
  OWNER_MANAGEMENT[selector('changeThreshold(uint256)')] = 'changeThreshold';

  /**
   * The Safe's own functions that hand it to something other than its
   * members — named on the card when a SafeTx calls one on the multisig.
   * Refused like every self-call that is not a member change (the core reads
   * those as "not a transaction Vela signs"); the name only says which.
   */
  var TAKEOVER = {};
  [
    'enableModule(address)', 'disableModule(address,address)', 'setGuard(address)',
    'setModuleGuard(address)', 'setFallbackHandler(address)',
    'setup(address[],uint256,address,bytes,address,address,uint256,address)',
    'execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)',
    'execTransactionFromModule(address,uint256,bytes,uint8)',
    'execTransactionFromModuleReturnData(address,uint256,bytes,uint8)',
    'approveHash(bytes32)',
    'simulateAndRevert(address,bytes)',
  ].forEach(function (signature) { TAKEOVER[selector(signature)] = signature.split('(')[0]; });
  // A singleton change is a storage write a delegatecall makes; the multisig
  // has no function for it. Said by name so the card can say what it is.
  TAKEOVER[selector('changeMasterCopy(address)')] = 'changeMasterCopy';

  var TRANSFER = selector('transfer(address,uint256)');
  var MULTI_SEND = selector('multiSend(bytes)');
  var RENAME = selector('rename(string)');

  /** vela-core `read_owner_management`: one of the four, exactly encoded. */
  function readOwnerManagement(data) {
    var kind = OWNER_MANAGEMENT[selectorOf(data)];
    if (!kind) return null;
    var w = function (i) { return wordAt(data, 4 + i * 32); };
    var a = function (i) {
      var v = w(i);
      if (v === null) return null;
      // `Address::from_word`: the low 20 bytes, whatever the high 12 hold.
      return checksum('0x' + (v & ((1n << 160n) - 1n)).toString(16).padStart(40, '0'));
    };
    if (kind === 'addOwner' && data.length === 68) {
      return { kind: 'addOwner', owner: a(0), threshold: w(1).toString() };
    }
    if (kind === 'removeOwner' && data.length === 100) {
      return { kind: 'removeOwner', owner: a(1), threshold: w(2).toString() };
    }
    if (kind === 'swapOwner' && data.length === 100) {
      return { kind: 'swapOwner', oldOwner: a(1), newOwner: a(2) };
    }
    if (kind === 'changeThreshold' && data.length === 36) {
      return { kind: 'changeThreshold', threshold: w(0).toString() };
    }
    return null;
  }

  /** vela-core `read_transfer`: native with no calldata, or an exact ERC-20 `transfer`. */
  function readTransfer(to, value, data) {
    if (!data.length) return { kind: 'transfer', token: null, to: to, amount: value.toString() };
    if (value === 0n && data.length === 68 && selectorOf(data) === TRANSFER) {
      for (var i = 4; i < 16; i++) if (data[i] !== 0) return null;
      return {
        kind: 'transfer',
        token: to,
        to: addressOf(data, 16),
        amount: wordAt(data, 36).toString(),
      };
    }
    return null;
  }

  /** Characters that draw nothing or reorder what is drawn (vela-core `hidden`). */
  function hidden(code) {
    return code === 0xad || code === 0x61c || code === 0x180e ||
      (code >= 0x200b && code <= 0x200f) || (code >= 0x202a && code <= 0x202e) ||
      (code >= 0x2060 && code <= 0x2064) || (code >= 0x2066 && code <= 0x206f) ||
      code === 0xfeff || (code >= 0xfff9 && code <= 0xfffb);
  }

  function pictograph(code) {
    return (code >= 0x2600 && code <= 0x27bf) || (code >= 0x2b00 && code <= 0x2bff) ||
      (code >= 0x1f000 && code <= 0x1faff);
  }

  /**
   * vela-core `valid_name`: a name every member reads in full — no space
   * around it, at most 64 bytes, no control character, nothing invisible
   * (a zero-width joiner only inside an emoji).
   */
  function validName(name) {
    if (typeof name !== 'string' || !name || name.trim() !== name) return false;
    if (utf8(name).length > MAX_NAME_BYTES) return false;
    var chars = Array.from(name).map(function (c) { return c.codePointAt(0); });
    return chars.every(function (c, i) {
      if (c <= 0x1f || (c >= 0x7f && c <= 0x9f) || c === 0xfffd) return false;
      if (!hidden(c)) return true;
      return c === 0x200d && i > 0 && (pictograph(chars[i - 1]) || chars[i - 1] === 0xfe0f) &&
        i + 1 < chars.length && pictograph(chars[i + 1]);
    });
  }

  /** vela-core `read_rename`: exactly one canonical `rename(string)` of text. */
  function readRename(data) {
    if (selectorOf(data) !== RENAME || wordAt(data, 4) !== 0x20n) return null;
    var len = wordAt(data, 36);
    if (len === null || len > BigInt(data.length)) return null;
    var n = Number(len);
    var padded = n + ((32 - (n % 32)) % 32);
    if (data.length !== 68 + padded) return null;
    for (var i = 68 + n; i < data.length; i++) if (data[i] !== 0) return null;
    try {
      return new TextDecoder('utf-8', { fatal: true }).decode(data.subarray(68, 68 + n));
    } catch (e) {
      return null;
    }
  }

  function readRegistryCall(to, value, data) {
    if (value !== 0n) return { refusal: 'refuse.msRegistryCoins' };
    if (selectorOf(data) === RENAME) {
      var name = readRename(data);
      return name !== null && validName(name)
        ? { action: { kind: 'rename', name: name } }
        : { refusal: 'refuse.msRenameUnreadable' };
    }
    return { action: { kind: 'call', to: to, value: value.toString(), data: toHex(data) } };
  }

  function selfCallRefusal(data) {
    var fn = TAKEOVER[selectorOf(data)];
    return fn ? { refusal: 'refuse.msSelfCall', params: { fn: fn } } : { refusal: 'refuse.msNotSigned' };
  }

  /** vela-core `read_batch`: the calls of a canonical MultiSendCallOnly batch, every one a CALL. */
  function readBatch(data, me) {
    var unreadable = { refusal: 'refuse.msBatchUnreadable' };
    if (selectorOf(data) !== MULTI_SEND || wordAt(data, 4) !== 0x20n) return unreadable;
    var len = wordAt(data, 36);
    if (len === null || len > BigInt(data.length)) return unreadable;
    var n = Number(len);
    if (68 + n > data.length) return unreadable;
    // Canonical encoding only: nothing may hide in padding or trailing bytes.
    if (data.length !== 68 + n + ((32 - (n % 32)) % 32)) return unreadable;
    for (var z = 68 + n; z < data.length; z++) if (data[z] !== 0) return unreadable;
    var packed = data.subarray(68, 68 + n);
    var calls = [];
    var i = 0;
    while (i < packed.length) {
      if (i + 85 > packed.length) return unreadable;
      // A delegatecall leg runs another contract's code AS the multisig.
      if (packed[i] !== 0) return { refusal: 'refuse.msBatchDelegate' };
      var to = addressOf(packed, i + 1);
      var value = wordAt(packed, i + 21);
      var size = wordAt(packed, i + 53);
      if (size > BigInt(packed.length)) return unreadable;
      var end = i + 85 + Number(size);
      if (end > packed.length) return unreadable;
      var legData = packed.subarray(i + 85, end);
      // MultiSend reads the zero address as the multisig itself: a call that
      // hides where it goes.
      if (same(to, ZERO)) return { refusal: 'refuse.msBatchZero' };
      var action;
      if (same(to, me)) {
        // Into the multisig itself: only the four member changes, no coin.
        action = value === 0n ? readOwnerManagement(legData) : null;
        if (!action) return selfCallRefusal(legData);
      } else if (same(to, GENESIS_REGISTRY)) {
        var registry = readRegistryCall(to, value, legData);
        if (registry.refusal) return registry;
        action = registry.action;
      } else {
        action = readTransfer(to, value, legData) ||
          { kind: 'call', to: to, value: value.toString(), data: toHex(legData) };
      }
      calls.push({ to: to, value: value.toString(), data: toHex(legData), action: action });
      if (calls.length > MAX_LEGS) return { refusal: 'refuse.msTooLarge' };
      i = end;
    }
    if (!calls.length) return unreadable;
    return { action: { kind: 'batch', calls: calls } };
  }

  /**
   * What `tx` does for `multisig`, read the core's way: `{action}` — one of
   * transfer, batch, cancel, addOwner, removeOwner, swapOwner,
   * changeThreshold, rename, call, in the core's own field names — or
   * `{refusal, params}` (an i18n key) for what Vela never signs.
   */
  function read(multisig, tx) {
    var data = fromHex(tx.data);
    if (tx.safeTxGas !== 0n || tx.baseGas !== 0n || tx.gasPrice !== 0n ||
      !same(tx.gasToken, ZERO) || !same(tx.refundReceiver, ZERO)) {
      return { refusal: 'refuse.msGasRefund' };
    }
    if (tx.operation === 1) {
      if (same(tx.to, MULTI_SEND_CALL_ONLY) && tx.value === 0n) return readBatch(data, multisig);
      return { refusal: 'refuse.msDelegateCall' };
    }
    if (tx.operation !== 0) return { refusal: 'refuse.msUnknownOperation' };
    if (same(tx.to, multisig)) {
      if (tx.value === 0n && !data.length) return { action: { kind: 'cancel' } };
      var members = tx.value === 0n ? readOwnerManagement(data) : null;
      return members ? { action: members } : selfCallRefusal(data);
    }
    if (same(tx.to, GENESIS_REGISTRY)) return readRegistryCall(tx.to, tx.value, data);
    return {
      action: readTransfer(tx.to, tx.value, data) ||
        { kind: 'call', to: tx.to, value: tx.value.toString(), data: toHex(data) },
    };
  }

  /**
   * The request's one parameter, read: `{chainId, safe, member, tx, claimed}`
   * — or `{error}` (an i18n key). `claimed` is the safeTxHash the app says the
   * fields make; nothing is derived from it, it is only compared.
   */
  function request(intent) {
    var params = intent && Array.isArray(intent.params) ? intent.params : null;
    var p = params && params.length === 1 ? params[0] : null;
    if (!p || typeof p !== 'object' || Array.isArray(p)) return { error: 'refuse.msShape' };
    var chainId = typeof p.chainId === 'number' && Number.isSafeInteger(p.chainId) && p.chainId > 0
      ? p.chainId : null;
    if (!chainId || !ADDRESS.test(p.safe) || !ADDRESS.test(p.member)) return { error: 'refuse.msShape' };
    if (typeof p.safeTxHash !== 'string' || !/^0x[0-9a-fA-F]{64}$/.test(p.safeTxHash)) {
      return { error: 'refuse.msShape' };
    }
    var parsed = parseTx(p.safeTx);
    if (parsed.error) return { error: parsed.error };
    return {
      chainId: chainId,
      safe: checksum(p.safe),
      member: checksum(p.member),
      tx: parsed.tx,
      claimed: p.safeTxHash.toLowerCase(),
    };
  }

  ns.multisig = {
    METHOD: 'vela_multisigApprove',
    MULTI_SEND_CALL_ONLY: MULTI_SEND_CALL_ONLY,
    GENESIS_REGISTRY: GENESIS_REGISTRY,
    MAX_DATA_BYTES: MAX_DATA_BYTES,
    MAX_LEGS: MAX_LEGS,
    request: request,
    parseTx: parseTx,
    hashes: hashes,
    approvalChallenge: approvalChallenge,
    read: read,
    validName: validName,
    checksum: checksum,
    toHex: toHex,
  };
})(window.VelaCS);
