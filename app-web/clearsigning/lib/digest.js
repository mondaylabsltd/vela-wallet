// What actually gets signed.
//
// The single most dangerous shortcut in a signing UI is to let the requester
// hand over "the 32 bytes to sign". Whoever supplies the digest decides what
// you sign, and the sheet above it becomes decoration. So this file derives the
// digest from the SAME intent the sheet rendered, and when it cannot, the
// answer is a refusal — never a placeholder.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var keccak = ns.keccak;

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
    var body = (hex || '').replace(/^0x/i, '');
    var out = new Uint8Array(body.length / 2);
    for (var i = 0; i < out.length; i++) out[i] = parseInt(body.substr(i * 2, 2), 16);
    return out;
  }

  function toHex(bytes) {
    var s = '';
    for (var i = 0; i < bytes.length; i++) s += bytes[i].toString(16).padStart(2, '0');
    return '0x' + s;
  }

  function word(value) {
    var v = typeof value === 'bigint' ? value : BigInt(value);
    if (v < 0n) v = (1n << 256n) + v;
    var hex = v.toString(16).padStart(64, '0');
    return fromHex(hex);
  }

  // --- EIP-191: personal_sign ------------------------------------------------

  function personalSignHash(payload) {
    var bytes = /^0x([0-9a-fA-F]{2})*$/.test(payload) ? fromHex(payload) : utf8(payload);
    // EIP-191 prefix: the 0x19 byte is written explicitly rather than as a
    // literal control character in the source — a byte you cannot see is a
    // byte nobody reviews.
    var prefix = concat([new Uint8Array([0x19]), utf8('Ethereum Signed Message:\n' + bytes.length)]);
    return keccak.hash(concat([prefix, bytes]));
  }

  // --- EIP-712: signTypedData ------------------------------------------------

  function dependencies(types, primary, found) {
    found = found || [];
    if (found.indexOf(primary) >= 0 || !types[primary]) return found;
    found.push(primary);
    types[primary].forEach(function (field) {
      var base = field.type.replace(/\[\d*\]$/, '');
      dependencies(types, base, found);
    });
    return found;
  }

  function encodeType(types, primary) {
    var deps = dependencies(types, primary).filter(function (name) { return name !== primary; });
    deps.sort();
    return [primary].concat(deps).map(function (name) {
      return name + '(' + types[name].map(function (f) { return f.type + ' ' + f.name; }).join(',') + ')';
    }).join('');
  }

  function typeHash(types, primary) {
    return keccak.hash(utf8(encodeType(types, primary)));
  }

  function encodeValue(types, type, value) {
    if (type.endsWith(']')) {
      var base = type.replace(/\[\d*\]$/, '');
      var items = value.map(function (item) { return encodeValue(types, base, item); });
      return keccak.hash(concat(items));
    }
    if (types[type]) return hashStruct(types, type, value);
    if (type === 'string') return keccak.hash(utf8(value));
    if (type === 'bytes') return keccak.hash(fromHex(value));
    if (type === 'address') return word(BigInt(value));
    if (type === 'bool') return word(value ? 1 : 0);
    if (/^bytes\d+$/.test(type)) {
      var fixed = new Uint8Array(32);
      fixed.set(fromHex(value).subarray(0, 32), 0);
      return fixed;
    }
    return word(BigInt(value)); // uint*/int*
  }

  function hashStruct(types, primary, data) {
    var parts = [typeHash(types, primary)];
    types[primary].forEach(function (field) {
      parts.push(encodeValue(types, field.type, data[field.name]));
    });
    return keccak.hash(concat(parts));
  }

  function typedDataHash(typedData) {
    var types = Object.assign({}, typedData.types);
    // The domain is hashed with the same machinery, so it must be in `types`.
    if (!types.EIP712Domain) {
      types.EIP712Domain = Object.keys(typedData.domain || {}).map(function (name) {
        var kinds = {
          name: 'string', version: 'string', chainId: 'uint256',
          verifyingContract: 'address', salt: 'bytes32',
        };
        return { name: name, type: kinds[name] || 'string' };
      });
    }
    var domainSeparator = hashStruct(types, 'EIP712Domain', typedData.domain || {});
    var message = hashStruct(types, typedData.primaryType, typedData.message || {});
    return keccak.hash(concat([new Uint8Array([0x19, 0x01]), domainSeparator, message]));
  }

  // --- the entry point -------------------------------------------------------

  /**
   * Derive the exact bytes to sign for an intent.
   * Returns { hash, describes } or { refuse: '<i18n key>' }.
   */
  function digestFor(intent, context) {
    context = context || {};

    // A Safe does not verify a bare EIP-191/712 hash: EIP-1271 checks the hash
    // wrapped in SafeMessage under the Safe's own domain. Signing the bare one
    // produces a signature that verifies nowhere, so wrap whenever the request
    // tells us which account and chain it is for.
    function forAccount(hash, kind) {
      if (context.account && context.chainId) {
        return {
          hash: ns.safeop.messageHash(hash, context.chainId, context.account),
          describes: kind + ' → SafeMessage',
        };
      }
      // No account was named, so we cannot wrap it — the renderer says so in
      // the reader's own language rather than this file inventing words.
      return { hash: hash, describes: kind, unwrapped: true };
    }

    if (intent.method === 'personal_sign') {
      return forAccount(personalSignHash(intent.params[0]), 'EIP-191');
    }
    if (intent.method.indexOf('signTypedData') >= 0) {
      var raw = intent.params[1];
      var data = typeof raw === 'string' ? JSON.parse(raw) : raw;
      return forAccount(typedDataHash(data), 'EIP-712');
    }
    if (intent.method === 'eth_sign') {
      // The payload IS the digest. Signing it is exactly the blind signature the
      // sheet warns about, so this page will not produce one.
      return { refuse: 'refuse.ethSign' };
    }
    // A transaction digest is the Safe 4337 SafeOp hash. It covers the whole
    // assembled operation, so the requester has to hand the operation over —
    // and the caller then has to prove the operation contains the call it
    // displayed (see sign.js). Without an operation there is nothing to hash,
    // and this page will not invent one.
    if (intent.method === 'eth_sendTransaction' || intent.method === 'wallet_sendCalls') {
      var operation = context.operation;
      if (!operation || !operation.userOp || !context.chainId) {
        return { refuse: 'refuse.noSafeOp' };
      }
      var result = {
        hash: ns.safeop.hash(operation.userOp, context.chainId, {
          entryPoint: operation.entryPoint,
          module: operation.module,
        }),
        describes: 'SafeOp',
      };
      // A different module or entry point is not automatically an attack, but
      // it is not the Vela account either, and nobody should sign for it by
      // accident.
      var offCanon = (operation.module &&
          operation.module.toLowerCase() !== ns.safeop.SAFE_4337_MODULE.toLowerCase()) ||
        (operation.entryPoint &&
          operation.entryPoint.toLowerCase() !== ns.safeop.ENTRY_POINT.toLowerCase());
      if (offCanon) result.warn = 'warn.nonCanonicalModule';
      return result;
    }
    return { refuse: 'refuse.unknownMethod' };
  }

  ns.digest = {
    of: digestFor,
    personalSignHash: personalSignHash,
    typedDataHash: typedDataHash,
    encodeType: encodeType,
    toHex: toHex,
    fromHex: fromHex,
  };
})(window.VelaCS);
