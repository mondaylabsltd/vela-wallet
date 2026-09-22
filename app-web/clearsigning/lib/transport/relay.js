// The relay — the cross-device WebSocket channel (spec 075, contracts/relay.md).
//
// The wallet shows a pairing link:
//
//   <this page>#relay=<wss URL>&room=<22 base64url>&rk=<22 base64url>&v=1
//
// The page reads it from its fragment (never sent to any server), joins the
// room as `signer`, and once the relay says both ends are there (`joined`) it
// runs the same handshake BLE runs (lib/transport/secure.js) under the label
// `vela-relay/1`. Two checks stop a stand-in:
//
//   · `rk` — the page refuses a requester hello whose key does not hash to the
//     fingerprint in the link. The relay, or anyone who guessed the room,
//     cannot pose as the wallet: they do not hold its key.
//   · the six-digit code — shown on both screens; the person confirms it on the
//     wallet before the wallet sends anything. That stops a stand-in for THIS
//     page (a stolen link).
//
// The relay is blind: after the two hellos every frame is binary,
// IV ‖ AES-GCM, and the relay only ever forwards it.
//
// Each `joined` starts a fresh handshake (a new key pair, new nonces, a new
// code): a wallet that drops out and comes back is checked again from scratch.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var ID = /^[A-Za-z0-9_-]{22}$/;
  var LOOPBACK = /^(localhost|127\.0\.0\.1|\[::1\])$/;

  // wss anywhere; ws only on this machine's loopback (tests, a local relay) —
  // the same rule as the wallet's Settings row.
  function validRelay(url) {
    var parsed;
    try { parsed = new URL(url); } catch (e) { return false; }
    if (parsed.username || parsed.password || parsed.search || parsed.hash) return false;
    if (parsed.protocol === 'wss:') return !!parsed.hostname;
    return parsed.protocol === 'ws:' && LOOPBACK.test(parsed.hostname);
  }

  /** `{relay, room, rk, v, valid}` from a fragment, or null when it is not a pairing link. */
  function parseLink(hash) {
    var fragment = new URLSearchParams(String(hash || '').replace(/^#/, ''));
    var relay = fragment.get('relay');
    if (!relay) return null;
    var link = {
      relay: relay.replace(/\/+$/, ''),
      room: fragment.get('room') || '',
      rk: fragment.get('rk') || '',
      v: fragment.get('v') || '',
    };
    link.valid = validRelay(link.relay) && ID.test(link.room) && ID.test(link.rk) && link.v === '1';
    return link;
  }

  function roomUrl(link) {
    return link.relay + '/v1/rooms/' + link.room + '?role=signer';
  }

  // The relay's close codes (relay.md §1), as reasons the page can word.
  var CLOSES = { 4408: 'expired', 4409: 'taken', 4400: 'bad', 1009: 'bad' };

  /**
   * Join the room. `handlers`:
   *   onOpen()          — connected to the relay, waiting for the wallet
   *   onJoined()        — both ends are in the room; the handshake starts
   *   onCode(code)      — the session is up; show the six digits
   *   onLeft()          — the wallet dropped out (the room waits for it)
   *   onMessage(object) — a decrypted message from the wallet
   *   onEnd(reason)     — `foreign` (rk check failed), `expired`, `taken`,
   *                       `bad`, or `closed`; the channel is gone for good
   * `options.secret` / `options.nonce` inject randomness (tests only).
   */
  function connect(link, handlers, options) {
    var channel = new Channel(link, handlers || {}, options || {});
    channel.open();
    return channel;
  }

  function Channel(link, handlers, options) {
    this.link = link;
    this.handlers = handlers;
    this.options = options;
    this.ws = null;
    this.attempt = null;   // the handshake in progress, per `joined`
    this.session = null;   // secure.js Session once the code is up
    this.epoch = 0;        // bumps with every session, so stale answers stay unsent
    this.outgoing = 0;     // `n` of our last message
    this.lastSeen = 0;     // `n` of the wallet's last accepted message
    this.ended = false;
  }

  Channel.prototype.emit = function (name) {
    var handler = this.handlers[name];
    if (typeof handler === 'function') handler.apply(null, Array.prototype.slice.call(arguments, 1));
  };

  Channel.prototype.open = function () {
    var self = this;
    var ws;
    try {
      ws = this.ws = new WebSocket(roomUrl(this.link));
    } catch (e) {
      this.end('bad');
      return;
    }
    ws.binaryType = 'arraybuffer';
    ws.onopen = function () { self.emit('onOpen'); };
    ws.onmessage = function (event) { self.receive(event.data); };
    ws.onclose = function (event) { self.end(CLOSES[event.code] || 'closed'); };
    ws.onerror = function () { /* onclose follows with the code */ };
  };

  Channel.prototype.receive = function (data) {
    if (this.ended) return;
    if (typeof data === 'string') {
      var frame = null;
      try { frame = JSON.parse(data); } catch (e) { return; }
      if (!frame || typeof frame !== 'object') return;
      // The relay's own frames carry a "relay" key; an end's never do.
      if (Object.prototype.hasOwnProperty.call(frame, 'relay')) {
        if (frame.relay === 'joined') this.joined();
        else if (frame.relay === 'left') this.left();
        return;
      }
      if (frame.t === 'hello' && this.attempt && !this.attempt.peerHello) {
        this.attempt.peerHello = frame;
        this.complete(this.attempt);
      }
      return;
    }
    // Binary: sealed. Nothing is accepted before the code is up.
    var session = this.session;
    if (!session) return;
    var self = this;
    session.open(new Uint8Array(data)).then(function (plain) {
      if (session !== self.session || self.ended) return;
      var message;
      try { message = JSON.parse(new TextDecoder().decode(plain)); } catch (e) { return; }
      // Every message carries its `n`, and `n` only goes up (relay.md §2.5).
      if (!message || typeof message.n !== 'number' || message.n <= self.lastSeen) return;
      self.lastSeen = message.n;
      self.emit('onMessage', message);
    }, function () { /* replayed or unreadable: dropped, never delivered */ });
  };

  Channel.prototype.joined = function () {
    var self = this;
    var attempt = { handshake: null, peerHello: null, completing: false };
    this.attempt = attempt;
    this.session = null;
    this.emit('onJoined');
    ns.transport.secure
      .handshake({ role: 'signer', secret: this.options.secret, nonce: this.options.nonce })
      .then(function (handshake) {
        if (self.attempt !== attempt || self.ended) return;
        attempt.handshake = handshake;
        self.ws.send(JSON.stringify(handshake.hello()));
        self.complete(attempt);
      }, function () { self.end('bad'); });
  };

  // Both hellos are in: the rk check, then the key and the code.
  Channel.prototype.complete = function (attempt) {
    var self = this;
    if (!attempt.handshake || !attempt.peerHello || attempt.completing) return;
    attempt.completing = true;
    attempt.handshake.complete(attempt.peerHello, 'relay', this.link.rk).then(function (session) {
      if (self.attempt !== attempt || self.ended) return;
      self.attempt = null;
      self.session = session;
      self.epoch += 1;
      self.outgoing = 0;
      self.lastSeen = 0;
      self.emit('onCode', session.code);
    }, function (error) {
      if (self.attempt !== attempt) return;
      // A requester that is not the link's wallet gets nothing — not even a
      // code to phish with — and the page leaves the room.
      self.end(error && error.code === 'foreign_peer' ? 'foreign' : 'bad');
    });
  };

  Channel.prototype.left = function () {
    this.attempt = null;
    this.session = null;
    this.emit('onLeft');
  };

  /** Seal and send one message; `v` and `n` are added here. */
  Channel.prototype.send = function (object) {
    if (!this.session || this.ended) return Promise.reject(new Error('the wallet is not connected'));
    var ws = this.ws;
    object.v = 1;
    object.n = Math.max(this.outgoing, this.lastSeen) + 1;
    this.outgoing = object.n;
    return this.session.seal(new TextEncoder().encode(JSON.stringify(object))).then(function (sealed) {
      if (ws.readyState !== 1) throw new Error('the relay connection closed');
      ws.send(sealed);
    });
  };

  /** Leave: say `bye` when a session is up, then close. */
  Channel.prototype.close = function (reason) {
    var self = this;
    if (this.ended) return Promise.resolve();
    var said = this.session
      ? this.send({ t: 'bye', reason: reason || 'done' }).catch(function () { /* gone already */ })
      : Promise.resolve();
    return said.then(function () {
      try { self.ws.close(1000, 'bye'); } catch (e) { /* already closed */ }
      self.end('closed');
    });
  };

  Channel.prototype.end = function (reason) {
    if (this.ended) return;
    this.ended = true;
    this.session = null;
    this.attempt = null;
    try { if (this.ws && this.ws.readyState < 2) this.ws.close(1000); } catch (e) { /* gone */ }
    this.emit('onEnd', reason);
  };

  ns.transport = ns.transport || {};
  ns.transport.relay = {
    parseLink: parseLink,
    connect: connect,
    _validRelay: validRelay,
  };
})(window.VelaCS);
