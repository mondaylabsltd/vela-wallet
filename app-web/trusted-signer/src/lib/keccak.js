// Keccak-256, ~80 lines, no dependencies.
//
// It is here for one reason: a descriptor that claims selector 0xa9059cbb means
// `transfer(address,uint256)` must be checkable, not trusted. Without this the
// whole point of the page collapses — a poisoned descriptor could label any
// call anything it liked and the rendering would faithfully lie.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var MASK = (1n << 64n) - 1n;

  var ROT = [
    0, 1, 62, 28, 27,
    36, 44, 6, 55, 20,
    3, 10, 43, 25, 39,
    41, 45, 15, 21, 8,
    18, 2, 61, 56, 14,
  ];

  var RC = [
    0x0000000000000001n, 0x0000000000008082n, 0x800000000000808an, 0x8000000080008000n,
    0x000000000000808bn, 0x0000000080000001n, 0x8000000080008081n, 0x8000000000008009n,
    0x000000000000008an, 0x0000000000000088n, 0x0000000080008009n, 0x000000008000000an,
    0x000000008000808bn, 0x800000000000008bn, 0x8000000000008089n, 0x8000000000008003n,
    0x8000000000008002n, 0x8000000000000080n, 0x000000000000800an, 0x800000008000000an,
    0x8000000080008081n, 0x8000000000008080n, 0x0000000080000001n, 0x8000000080008008n,
  ];

  function rotl(x, n) {
    var b = BigInt(n);
    if (b === 0n) return x;
    return ((x << b) | (x >> (64n - b))) & MASK;
  }

  function permute(A) {
    for (var round = 0; round < 24; round++) {
      var C = [0n, 0n, 0n, 0n, 0n];
      var x, y;
      for (x = 0; x < 5; x++) C[x] = A[x] ^ A[x + 5] ^ A[x + 10] ^ A[x + 15] ^ A[x + 20];
      for (x = 0; x < 5; x++) {
        var D = C[(x + 4) % 5] ^ rotl(C[(x + 1) % 5], 1);
        for (y = 0; y < 25; y += 5) A[x + y] = A[x + y] ^ D;
      }
      var B = new Array(25).fill(0n);
      for (x = 0; x < 5; x++) {
        for (y = 0; y < 5; y++) {
          B[y + 5 * ((2 * x + 3 * y) % 5)] = rotl(A[x + 5 * y], ROT[x + 5 * y]);
        }
      }
      for (y = 0; y < 25; y += 5) {
        for (x = 0; x < 5; x++) {
          A[x + y] = B[x + y] ^ ((~B[((x + 1) % 5) + y] & MASK) & B[((x + 2) % 5) + y]);
        }
      }
      A[0] = A[0] ^ RC[round];
    }
  }

  /** keccak256 over bytes → 32-byte Uint8Array. */
  function keccak256(input) {
    var rate = 136;
    var padded = new Uint8Array(Math.ceil((input.length + 1) / rate) * rate);
    padded.set(input, 0);
    padded[input.length] = 0x01;
    padded[padded.length - 1] |= 0x80;

    var A = new Array(25).fill(0n);
    for (var offset = 0; offset < padded.length; offset += rate) {
      for (var i = 0; i < rate / 8; i++) {
        var lane = 0n;
        for (var b = 7; b >= 0; b--) lane = (lane << 8n) | BigInt(padded[offset + i * 8 + b]);
        A[i] = A[i] ^ lane;
      }
      permute(A);
    }

    var out = new Uint8Array(32);
    for (var w = 0; w < 4; w++) {
      var lane2 = A[w];
      for (var k = 0; k < 8; k++) {
        out[w * 8 + k] = Number((lane2 >> BigInt(8 * k)) & 0xffn);
      }
    }
    return out;
  }

  function hex(bytes) {
    var s = '';
    for (var i = 0; i < bytes.length; i++) s += bytes[i].toString(16).padStart(2, '0');
    return '0x' + s;
  }

  ns.keccak = {
    hash: keccak256,
    hex: function (input) {
      return hex(keccak256(input));
    },
    /** The 4-byte selector a function signature really has. */
    selector: function (signature) {
      return hex(keccak256(new TextEncoder().encode(signature))).slice(0, 10);
    },
  };
})(window.VelaCS);
