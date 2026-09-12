// The other half of the honesty claim: the calldata in the gallery is BUILT
// from the same signature the renderer decodes, with the selector derived by
// keccak. Nothing is hand-typed hex that could quietly disagree with its label.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  function word(value) {
    var v = typeof value === 'bigint' ? value : BigInt(value);
    return v.toString(16).padStart(64, '0');
  }

  function addressWord(address) {
    return address.toLowerCase().replace(/^0x/, '').padStart(64, '0');
  }

  function isDynamic(type) {
    if (type === 'bytes' || type === 'string') return true;
    if (type.endsWith('[]')) return true;
    if (type.charAt(0) === '(') return ns.abi.splitTypes(type.slice(1, -1)).some(isDynamic);
    return false;
  }

  function staticWords(type) {
    if (type.charAt(0) === '(') {
      return ns.abi.splitTypes(type.slice(1, -1)).reduce(function (n, t) { return n + staticWords(t); }, 0);
    }
    return 1;
  }

  function encodeStatic(type, value) {
    if (type === 'address') return addressWord(value);
    if (type === 'bool') return word(value ? 1 : 0);
    if (type.charAt(0) === '(') {
      var types = ns.abi.splitTypes(type.slice(1, -1));
      return types.map(function (t, i) { return encodeStatic(t, value[i]); }).join('');
    }
    return word(value);
  }

  function encodeDynamic(type, value) {
    if (type === 'bytes' || type === 'string') {
      var body;
      if (type === 'string') {
        var bytes = new TextEncoder().encode(value);
        body = Array.from(bytes).map(function (b) { return b.toString(16).padStart(2, '0'); }).join('');
      } else {
        body = value.replace(/^0x/, '');
      }
      var padded = body + '0'.repeat((64 - (body.length % 64)) % 64);
      return word(body.length / 2) + padded;
    }
    if (type.endsWith('[]')) {
      var inner = type.slice(0, -2);
      return word(value.length) + value.map(function (v) { return encodeStatic(inner, v); }).join('');
    }
    throw new Error('unsupported dynamic type ' + type);
  }

  function encodeTuple(types, values) {
    var headSize = types.reduce(function (n, t) {
      return n + (isDynamic(t) ? 1 : staticWords(t));
    }, 0);
    var head = '';
    var tail = '';
    types.forEach(function (type, i) {
      if (isDynamic(type)) {
        head += word(headSize * 32 + tail.length / 2);
        tail += encodeDynamic(type, values[i]);
      } else {
        head += encodeStatic(type, values[i]);
      }
    });
    return head + tail;
  }

  /** encodeCall('transfer(address,uint256)', [to, 1000n]) → 0x… */
  function encodeCall(signature, values) {
    var parsed = ns.abi.parseSignature(signature);
    return ns.keccak.selector(signature) + encodeTuple(parsed.types, values);
  }

  /** Safe multiSend payload: operation ‖ to ‖ value ‖ len ‖ data, repeated. */
  function packMultiSend(legs) {
    return '0x' + legs.map(function (leg) {
      var data = (leg.data || '0x').replace(/^0x/, '');
      return '00' +
        leg.to.toLowerCase().replace(/^0x/, '') +
        word(leg.value || 0) +
        word(data.length / 2) +
        data;
    }).join('');
  }

  ns.encode = { call: encodeCall, packMultiSend: packMultiSend, word: word };
})(window.VelaCS);
