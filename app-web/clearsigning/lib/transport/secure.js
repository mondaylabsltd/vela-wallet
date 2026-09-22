// The end-to-end session a cross-device channel runs: BLE (PROTOCOL.md §3) and
// the relay (specs/075-clear-signer-channel/contracts/relay.md §2).
//
// P-256 ECDH → HKDF-SHA256 → AES-256-GCM, and a six-digit code both screens
// show — the one place a stand-in on the path is caught. The channel's label
// (`vela-ble/1`, `vela-relay/1`) goes into every derivation and every AAD, so a
// message from one channel can never be replayed into the other.
//
// Byte-identical to vela-core's `clear_signer::secure` (Rust), and pinned
// against it by `rust/crates/vela-core/tests/vectors/secure-session.json`
// (samples/secure-vectors.mjs). Nothing here knows about frames or sockets:
// BLE adds its framing around it, the relay sends each sealed message as one
// binary WebSocket frame.
//
// Roles are the BLE ones. The SIGNER is this page (BLE's central) and speaks
// first; the REQUESTER is the wallet (the peripheral). `c2p` is signer →
// requester, `p2c` is requester → signer.
//
// Randomness is injectable (`secret`, `nonce`) so the vectors can be checked;
// the page itself never passes either, and then the key pair is generated
// non-extractable by WebCrypto.
//
// Zero dependencies: every primitive is WebCrypto.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var LABELS = { ble: 'vela-ble/1', relay: 'vela-relay/1' };

  // --- bytes -----------------------------------------------------------------

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

  function b64url(bytes) {
    var binary = '';
    for (var i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }

  function unb64url(text) {
    if (typeof text !== 'string' || !/^[A-Za-z0-9_-]*={0,2}$/.test(text)) return null;
    var padded = text.replace(/=+$/, '').replace(/-/g, '+').replace(/_/g, '/');
    while (padded.length % 4) padded += '=';
    try {
      var binary = atob(padded);
      var out = new Uint8Array(binary.length);
      for (var i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
      return out;
    } catch (e) {
      return null;
    }
  }

  function fail(code, message) {
    var error = new Error(message);
    error.code = code;
    return error;
  }

  // --- keys --------------------------------------------------------------------

  // PKCS#8 PrivateKeyInfo for a P-256 key, without the optional public key: the
  // importer derives it. Used only when a test injects a bare 32-byte secret.
  var PKCS8_PREFIX = [
    0x30, 0x41, 0x02, 0x01, 0x00, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
    0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x04, 0x27, 0x30, 0x25, 0x02, 0x01,
    0x01, 0x04, 0x20,
  ];

  var ECDH = { name: 'ECDH', namedCurve: 'P-256' };

  function injected(secret, publicKey) {
    if (!(secret instanceof Uint8Array) || secret.length !== 32) {
      return Promise.reject(fail('bad_secret', 'the secret is not 32 bytes'));
    }
    if (publicKey) {
      if (publicKey.length !== 65 || publicKey[0] !== 0x04) {
        return Promise.reject(fail('bad_secret', 'the public key is not an uncompressed P-256 point'));
      }
      return crypto.subtle.importKey('jwk', {
        kty: 'EC', crv: 'P-256', ext: false,
        d: b64url(secret), x: b64url(publicKey.subarray(1, 33)), y: b64url(publicKey.subarray(33)),
      }, ECDH, false, ['deriveBits']).then(function (privateKey) {
        return { privateKey: privateKey, publicRaw: new Uint8Array(publicKey) };
      }, function () { throw fail('bad_secret', 'the secret is not a P-256 scalar'); });
    }
    var der = concat([new Uint8Array(PKCS8_PREFIX), secret]);
    return crypto.subtle.importKey('pkcs8', der, ECDH, true, ['deriveBits'])
      .then(function (privateKey) {
        return crypto.subtle.exportKey('jwk', privateKey).then(function (jwk) {
          return {
            privateKey: privateKey,
            publicRaw: concat([new Uint8Array([0x04]), unb64url(jwk.x), unb64url(jwk.y)]),
          };
        });
      }, function () { throw fail('bad_secret', 'the secret is not a P-256 scalar'); });
  }

  function generated() {
    return crypto.subtle.generateKey(ECDH, false, ['deriveBits']).then(function (pair) {
      return crypto.subtle.exportKey('raw', pair.publicKey).then(function (raw) {
        return { privateKey: pair.privateKey, publicRaw: new Uint8Array(raw) };
      });
    });
  }

  function hkdf(secret, salt, info, length) {
    return crypto.subtle.importKey('raw', secret, 'HKDF', false, ['deriveBits']).then(function (key) {
      return crypto.subtle.deriveBits(
        { name: 'HKDF', hash: 'SHA-256', salt: salt, info: utf8(info) },
        key,
        length * 8,
      );
    }).then(function (bits) { return new Uint8Array(bits); });
  }

  /** `b64url(SHA-256(pk)[0..16])` — the requester key's fingerprint (`rk`). */
  function fingerprint(publicKey) {
    return crypto.subtle.digest('SHA-256', publicKey).then(function (digest) {
      return b64url(new Uint8Array(digest).subarray(0, 16));
    });
  }

  // --- the handshake -----------------------------------------------------------

  function Handshake(keys, nonce, role) {
    this.privateKey = keys.privateKey;
    this.publicKey = keys.publicRaw; // 65 bytes, 04 ‖ x ‖ y
    this.nonce = nonce;
    this.role = role;
  }

  /** This side's hello: `{v:1, t:'hello', role, pk, nonce[, app]}`. */
  Handshake.prototype.hello = function (app) {
    var hello = { v: 1, t: 'hello', role: this.role, pk: b64url(this.publicKey), nonce: b64url(this.nonce) };
    if (app) hello.app = app;
    return hello;
  };

  /**
   * Finish with the peer's hello (object or JSON text). `expectedFingerprint`
   * is the page's `rk` check — the requester it was paired with; BLE, which has
   * no link, passes nothing. Rejects with `.code`:
   * `bad_hello`, `wrong_role`, `foreign_peer`.
   */
  Handshake.prototype.complete = function (peerHello, label, expectedFingerprint) {
    var self = this;
    var hello = peerHello;
    if (typeof hello === 'string') {
      try { hello = JSON.parse(hello); } catch (e) { hello = null; }
    }
    if (!hello || typeof hello !== 'object' || hello.t !== 'hello') {
      return Promise.reject(fail('bad_hello', 'the peer did not answer the handshake'));
    }
    if (hello.role !== 'signer' && hello.role !== 'requester') {
      return Promise.reject(fail('bad_hello', 'the peer named no role'));
    }
    if (hello.role === this.role) {
      return Promise.reject(fail('wrong_role', 'the peer spoke in our own role'));
    }
    var peerPk = unb64url(hello.pk);
    var peerNonce = unb64url(hello.nonce);
    if (!peerPk || !peerNonce || peerNonce.length !== 16) {
      return Promise.reject(fail('bad_hello', 'the peer hello is malformed'));
    }
    var name = LABELS[label] || label;
    var check = expectedFingerprint === undefined || expectedFingerprint === null
      ? Promise.resolve()
      : fingerprint(peerPk).then(function (actual) {
        if (actual !== expectedFingerprint) {
          throw fail('foreign_peer', 'the peer key does not match the pairing link');
        }
      });

    return check.then(function () {
      return crypto.subtle.importKey('raw', peerPk, ECDH, false, [])
        .catch(function () { throw fail('bad_hello', 'the peer key is not a P-256 point'); });
    }).then(function (peerKey) {
      return crypto.subtle.deriveBits({ name: 'ECDH', public: peerKey }, self.privateKey, 256);
    }).then(function (shared) {
      var salt = self.role === 'signer'
        ? concat([self.nonce, peerNonce])
        : concat([peerNonce, self.nonce]);
      return Promise.all([
        hkdf(new Uint8Array(shared), salt, name + ' key', 32),
        hkdf(new Uint8Array(shared), salt, name + ' code', 4),
      ]);
    }).then(function (derived) {
      var c = derived[1];
      var code = ((c[0] << 24) >>> 0) + (c[1] << 16) + (c[2] << 8) + c[3];
      return crypto.subtle.importKey('raw', derived[0], 'AES-GCM', false, ['encrypt', 'decrypt'])
        .then(function (key) {
          return new Session({
            key: key,
            code: String(code % 1000000).padStart(6, '0'),
            label: name,
            role: self.role,
            peerPublicKey: peerPk,
          });
        });
    });
  };

  /**
   * One side, before the peer's hello. `options.role` is `signer` (the page)
   * or `requester` (the wallet — tests only). `options.secret` (32 bytes, plus
   * optionally `options.publicKey`) and `options.nonce` (16 bytes) are for
   * vectors; without them both are fresh randomness.
   */
  function handshake(options) {
    options = options || {};
    var role = options.role || 'signer';
    if (role !== 'signer' && role !== 'requester') return Promise.reject(fail('bad_hello', 'unknown role ' + role));
    var nonce = options.nonce || crypto.getRandomValues(new Uint8Array(16));
    if (nonce.length !== 16) return Promise.reject(fail('bad_secret', 'the nonce is not 16 bytes'));
    var keys = options.secret ? injected(options.secret, options.publicKey) : generated();
    return keys.then(function (k) { return new Handshake(k, new Uint8Array(nonce), role); });
  }

  // --- the session -------------------------------------------------------------

  function Session(config) {
    this.key = config.key;         // CryptoKey, AES-GCM
    this.code = config.code;       // six digits, shown on both screens
    this.label = config.label;
    this.role = config.role;
    this.peerPublicKey = config.peerPublicKey;
    this.sent = 0n;
    this.received = 0n;
    // Seals and opens run one at a time, in call order. Otherwise two messages
    // in flight could resolve out of counter order (the peer would refuse the
    // later-arriving lower counter), and two opens of one replayed frame could
    // both pass the counter check before either recorded it.
    this.queue = Promise.resolve();
  }

  Session.prototype.outgoing = function () { return this.role === 'signer' ? 'c2p' : 'p2c'; };
  Session.prototype.incoming = function () { return this.role === 'signer' ? 'p2c' : 'c2p'; };

  function tagOf(direction) {
    return utf8(direction === 'c2p' ? 'C2P.' : 'P2C.');
  }

  // IV = 4-byte direction tag ‖ 8-byte big-endian counter. Never reused: each
  // direction keeps its own counter and it only goes up, from 1.
  function iv(direction, counter) {
    var out = new Uint8Array(12);
    out.set(tagOf(direction), 0);
    for (var i = 0; i < 8; i++) out[11 - i] = Number((counter >> BigInt(8 * i)) & 0xffn);
    return out;
  }

  // The relay binds the IV's counter; BLE binds the frame's msgId (§3.5).
  Session.prototype.aad = function (direction, counter, tail) {
    var end = tail === undefined || tail === null ? counter.toString() : String(tail);
    return utf8(this.label + '|' + direction + '|' + end);
  };

  Session.prototype.serial = function (work) {
    var run = this.queue.then(work, work);
    this.queue = run.catch(function () { /* the next one still runs */ });
    return run;
  };

  /** `IV(12) ‖ AES-GCM(plaintext)` — this side's next message. */
  Session.prototype.seal = function (plaintext, tail) {
    var self = this;
    this.sent += 1n;
    var counter = this.sent;
    var direction = this.outgoing();
    return this.serial(function () {
      var nonce = iv(direction, counter);
      return crypto.subtle.encrypt(
        { name: 'AES-GCM', iv: nonce, additionalData: self.aad(direction, counter, tail) },
        self.key,
        plaintext,
      ).then(function (ct) { return concat([nonce, new Uint8Array(ct)]); });
    });
  };

  /**
   * Open the peer's next message. Its IV must carry the peer's direction and a
   * counter above every one already opened — a replay or a reordering is
   * refused, never decrypted. Rejects with `.code` `replayed` or `unreadable`.
   */
  Session.prototype.open = function (sealed, tail) {
    var self = this;
    return this.serial(function () {
      if (!(sealed instanceof Uint8Array) || sealed.length < 12 + 16) {
        throw fail('unreadable', 'the message is too short');
      }
      var head = sealed.subarray(0, 12);
      var direction = self.incoming();
      var tag = tagOf(direction);
      for (var i = 0; i < 4; i++) {
        if (head[i] !== tag[i]) throw fail('replayed', 'the message came from our own direction');
      }
      var counter = 0n;
      for (var j = 4; j < 12; j++) counter = (counter << 8n) | BigInt(head[j]);
      if (counter <= self.received) throw fail('replayed', 'replayed message (counter ' + counter + ')');
      return crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: head, additionalData: self.aad(direction, counter, tail) },
        self.key,
        sealed.subarray(12),
      ).then(function (plain) {
        self.received = counter;
        return new Uint8Array(plain);
      }, function () {
        throw fail('unreadable', 'the message could not be opened');
      });
    });
  };

  ns.transport = ns.transport || {};
  ns.transport.secure = {
    LABELS: LABELS,
    handshake: handshake,
    fingerprint: fingerprint,
    Session: Session,
    // shared byte helpers for the transports
    _b64url: b64url,
    _unb64url: unb64url,
    _concat: concat,
  };
})(window.VelaCS);
