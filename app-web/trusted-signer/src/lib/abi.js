// Minimal ABI reader. No dependencies, on purpose: everything this page uses to
// decide what to show you is code you can read in one sitting.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  function strip(hex) {
    return (hex || '').replace(/^0x/i, '');
  }

  function selectorOf(calldata) {
    var body = strip(calldata);
    return body.length >= 8 ? '0x' + body.slice(0, 8).toLowerCase() : null;
  }

  function wordAt(body, index) {
    return body.slice(index * 64, index * 64 + 64);
  }

  function toBigInt(word) {
    return BigInt('0x' + (word || '0'));
  }

  function toAddress(word) {
    return '0x' + word.slice(24);
  }

  // Split "swap(address,(address,uint256),bytes)" into its top-level types.
  function splitTypes(list) {
    var out = [];
    var depth = 0;
    var current = '';
    for (var i = 0; i < list.length; i++) {
      var c = list[i];
      if (c === '(') depth++;
      if (c === ')') depth--;
      if (c === ',' && depth === 0) {
        out.push(current.trim());
        current = '';
      } else {
        current += c;
      }
    }
    if (current.trim()) out.push(current.trim());
    return out;
  }

  function parseSignature(signature) {
    var open = signature.indexOf('(');
    return {
      name: signature.slice(0, open),
      types: splitTypes(signature.slice(open + 1, signature.lastIndexOf(')'))),
    };
  }

  function isDynamic(type) {
    if (type === 'bytes' || type === 'string') return true;
    if (type.endsWith('[]')) return true;
    if (type.charAt(0) === '(') {
      return splitTypes(type.slice(1, -1)).some(isDynamic);
    }
    return false;
  }

  // Decode one value. `base` is the word offset the current tuple's data starts
  // at — every dynamic offset in ABI encoding is relative to its own tuple.
  function decodeValue(type, body, slot, base) {
    if (type.charAt(0) === '(') {
      var inner = splitTypes(type.slice(1, -1));
      if (isDynamic(type)) {
        var at = Number(toBigInt(wordAt(body, slot))) / 32 + base;
        return decodeTuple(inner, body, at);
      }
      return decodeTuple(inner, body, slot, true);
    }
    if (type.endsWith('[]')) {
      var head = Number(toBigInt(wordAt(body, slot))) / 32 + base;
      var length = Number(toBigInt(wordAt(body, head)));
      var items = [];
      for (var i = 0; i < length; i++) {
        items.push(decodeValue(type.slice(0, -2), body, head + 1 + i, head + 1));
      }
      return items;
    }
    if (type === 'bytes' || type === 'string') {
      var start = Number(toBigInt(wordAt(body, slot))) / 32 + base;
      var size = Number(toBigInt(wordAt(body, start)));
      var raw = body.slice((start + 1) * 64, (start + 1) * 64 + size * 2);
      if (type === 'bytes') return '0x' + raw;
      var text = '';
      for (var j = 0; j < raw.length; j += 2) {
        text += String.fromCharCode(parseInt(raw.substr(j, 2), 16));
      }
      try {
        return decodeURIComponent(escape(text));
      } catch (e) {
        return text;
      }
    }
    var word = wordAt(body, slot);
    if (type === 'address') return toAddress(word);
    if (type === 'bool') return toBigInt(word) !== 0n;
    if (type.startsWith('bytes')) return '0x' + word.slice(0, parseInt(type.slice(5), 10) * 2);
    return toBigInt(word); // uint*/int*
  }

  // `flat` means the tuple is static and its members sit inline from `start`.
  function decodeTuple(types, body, start, flat) {
    var out = [];
    var slot = start;
    for (var i = 0; i < types.length; i++) {
      out.push(decodeValue(types[i], body, slot, start));
      slot += 1; // static members and dynamic offsets both take exactly one word
    }
    void flat;
    return out;
  }

  /** Decode calldata against a signature. Returns null if the shape disagrees. */
  function decode(signature, calldata) {
    try {
      var parsed = parseSignature(signature);
      var body = strip(calldata).slice(8);
      return decodeTuple(parsed.types, body, 0);
    } catch (e) {
      return null;
    }
  }

  /**
   * A call's `value` in wei, or `null` when it cannot be read exactly.
   *
   * Absent, null, "" and "0x" are zero (vela-core RC4); "0x" and hex digits,
   * plain decimal digits, a non-negative safe integer and a non-negative
   * BigInt are that number. Anything else — whitespace, "0X", a sign, "0b"
   * or "0o", a fraction — is unreadable.
   *
   * The page's own grammar, not BigInt()'s: engines disagree at the edges.
   * JavaScriptCore (Safari, the page's engine on the iPhone and the Mac)
   * reads "0x " and " 0x " as 0 where V8 throws, so a verdict left to
   * BigInt() would change with the browser.
   */
  function quantity(value) {
    if (value === undefined || value === null || value === '' || value === '0x') return 0n;
    if (typeof value === 'bigint') return value >= 0n ? value : null;
    if (typeof value === 'number') {
      return Number.isSafeInteger(value) && value >= 0 ? BigInt(value) : null;
    }
    if (typeof value !== 'string') return null;
    if (/^0x[0-9a-fA-F]+$/.test(value) || /^[0-9]+$/.test(value)) return BigInt(value);
    return null;
  }

  /** Walk a decoded tree by index path, e.g. [1, 4]. */
  function at(values, path) {
    var node = values;
    for (var i = 0; i < path.length; i++) {
      if (node == null) return null;
      node = node[path[i]];
    }
    return node;
  }

  ns.abi = {
    strip: strip,
    selectorOf: selectorOf,
    parseSignature: parseSignature,
    splitTypes: splitTypes,
    decode: decode,
    at: at,
    quantity: quantity,
    byteLength: function (calldata) {
      return Math.floor(strip(calldata).length / 2);
    },
  };
})(window.VelaCS);
