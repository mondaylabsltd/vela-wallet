// BLE central — the browser half of the cross-device signing channel.
//
// The browser can only ever be the central, and Web Bluetooth only exists in a
// document, so this file's counterpart is always one of our native apps acting
// as a GATT peripheral. See PROTOCOL.md for the wire format; this is a faithful
// implementation of sections 1–4 and nothing more.
//
// Zero dependencies: framing is hand-rolled, and every cryptographic operation
// is WebCrypto (P-256 ECDH, HKDF-SHA256, AES-GCM).
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var SERVICE = '76656c61-0001-4000-8000-00805f9b34fb';
  var C2P = '76656c61-0002-4000-8000-00805f9b34fb';
  var P2C = '76656c61-0003-4000-8000-00805f9b34fb';

  var HEADER = 6;
  var DEFAULT_CHUNK = 244; // ATT MTU 247 minus 3 bytes of GATT overhead
  var MIN_CHUNK = 20;      // the default MTU of a very old peripheral
  var REASSEMBLY_TIMEOUT = 10000;

  // --- bytes -----------------------------------------------------------------

  function utf8(text) {
    return new TextEncoder().encode(text);
  }

  function fromUtf8(bytes) {
    return new TextDecoder().decode(bytes);
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
    var padded = text.replace(/-/g, '+').replace(/_/g, '/');
    while (padded.length % 4) padded += '=';
    var binary = atob(padded);
    var out = new Uint8Array(binary.length);
    for (var i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
    return out;
  }

  // --- framing ---------------------------------------------------------------

  function frame(flags, msgId, seq, total, payload) {
    var out = new Uint8Array(HEADER + payload.length);
    out[0] = flags;
    out[1] = msgId;
    out[2] = (seq >> 8) & 0xff;
    out[3] = seq & 0xff;
    out[4] = (total >> 8) & 0xff;
    out[5] = total & 0xff;
    out.set(payload, HEADER);
    return out;
  }

  function split(payload, chunkSize) {
    var frames = [];
    var total = Math.max(1, Math.ceil(payload.length / chunkSize));
    for (var i = 0; i < total; i++) {
      frames.push(payload.subarray(i * chunkSize, (i + 1) * chunkSize));
    }
    return { frames: frames, total: total };
  }

  /** Collects frames until a message is whole. Tolerates out-of-order arrival. */
  function Reassembler(onMessage, onError) {
    this.pending = {};
    this.onMessage = onMessage;
    this.onError = onError;
  }

  Reassembler.prototype.accept = function (bytes) {
    if (bytes.length < HEADER) return;
    var flags = bytes[0];
    var msgId = bytes[1];
    var seq = (bytes[2] << 8) | bytes[3];
    var total = (bytes[4] << 8) | bytes[5];
    var payload = bytes.subarray(HEADER);

    var entry = this.pending[msgId];
    if (!entry || entry.total !== total) {
      entry = this.pending[msgId] = {
        total: total,
        flags: flags,
        parts: new Array(total),
        got: 0,
        timer: null,
      };
      var self = this;
      entry.timer = setTimeout(function () {
        delete self.pending[msgId];
        if (self.onError) self.onError(new Error('message ' + msgId + ' never completed'));
      }, REASSEMBLY_TIMEOUT);
    }
    if (entry.parts[seq] === undefined) {
      entry.parts[seq] = payload;
      entry.got += 1;
    }
    if (entry.got === entry.total) {
      clearTimeout(entry.timer);
      delete this.pending[msgId];
      this.onMessage(entry.flags, concat(entry.parts));
    }
  };

  // --- crypto ----------------------------------------------------------------

  function hkdf(secret, salt, info, length) {
    return crypto.subtle.importKey('raw', secret, 'HKDF', false, ['deriveBits']).then(function (key) {
      return crypto.subtle.deriveBits(
        { name: 'HKDF', hash: 'SHA-256', salt: salt, info: utf8(info) },
        key,
        length * 8,
      );
    }).then(function (bits) { return new Uint8Array(bits); });
  }

  function Session(config) {
    this.key = config.key;          // CryptoKey, AES-GCM
    this.code = config.code;        // six digits, shown on both screens
    this.counters = { c2p: 0n, p2c: 0n };
    this.lastSeen = 0;
    this.outgoing = 0;
  }

  // IV = 4-byte direction tag ‖ 8-byte counter. Never reused: the counter only
  // ever goes up, and each direction keeps its own.
  Session.prototype.iv = function (direction) {
    var counter = this.counters[direction] + 1n;
    this.counters[direction] = counter;
    var out = new Uint8Array(12);
    out.set(utf8(direction === 'c2p' ? 'C2P.' : 'P2C.'), 0);
    for (var i = 0; i < 8; i++) out[11 - i] = Number((counter >> BigInt(8 * i)) & 0xffn);
    return out;
  };

  Session.prototype.seal = function (plaintext, msgId) {
    var iv = this.iv('c2p');
    var aad = utf8('vela-ble/1|c2p|' + msgId);
    return crypto.subtle
      .encrypt({ name: 'AES-GCM', iv: iv, additionalData: aad }, this.key, plaintext)
      .then(function (ct) { return concat([iv, new Uint8Array(ct)]); });
  };

  Session.prototype.open = function (sealed, msgId) {
    var iv = sealed.subarray(0, 12);
    var body = sealed.subarray(12);
    var aad = utf8('vela-ble/1|p2c|' + msgId);
    return crypto.subtle
      .decrypt({ name: 'AES-GCM', iv: iv, additionalData: aad }, this.key, body)
      .then(function (plain) { return new Uint8Array(plain); });
  };

  // --- the channel -----------------------------------------------------------

  function Channel(device, server, c2p, p2c) {
    this.device = device;
    this.server = server;
    this.c2p = c2p;
    this.p2c = p2c;
    this.chunk = DEFAULT_CHUNK;
    this.session = null;
    this.msgId = 0;
    this.handlers = [];
    this.errorHandlers = [];
    this.inbox = [];
    this.waiters = [];
  }

  Channel.prototype.onMessage = function (handler) {
    this.handlers.push(handler);
  };

  Channel.prototype.onError = function (handler) {
    this.errorHandlers.push(handler);
  };

  /** Resolve with the next message, or reject after `timeoutMs`. */
  Channel.prototype.next = function (timeoutMs) {
    var self = this;
    if (this.inbox.length) return Promise.resolve(this.inbox.shift());
    return new Promise(function (resolve, reject) {
      var timer = setTimeout(function () {
        self.waiters = self.waiters.filter(function (w) { return w.resolve !== resolve; });
        reject(new Error('timed out waiting for the peripheral'));
      }, timeoutMs || 30000);
      self.waiters.push({
        resolve: function (message) { clearTimeout(timer); resolve(message); },
      });
    });
  };

  Channel.prototype.deliver = function (message) {
    if (this.waiters.length) this.waiters.shift().resolve(message);
    else this.inbox.push(message);
    this.handlers.forEach(function (handler) { handler(message); });
  };

  Channel.prototype.fail = function (error) {
    this.errorHandlers.forEach(function (handler) { handler(error); });
  };

  Channel.prototype.writeFrames = function (flags, payload) {
    var self = this;
    var msgId = this.msgId = (this.msgId + 1) & 0xff;
    var pieces = split(payload, this.chunk);
    var index = 0;

    function writeNext() {
      if (index >= pieces.frames.length) return Promise.resolve(msgId);
      var body = frame(flags, msgId, index, pieces.total, pieces.frames[index]);
      return self.write(body).then(function () {
        index += 1;
        return writeNext();
      });
    }
    return writeNext();
  };

  // A peripheral that cannot take our chunk size answers with an error; halve
  // and retry rather than giving up, down to the 20 bytes every device accepts.
  Channel.prototype.write = function (bytes) {
    var self = this;
    var characteristic = this.c2p;
    var write = characteristic.writeValueWithoutResponse
      ? characteristic.writeValueWithoutResponse.bind(characteristic)
      : characteristic.writeValue.bind(characteristic);
    return write(bytes).catch(function (error) {
      if (self.chunk <= MIN_CHUNK) throw error;
      self.chunk = Math.max(MIN_CHUNK, Math.floor(self.chunk / 2));
      throw error; // the caller restarts the message at the smaller size
    });
  };

  Channel.prototype.sendPlain = function (object) {
    return this.writeFrames(0, utf8(JSON.stringify(object)));
  };

  Channel.prototype.send = function (object) {
    var self = this;
    if (!this.session) return Promise.reject(new Error('no session: handshake first'));
    object.v = 1;
    object.n = ++this.session.outgoing;
    var msgId = (this.msgId + 1) & 0xff;
    return this.session.seal(utf8(JSON.stringify(object)), msgId).then(function (sealed) {
      return self.writeFrames(1, sealed);
    });
  };

  Channel.prototype.disconnect = function () {
    try {
      if (this.server && this.server.connected) this.server.disconnect();
    } catch (e) { /* already gone */ }
  };

  // --- connect + handshake ---------------------------------------------------

  function isSupported() {
    return typeof navigator !== 'undefined' && !!navigator.bluetooth;
  }

  /**
   * Ask the user to pick a device, connect, and run the ECDH handshake.
   * Must be called from a user gesture — Web Bluetooth requires one, and there
   * is no persistent-permission API to skip the chooser with.
   */
  function connect(options) {
    options = options || {};
    if (!isSupported()) return Promise.reject(new Error('this browser has no Web Bluetooth'));

    var channel;
    return navigator.bluetooth
      .requestDevice({ filters: [{ services: [SERVICE] }], optionalServices: [SERVICE] })
      .then(function (device) {
        return device.gatt.connect().then(function (server) {
          return server.getPrimaryService(SERVICE).then(function (service) {
            return Promise.all([
              service.getCharacteristic(C2P),
              service.getCharacteristic(P2C),
            ]).then(function (pair) {
              channel = new Channel(device, server, pair[0], pair[1]);
              return channel;
            });
          });
        });
      })
      .then(function () { return subscribe(channel); })
      .then(function () { return handshake(channel, options); })
      .then(function () { return channel; });
  }

  function subscribe(channel) {
    var reassembler = new Reassembler(function (flags, payload) {
      if (!flags) {
        channel.deliver(JSON.parse(fromUtf8(payload)));
        return;
      }
      channel.session
        .open(payload, channel.lastInboundId)
        .then(function (plain) {
          var message = JSON.parse(fromUtf8(plain));
          // Replay guard: a counter that does not advance is a replayed frame.
          if (typeof message.n === 'number') {
            if (message.n <= channel.session.lastSeen) {
              channel.fail(new Error('replayed message ' + message.n + ' dropped'));
              return;
            }
            channel.session.lastSeen = message.n;
          }
          channel.deliver(message);
        })
        .catch(function (error) { channel.fail(error); });
    }, function (error) { channel.fail(error); });

    channel.p2c.addEventListener('characteristicvaluechanged', function (event) {
      var view = event.target.value;
      var bytes = new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
      channel.lastInboundId = bytes[1];
      reassembler.accept(bytes);
    });
    return channel.p2c.startNotifications();
  }

  function handshake(channel, options) {
    var ours;
    var nonce = crypto.getRandomValues(new Uint8Array(16));

    return crypto.subtle
      .generateKey({ name: 'ECDH', namedCurve: 'P-256' }, false, ['deriveBits'])
      .then(function (pair) {
        ours = pair;
        return crypto.subtle.exportKey('raw', pair.publicKey);
      })
      .then(function (raw) {
        return channel.sendPlain({
          v: 1,
          t: 'hello',
          role: 'signer',
          pk: b64url(new Uint8Array(raw)),
          nonce: b64url(nonce),
        });
      })
      .then(function () { return channel.next(options.timeoutMs || 30000); })
      .then(function (reply) {
        if (!reply || reply.t !== 'hello' || !reply.pk) {
          throw new Error('peripheral did not answer the handshake');
        }
        channel.peer = { app: reply.app, role: reply.role };
        return crypto.subtle
          .importKey('raw', unb64url(reply.pk), { name: 'ECDH', namedCurve: 'P-256' }, false, [])
          .then(function (peerKey) {
            return crypto.subtle.deriveBits({ name: 'ECDH', public: peerKey }, ours.privateKey, 256);
          })
          .then(function (shared) {
            var salt = concat([nonce, unb64url(reply.nonce)]);
            return Promise.all([
              hkdf(new Uint8Array(shared), salt, 'vela-ble/1 key', 32),
              hkdf(new Uint8Array(shared), salt, 'vela-ble/1 code', 4),
            ]);
          });
      })
      .then(function (derived) {
        var code = ((derived[1][0] << 24 >>> 0) + (derived[1][1] << 16) + (derived[1][2] << 8) + derived[1][3]) % 1000000;
        return crypto.subtle
          .importKey('raw', derived[0], 'AES-GCM', false, ['encrypt', 'decrypt'])
          .then(function (key) {
            channel.session = new Session({
              key: key,
              code: String(code).padStart(6, '0'),
            });
            if (options.onCode) options.onCode(channel.session.code);
            return channel;
          });
      });
  }

  ns.transport = ns.transport || {};
  ns.transport.ble = {
    SERVICE: SERVICE,
    C2P: C2P,
    P2C: P2C,
    isSupported: isSupported,
    connect: connect,
    // exported for tests
    _frame: frame,
    _split: split,
    _Reassembler: Reassembler,
  };
})(window.VelaCS);
