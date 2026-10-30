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

  // --- an executor's operation (round 7, r7-1) --------------------------------
  //
  // Executing a multisig transaction is the executor's OWN user operation
  // (vela-core `execution_calls`): deploy the multisig and every approving
  // member's Vela account that has no code on the chain yet — both
  // permissionless, from their geneses — then `execTransaction` on the
  // multisig. Signed blind, that is the Bybit signature: the SafeTx inside
  // `execTransaction` is what moves the multisig's money, and a deploy's
  // setup decides who controls what it deploys. So every leg is read here
  // from its bytes, or the operation is refused:
  //
  //  · `execTransaction`: the SafeTx decoded (the core's encoding only) and
  //    read by the same rules as a member's approval (`read` — the core's
  //    `review`, refusals included); the approvals it carries counted from
  //    the signature bytes. Its safeTxHash is recomputed by resolve.js at
  //    the nonce the app names: `execTransaction` carries no nonce — the
  //    multisig supplies its own — so the number is the app's word, while
  //    what runs is read from the bytes whatever the number;
  //  · a deploy of the multisig: `createProxyWithNonce` on Safe's v1.4.1
  //    factory, the L2 singleton, and a setup that is EXACTLY the one Vela
  //    creates a multisig with (`multisig_genesis`: the members, the
  //    threshold, the CompatibilityFallbackHandler, no hook, no payment) —
  //    re-encoded here and compared byte for byte — whose CREATE2 address is
  //    the multisig the operation executes on;
  //  · a deploy of a member's Vela account: the same factory and singleton,
  //    a setup that is exactly a Vela account's (`vela_account_genesis`,
  //    re-encoded from the keys it configures), salted as Vela salts it,
  //    deriving the address of a member whose approval the execution carries.
  //
  // Anything else in the operation — another call, a second execution, a
  // deploy this page cannot place — is refused, not drawn. A multisig created
  // elsewhere (a Safe{Wallet} genesis) and deployed by its first execution is
  // refused too: this page cannot vouch for a setup it does not write.

  var SAFE_PROXY_FACTORY = '0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67';
  var SAFE_SINGLETON = '0x29fcB43b46531BcA003ddC8FCB67FFE91900C762';
  var COMPATIBILITY_FALLBACK_HANDLER = '0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99';
  var SAFE_4337_MODULE = '0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226';
  var SAFE_MODULE_SETUP = '0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47';
  var WEBAUTHN_SIGNER = '0x94a4F6affBd8975951142c3999aEAB7ecee555c2';
  var WEBAUTHN_SIGNER_FACTORY = '0x1d31F259eE307358a26dFb23EB365939E8641195';
  var WEBAUTHN_SIGNER_SINGLETON = '0x4E27b51350e6c2083EE19011120F50DAfEc5CA50';
  /** Safe's MultiSend, which a Vela account's setup delegatecalls (vela-core `safe::MULTI_SEND`). */
  var VELA_MULTI_SEND = '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526';
  /** The RIP-7212 precompile a Vela account's passkey verifier uses (`WEBAUTHN_VERIFIERS_HEX`). */
  var WEBAUTHN_VERIFIERS = 0x100n;
  /** vela-core `safe::MAX_MULTI_KEYS`. */
  var MAX_ACCOUNT_KEYS = 7;
  /** vela-core `multisig::MAX_OWNERS`. */
  var MAX_OWNERS = 20;
  var SENTINEL = '0x0000000000000000000000000000000000000001';

  /** Safe v1.4.1's `proxyCreationCode()` (vela-core `safe::PROXY_CREATION_CODE`). */
  var PROXY_CREATION_CODE = '608060405234801561001057600080fd5b506040516101e63803806101e68339818101604052602081101561003357600080fd5b8101908080519060200190929190505050600073ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff1614156100ca576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260228152602001806101c46022913960400191505060405180910390fd5b806000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505060ab806101196000396000f3fe608060405273ffffffffffffffffffffffffffffffffffffffff600054167fa619486e0000000000000000000000000000000000000000000000000000000060003514156050578060005260206000f35b3660008037600080366000845af43d6000803e60008114156070573d6000fd5b3d6000f3fea264697066735822122003d1488ee65e08fa41e58e888a9865554c535f2c77126a82cb4c0f917f31441364736f6c63430007060033496e76616c69642073696e676c65746f6e20616464726573732070726f7669646564';
  /** A further key's signer proxy creation code (vela-core `safe::WEBAUTHN_SIGNER_PROXY_CREATION_CODE`). */
  var WEBAUTHN_SIGNER_PROXY_CREATION_CODE = '610100346100ad57601f6101b538819003918201601f19168301916001600160401b038311848410176100b2578084926080946040528339810103126100ad578051906001600160a01b03821682036100ad5760208101516040820151606090920151926001600160b01b03841684036100ad5760805260a05260c05260e05260405160ec90816100c98239608051816082015260a05181604d015260c051816027015260e0518160010152f35b600080fd5b634e487b7160e01b600052604160045260246000fdfe7f000000000000000000000000000000000000000000000000000000000000000060b63601527f000000000000000000000000000000000000000000000000000000000000000060a03601527f000000000000000000000000000000000000000000000000000000000000000036608001523660006080376000806056360160807f00000000000000000000000000000000000000000000000000000000000000005af43d600060803e60b1573d6080fd5b3d6080f3fea26469706673582212201660515548d15702d720bbc046b457ca85e941a4559ab9f9518488e4c82e5ee964736f6c634300081a0033';

  var EXEC_TRANSACTION = selector('execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)');
  var CREATE_PROXY = selector('createProxyWithNonce(address,bytes,uint256)');
  var SETUP = selector('setup(address[],uint256,address,bytes,address,address,uint256,address)');
  var ENABLE_MODULES = selector('enableModules(address[])');
  var CONFIGURE = selector('configure((uint256,uint256,uint176))');
  var CREATE_SIGNER = selector('createSigner(uint256,uint256,uint176)');

  function padded(n) {
    return n + ((32 - (n % 32)) % 32);
  }

  /** ABI `bytes` tail: its length word, the bytes, zero padding. */
  function bytesTail(bytes) {
    var out = new Uint8Array(32 + padded(bytes.length));
    out.set(word(bytes.length), 0);
    out.set(bytes, 32);
    return out;
  }

  /** `bytes` whose length word is at `at`, canonically: inside `data`, zero padding. */
  function bytesAt(data, at) {
    var len = wordAt(data, at);
    if (len === null || len > BigInt(data.length)) return null;
    var n = Number(len);
    var end = at + 32 + padded(n);
    if (end > data.length) return null;
    for (var i = at + 32 + n; i < end; i++) if (data[i] !== 0) return null;
    return { bytes: data.subarray(at + 32, at + 32 + n), end: end };
  }

  /** A word that holds an address: its high twelve bytes zero. */
  function addressWordAt(data, at) {
    var v = wordAt(data, at);
    if (v === null || v >> 160n !== 0n) return null;
    return checksum('0x' + v.toString(16).padStart(40, '0'));
  }

  function bytesEqual(a, b) {
    if (a.length !== b.length) return false;
    for (var i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
    return true;
  }

  function create2(deployer, salt, initCodeHash) {
    var hash = keccak.hash(concat([new Uint8Array([0xff]), fromHex(deployer), salt, initCodeHash]));
    return checksum(toHex(hash.subarray(12)));
  }

  /** What `createProxyWithNonce(singleton, initializer, saltNonce)` deploys to on Safe's v1.4.1 factory. */
  function proxyAddress(singleton, initializer, saltNonce) {
    var initCodeHash = keccak.hash(concat([fromHex(PROXY_CREATION_CODE), addressWord(singleton)]));
    var salt = keccak.hash(concat([keccak.hash(initializer), word(saltNonce)]));
    return create2(SAFE_PROXY_FACTORY, salt, initCodeHash);
  }

  /**
   * `execTransaction(...)` calldata, read as vela-core `exec_transaction_data`
   * writes it — the ten head words, the data, the signatures, nothing after
   * — or null. `{tx, signatures}`: `tx` in `parseTx`'s shape, without the
   * nonce `execTransaction` does not carry.
   */
  function readExecCall(data) {
    if (selectorOf(data) !== EXEC_TRANSACTION) return null;
    var body = data.subarray(4);
    var to = addressWordAt(body, 0);
    var gasToken = addressWordAt(body, 7 * 32);
    var refundReceiver = addressWordAt(body, 8 * 32);
    var operation = wordAt(body, 3 * 32);
    if (!to || !gasToken || !refundReceiver || operation === null || operation > 255n) return null;
    if (wordAt(body, 2 * 32) !== 320n) return null;
    var inner = bytesAt(body, 320);
    if (!inner || inner.bytes.length > MAX_DATA_BYTES) return null;
    if (wordAt(body, 9 * 32) !== BigInt(inner.end)) return null;
    var signatures = bytesAt(body, inner.end);
    if (!signatures || signatures.end !== body.length) return null;
    return {
      tx: {
        to: to,
        value: wordAt(body, 32),
        data: toHex(inner.bytes),
        operation: Number(operation),
        safeTxGas: wordAt(body, 4 * 32),
        baseGas: wordAt(body, 5 * 32),
        gasPrice: wordAt(body, 6 * 32),
        gasToken: gasToken,
        refundReceiver: refundReceiver,
      },
      signatures: signatures.bytes,
    };
  }

  /**
   * The approvals in `execTransaction`'s signature bytes, as the Safe reads
   * them (`checkNSignatures`): 65-byte entries `r ‖ s ‖ v`, then the
   * contract signatures' tails (vela-core `assemble_signatures`). `v = 0` —
   * a contract's signature (a Vela account's passkey, EIP-1271), `r` the
   * owner; `v = 1` — approved by sending it, or on chain (`approveHash`), `r`
   * the owner; 27/28 (31/32 in the `eth_sign` form) — a plain wallet's ECDSA
   * signature, whose owner only recovering it tells (the multisig does).
   * Null when the bytes are not entries this page can count.
   */
  function readSignatures(sigs) {
    var head = sigs.length;
    var entries = [];
    for (var i = 0; (i + 1) * 65 <= head; i++) {
      var at = i * 65;
      var v = sigs[at + 64];
      if (v === 0 || v === 1) {
        var r = wordAt(sigs, at);
        if (r === null || r >> 160n !== 0n) return null;
        var owner = checksum('0x' + r.toString(16).padStart(40, '0'));
        if (v === 0) {
          var offset = wordAt(sigs, at + 32);
          if (offset === null || offset >= BigInt(sigs.length)) return null;
          // A contract signature's tail starts after every head entry.
          if (Number(offset) < head) head = Number(offset);
          entries.push({ kind: 'contract', owner: owner });
        } else {
          entries.push({ kind: 'sender', owner: owner });
        }
      } else if (v === 27 || v === 28 || v === 31 || v === 32) {
        entries.push({ kind: 'ecdsa', owner: null });
      } else {
        return null;
      }
    }
    if (!entries.length || entries.length * 65 !== head) return null;
    return entries;
  }

  /** `createProxyWithNonce(singleton, initializer, saltNonce)` calldata as vela-core `deploy_call` writes it, or null. */
  function readDeployCall(data) {
    if (selectorOf(data) !== CREATE_PROXY) return null;
    var body = data.subarray(4);
    var singleton = addressWordAt(body, 0);
    if (!singleton || wordAt(body, 32) !== 0x60n) return null;
    var saltNonce = wordAt(body, 64);
    var initializer = bytesAt(body, 96);
    if (saltNonce === null || !initializer || initializer.end !== body.length) return null;
    return { singleton: singleton, saltNonce: saltNonce, initializer: initializer.bytes };
  }

  /** `setup(...)` as vela-core encodes it (`encode_setup_full`), with no payment. */
  function encodeSetup(owners, threshold, to, data, fallbackHandler) {
    return concat([
      fromHex(SETUP),
      word(0x100),
      word(threshold),
      addressWord(to),
      word(0x100 + 32 + owners.length * 32),
      addressWord(fallbackHandler),
      addressWord(ZERO),
      word(0),
      addressWord(ZERO),
      word(owners.length),
    ].concat(owners.map(addressWord)).concat([bytesTail(data)]));
  }

  /** The owners and threshold a `setup` call names, or null (vela-core `genesis_members`). */
  function setupMembers(initializer) {
    if (selectorOf(initializer) !== SETUP) return null;
    var body = initializer.subarray(4);
    var offset = wordAt(body, 0);
    var threshold = wordAt(body, 32);
    if (offset !== 0x100n || threshold === null) return null;
    var count = wordAt(body, 0x100);
    if (count === null || count > BigInt(MAX_OWNERS)) return null;
    var owners = [];
    for (var i = 0; i < Number(count); i++) {
      var owner = addressWordAt(body, 0x120 + i * 32);
      if (!owner) return null;
      owners.push(owner);
    }
    return { owners: owners, threshold: threshold };
  }

  /**
   * A deploy of a multisig as Vela creates one (vela-core `multisig_genesis`):
   * `{owners, threshold, address}`, or null. The setup is re-encoded from the
   * members it names and must be byte for byte the one deployed — no setup
   * hook (`to`, `data`), no payment, the CompatibilityFallbackHandler — with
   * members a multisig can have (distinct, not zero, not the sentinel) and a
   * threshold between one and their number.
   */
  function readMultisigDeploy(deploy) {
    if (!same(deploy.singleton, SAFE_SINGLETON)) return null;
    var members = setupMembers(deploy.initializer);
    if (!members || !members.owners.length) return null;
    var seen = {};
    for (var i = 0; i < members.owners.length; i++) {
      var o = members.owners[i].toLowerCase();
      if (o === ZERO || o === SENTINEL || seen[o]) return null;
      seen[o] = true;
    }
    if (members.threshold < 1n || members.threshold > BigInt(members.owners.length)) return null;
    var expected = encodeSetup(members.owners, members.threshold, ZERO, new Uint8Array(0),
      COMPATIBILITY_FALLBACK_HANDLER);
    if (!bytesEqual(expected, deploy.initializer)) return null;
    return {
      owners: members.owners,
      threshold: members.threshold,
      address: proxyAddress(deploy.singleton, deploy.initializer, deploy.saltNonce),
    };
  }

  /** One MultiSend leg as vela-core packs it (`encode_multisend_tx`): op ‖ to ‖ 0 ‖ length ‖ data. */
  function packLeg(to, data, operation) {
    return concat([new Uint8Array([operation]), fromHex(to), word(0), word(data.length), data]);
  }

  function signerArgs(x, y) {
    return concat([word(x), word(y), word(WEBAUTHN_VERIFIERS)]);
  }

  /** vela-core `compute_webauthn_signer_address`: a further key's own signer. */
  function webauthnSigner(x, y) {
    var initCode = concat([fromHex(WEBAUTHN_SIGNER_PROXY_CREATION_CODE),
      addressWord(WEBAUTHN_SIGNER_SINGLETON), signerArgs(x, y)]);
    return create2(WEBAUTHN_SIGNER_FACTORY, new Uint8Array(32), keccak.hash(initCode));
  }

  /** vela-core `encode_setup_data`: a Vela account's setup from its keys, `keys[0]` first. */
  function velaAccountSetup(keys) {
    var enable = concat([fromHex(ENABLE_MODULES), word(0x20), word(1), addressWord(SAFE_4337_MODULE)]);
    var configure = concat([fromHex(CONFIGURE), signerArgs(keys[0].x, keys[0].y)]);
    var legs = [packLeg(SAFE_MODULE_SETUP, enable, 1), packLeg(WEBAUTHN_SIGNER, configure, 1)];
    var owners = [WEBAUTHN_SIGNER];
    keys.slice(1).forEach(function (key) {
      owners.push(webauthnSigner(key.x, key.y));
      legs.push(packLeg(WEBAUTHN_SIGNER_FACTORY, concat([fromHex(CREATE_SIGNER), signerArgs(key.x, key.y)]), 0));
    });
    var multiSend = concat([fromHex(MULTI_SEND), word(0x20), bytesTail(concat(legs))]);
    return encodeSetup(owners, 1n, VELA_MULTI_SEND, multiSend, SAFE_4337_MODULE);
  }

  // Where a Vela account's keys sit in its setup's MultiSend: the first leg
  // (`enableModules([module])`, 85 + 100 bytes), then `configure(x, y, …)`
  // and one `createSigner(x, y, …)` per further key — each 85 + 100 bytes,
  // its x at 85 + 4 into the leg.
  var ACCOUNT_LEG = 85 + 100;
  var KEY_IN_LEG = 85 + 4;

  /**
   * A deploy of a Vela account (vela-core `vela_account_genesis`): its
   * address, or null. The keys are read from the setup's own MultiSend, the
   * setup is re-encoded from them and must be the one deployed, and the salt
   * nonce must be the hash of the keys, as Vela derives it.
   */
  function readAccountDeploy(deploy) {
    if (!same(deploy.singleton, SAFE_SINGLETON)) return null;
    var init = deploy.initializer;
    var members = setupMembers(init);
    if (!members || members.owners.length < 1 || members.owners.length > MAX_ACCOUNT_KEYS) return null;
    var multiSend = bytesAt(init, 4 + 0x120 + members.owners.length * 32);
    if (!multiSend || selectorOf(multiSend.bytes) !== MULTI_SEND) return null;
    var packedBytes = bytesAt(multiSend.bytes, 36);
    if (!packedBytes || packedBytes.bytes.length !== ACCOUNT_LEG * (1 + members.owners.length)) return null;
    var packed = packedBytes.bytes;
    var keys = [];
    for (var k = 0; k < members.owners.length; k++) {
      var at = ACCOUNT_LEG * (k + 1) + KEY_IN_LEG;
      keys.push({ x: wordAt(packed, at), y: wordAt(packed, at + 32) });
    }
    if (!bytesEqual(velaAccountSetup(keys), init)) return null;
    var xy = concat(keys.map(function (key) { return concat([word(key.x), word(key.y)]); }));
    if (BigInt(toHex(keccak.hash(xy))) !== deploy.saltNonce) return null;
    return { address: proxyAddress(deploy.singleton, init, deploy.saltNonce) };
  }

  function zeroValue(call) {
    var v = call.value === undefined || call.value === null ? '0x0' : String(call.value);
    return quantity(v) === 0n;
  }

  /**
   * An executor's operation, read. `calls` are the operation's legs before
   * its fee leg (`{to, value, data, operation}`, lib/safeop.js), `executor`
   * the account whose operation it is (`userOp.sender`). Null when no leg
   * calls `execTransaction`: the operation is no execution. Otherwise
   * `{multisig, tx, approvals, deploy, activates}` — `deploy` the multisig's
   * own setup (`{owners, threshold}`) when this operation deploys it,
   * `activates` the members' accounts it deploys — or `{refusal}` (an i18n
   * key) for an execution this page cannot read in full.
   */
  function execution(calls, executor) {
    var execs = calls.filter(function (call) {
      return typeof call.data === 'string' && HEX_BYTES.test(call.data) &&
        selectorOf(fromHex(call.data)) === EXEC_TRANSACTION;
    });
    if (!execs.length) return null;
    if (execs.length > 1) return { refusal: 'refuse.msExecExtra' };
    var exec = execs[0];
    if (!ADDRESS.test(exec.to || '') || !zeroValue(exec) || Number(exec.operation || 0) !== 0) {
      return { refusal: 'refuse.msExecShape' };
    }
    var call = readExecCall(fromHex(exec.data));
    var approvals = call && readSignatures(call.signatures);
    if (!approvals || (executor && same(exec.to, executor))) return { refusal: 'refuse.msExecShape' };
    var multisig = checksum(exec.to);
    var out = { multisig: multisig, tx: call.tx, approvals: approvals, deploy: null, activates: [] };
    var contractOwners = approvals.filter(function (a) { return a.kind === 'contract'; })
      .map(function (a) { return a.owner.toLowerCase(); });
    for (var i = 0; i < calls.length; i++) {
      var leg = calls[i];
      if (leg === exec) continue;
      var deploy = same(leg.to || '', SAFE_PROXY_FACTORY) && zeroValue(leg) && Number(leg.operation || 0) === 0 &&
        typeof leg.data === 'string' && HEX_BYTES.test(leg.data) ? readDeployCall(fromHex(leg.data)) : null;
      if (!deploy) return { refusal: 'refuse.msExecExtra' };
      var asMultisig = readMultisigDeploy(deploy);
      if (asMultisig && same(asMultisig.address, multisig) && !out.deploy) {
        out.deploy = { owners: asMultisig.owners, threshold: asMultisig.threshold };
        continue;
      }
      var account = readAccountDeploy(deploy);
      if (account && contractOwners.indexOf(account.address.toLowerCase()) >= 0 &&
        out.activates.indexOf(account.address) < 0) {
        out.activates.push(account.address);
        continue;
      }
      return { refusal: 'refuse.msExecDeploy' };
    }
    return out;
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
    execution: execution,
    validName: validName,
    checksum: checksum,
    toHex: toHex,
    quantity: quantity,
  };
})(window.VelaCS);
