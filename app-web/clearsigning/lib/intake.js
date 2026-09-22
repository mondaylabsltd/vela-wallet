// Where requests come in, and where the answers go back.
//
// Six channels, one shape. Every channel opens a SESSION (spec 075 §1.5):
//
//   session.next()      → Promise<request | null>   null: the session is over,
//                                                    and session.endReason says why
//   session.end(reason)   the page ends it (says `bye` where the channel can)
//   session.persistent    true when it carries several requests in order
//                         (loopback WebSocket, postMessage, relay, BLE); the
//                         URL fragment and the extension carry exactly one
//   session.channel       'post' | 'url' | 'ws' | 'ext' | 'ble' | 'relay'
//   session.answered      how many requests have been answered
//
// and every request has one shape:
//
//   { id, intent, context, channel, originVerified, requester, comparisonCode,
//     respond(payload), reject(code, closing) }
//
// `payload` is the answer's own fields. A signing intent answers
// `{result: assertion}` (071); a key ceremony answers `{registration, origin}`
// or `{assertion, origin}` (075 §1.4). Each channel wraps it in its envelope.
//
// `originVerified` is the only claim this layer makes that the sheet acts on:
// it is true ONLY where the browser itself vouches for the sender (postMessage,
// extension messaging). A URL, a QR or a radio link carries whatever the
// requester typed, and the sheet says so out loud. `channel` is the other fact
// resolve.js uses: only an app channel or a verified Vela origin may ask for a
// key to be created.
//
// A persistent session waits for the next request until the wallet says
// `bye`, the channel closes, or it has been idle for five minutes.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var IDLE_MS = 5 * 60 * 1000;

  // Tests shorten the idle clock by defining this before the page loads.
  function idleMs() {
    var override = window.__velaIdleMs;
    return typeof override === 'number' && override > 0 ? override : IDLE_MS;
  }

  function params() {
    return new URLSearchParams(location.search);
  }

  function b64urlToBytes(text) {
    var padded = text.replace(/-/g, '+').replace(/_/g, '/');
    while (padded.length % 4) padded += '=';
    var binary = atob(padded);
    var out = new Uint8Array(binary.length);
    for (var i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
    return out;
  }

  function bytesToB64url(bytes) {
    var binary = '';
    for (var i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }

  // A one-time token or a pairing link has no business in browser history.
  function scrubFragment() {
    try {
      history.replaceState(null, '', location.pathname + location.search);
    } catch (e) { /* not fatal */ }
  }

  // Native DecompressionStream — still zero dependencies, and hex calldata
  // compresses to roughly a third, which is what keeps big batches inside a URL.
  function inflate(bytes) {
    if (typeof DecompressionStream !== 'function') {
      return Promise.reject(new Error('this browser cannot inflate the payload'));
    }
    var stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('deflate-raw'));
    return new Response(stream).arrayBuffer().then(function (buffer) {
      return new TextDecoder().decode(buffer);
    });
  }

  // --- the session -------------------------------------------------------------

  function Session(channel, persistent, events) {
    this.channel = channel;
    this.persistent = persistent;
    this.events = events || {};
    this.inbox = [];
    this.waiters = [];
    this.ended = false;
    this.endReason = null;   // 'bye' | 'closed' | 'idle' | 'single' | 'error'
    this.endDetail = null;   // for 'error': an i18n key
    this.answered = 0;
    this.received = 0;
    this.inFlight = null;    // the request on screen, until it is answered
    this.comparisonCode = null;
    /** The requester's own name for itself, where a channel carries one. */
    this.requesterApp = '';
    this.close = null;       // how the channel says goodbye, set by the channel
    this.idleTimer = null;
    this.armIdle();
  }

  Session.prototype.emit = function (name) {
    var handler = this.events[name];
    if (typeof handler === 'function') handler.apply(null, Array.prototype.slice.call(arguments, 1));
  };

  // Five minutes with nothing to do ends a persistent session. A request on
  // screen is not "nothing": the person may be reading it.
  Session.prototype.armIdle = function () {
    var self = this;
    clearTimeout(this.idleTimer);
    if (!this.persistent || this.ended) return;
    this.idleTimer = setTimeout(function () {
      if (self.inFlight) self.armIdle();
      else self.end('idle');
    }, idleMs());
  };

  /**
   * A channel hands in a request. Its `send(kind, body)` delivers one answer
   * (`kind` = 'result' | 'error'); the wrapper makes sure there is exactly one.
   */
  Session.prototype.push = function (fields, send) {
    if (this.ended) return;
    var self = this;
    var request = Object.assign({ comparisonCode: this.comparisonCode, answered: false, gone: false }, fields);
    function finish(kind, body, closing) {
      if (request.answered) return Promise.resolve();
      request.answered = true;
      if (self.inFlight === request) self.inFlight = null;
      if (!request.gone) self.answered += 1;
      self.armIdle();
      return request.gone ? Promise.resolve() : Promise.resolve(send(kind, body, closing));
    }
    request.respond = function (payload) { return finish('result', payload || {}); };
    request.reject = function (code, closing) { return finish('error', { code: code || 'user_rejected' }, closing); };
    this.received += 1;
    clearTimeout(this.idleTimer);
    if (this.waiters.length) this.hand(this.waiters.shift(), request);
    else this.inbox.push(request);
  };

  Session.prototype.hand = function (resolve, request) {
    this.inFlight = request;
    resolve(request);
  };

  Session.prototype.next = function () {
    var self = this;
    if (this.inbox.length) {
      return new Promise(function (resolve) { self.hand(resolve, self.inbox.shift()); });
    }
    if (this.ended) return Promise.resolve(null);
    return new Promise(function (resolve) { self.waiters.push(resolve); });
  };

  /** The request on screen can no longer be answered (its wallet went away). */
  Session.prototype.lose = function () {
    var request = this.inFlight;
    if (!request || request.answered) return;
    request.gone = true;
    this.inFlight = null;
    this.emit('onGone', request);
  };

  /** The channel is over. */
  Session.prototype.finish = function (reason, detail) {
    if (this.ended) return;
    this.lose();
    this.ended = true;
    this.endReason = reason;
    this.endDetail = detail || null;
    clearTimeout(this.idleTimer);
    var waiters = this.waiters;
    this.waiters = [];
    waiters.forEach(function (resolve) { resolve(null); });
    this.emit('onEnd', this);
  };

  /** The page ends it, saying goodbye where the channel can. */
  Session.prototype.end = function (reason) {
    if (this.ended) return;
    try { if (this.close) this.close(reason); } catch (e) { /* best effort */ }
    this.finish(reason);
  };

  // --- 1. same browser: opened by the wallet or a dApp, answered to the opener -

  function fromPostMessage(options) {
    if (!window.opener) return null;
    var session = new Session('post', true, options);
    var opener = window.opener;
    var peer = null; // { source, origin }, pinned by the first intent

    function post(message) {
      try { peer.source.postMessage(message, peer.origin); } catch (e) { /* the opener is gone */ }
    }

    function onMessage(event) {
      var data = event.data;
      if (!data || typeof data !== 'object' || session.ended) return;
      // Whoever sent the first intent is the only one heard from then on —
      // and the first must come from the window that opened this page.
      if (peer ? event.source !== peer.source || event.origin !== peer.origin : event.source !== opener) return;
      if (data.vela === 'bye') {
        if (peer) session.finish('bye');
        return;
      }
      if (data.vela !== 'intent') return;
      if (!peer) {
        peer = { source: event.source, origin: event.origin };
        clearTimeout(first);
      }
      var id = data.id;
      session.push({
        id: id,
        intent: data.intent,
        context: data.context || {},
        channel: 'post',
        // event.origin is filled in by the browser. This is the only channel
        // pair where "who is asking" is a fact rather than a claim.
        originVerified: true,
        requester: event.origin,
      }, function (kind, body) {
        post(Object.assign({ vela: kind, id: id }, body));
      });
    }

    window.addEventListener('message', onMessage);
    var first = setTimeout(function () {
      if (!peer) session.finish('error', 'ui.openerSilent');
    }, 30000);
    // A closed opener is a finished session: nobody is left to answer.
    var watch = setInterval(function () {
      var target = peer ? peer.source : opener;
      var gone = true;
      try { gone = !target || target.closed; } catch (e) { gone = true; }
      if (gone || session.ended) {
        clearInterval(watch);
        session.finish('closed');
      }
    }, 1000);
    session.close = function (reason) {
      if (peer) post({ vela: 'bye', v: 1, reason: reason || 'done' });
    };
    // Tell the opener we are ready. It cannot know when our scripts finished.
    opener.postMessage({ vela: 'ready', v: 1 }, '*');
    return Promise.resolve(session);
  }

  // --- 2. same machine, native app: URL fragment in, loopback callback out ---

  function fromUrlFragment(options) {
    var hash = new URLSearchParams(location.hash.replace(/^#/, ''));
    if (!hash.get('i')) return null;

    var raw = b64urlToBytes(hash.get('i'));
    var callback = hash.get('cb') ? new TextDecoder().decode(b64urlToBytes(hash.get('cb'))) : null;
    var token = hash.get('t') || '';
    var decoded = hash.get('z') === '1'
      ? inflate(raw)
      : Promise.resolve(new TextDecoder().decode(raw));

    return decoded.then(function (json) {
      var payload = JSON.parse(json);
      scrubFragment();

      function callbackUrl(query) {
        return callback + (callback.indexOf('?') >= 0 ? '&' : '?') +
          't=' + encodeURIComponent(token) + '&' + query;
      }

      // A live page navigates; a page being closed cannot. `sendBeacon` exists
      // for exactly that moment, and without it "closing refuses" would be a
      // promise this channel silently breaks.
      function answer(query, closing) {
        if (!callback) return;
        var url = callbackUrl(query);
        if (closing && navigator.sendBeacon && navigator.sendBeacon(url)) return;
        location.href = url;
      }

      var session = new Session('url', false, options);
      session.push({
        id: null,
        intent: payload.intent,
        context: payload.context || {},
        channel: 'url',
        originVerified: false, // any page or program can open this URL
        requester: payload.intent && payload.intent.origin,
      }, function (kind, body, closing) {
        if (kind === 'error') {
          answer('error=' + encodeURIComponent(body.code), closing);
          return;
        }
        // The callback carries one JSON object: a signing answer's `result`
        // as it always has, otherwise the answer's own fields.
        var object = body.result !== undefined ? body.result : body;
        answer('result=' + encodeURIComponent(
          bytesToB64url(new TextEncoder().encode(JSON.stringify(object))),
        ));
      });
      session.finish('single');
      return session;
    });
  }

  // --- 2b. same device, native app: a WebSocket on the app's loopback --------
  //
  // A phone app cannot catch a redirect to 127.0.0.1 — it is suspended the
  // moment the browser tab covers it — but it can keep a loopback socket open
  // while it shows this page in its own browser tab (SFSafariViewController,
  // a Custom Tab). `#p=<port>&t=<token>`: the page connects, says hello with
  // the token (proof the app that listens is the app that opened it), then
  // answers each intent the app sends on that socket, in order, until the app
  // says `bye` or closes it. A tab closed with a request unanswered closes the
  // socket, and the app reads that as "closed without signing".
  //
  // The browser may first ask the person to let this page reach "other apps
  // and services on this device" (Chrome's Local Network Access); the status
  // line says so while the socket waits.

  function fromWebSocket(options) {
    var hash = new URLSearchParams(location.hash.replace(/^#/, ''));
    var port = Number(hash.get('p'));
    var token = hash.get('t') || '';
    if (!port || port < 1 || port > 65535 || !token) return null;
    scrubFragment();

    var session = new Session('ws', true, options);
    var socket = new WebSocket('ws://127.0.0.1:' + port);
    var outgoing = 0;
    var lastSeen = 0;
    var waiting = setTimeout(function () { session.emit('onWaiting'); }, 1500);

    function send(object) {
      outgoing = Math.max(outgoing, lastSeen) + 1;
      try {
        socket.send(JSON.stringify(Object.assign({ v: 1, n: outgoing }, object)));
      } catch (e) { /* gone */ }
    }

    socket.onopen = function () {
      clearTimeout(waiting);
      socket.send(JSON.stringify({ v: 1, t: 'hello', token: token }));
    };
    socket.onerror = function () { /* onclose follows */ };
    // A close before any intent: the wallet is not there (or stopped
    // waiting). After a request, unanswered: the wallet gave up — signing now
    // would sign into nothing, so the request is marked gone.
    socket.onclose = function () {
      clearTimeout(waiting);
      session.finish(session.received ? 'closed' : 'error', session.received ? null : 'ui.walletGone');
    };
    socket.onmessage = function (event) {
      var message = null;
      try { message = JSON.parse(event.data); } catch (e) { return; }
      if (!message || typeof message !== 'object') return;
      // `n`, when the app sends it, only goes up.
      if (typeof message.n === 'number') {
        if (message.n <= lastSeen) return;
        lastSeen = message.n;
      }
      if (message.t === 'bye') {
        session.finish('bye');
        try { socket.close(1000); } catch (e) { /* gone */ }
        return;
      }
      if (message.t !== 'intent') return;
      var id = message.id;
      session.push({
        id: id,
        intent: message.intent,
        context: message.context || {},
        channel: 'ws',
        // Any app on the device could listen on a port; the token proves
        // this one opened us, not who the requesting site is.
        originVerified: false,
        requester: message.intent && message.intent.origin,
      }, function (kind, body) {
        send(Object.assign({ t: kind, id: id }, body));
      });
    };
    session.close = function (reason) {
      send({ t: 'bye', reason: reason || 'done' });
      try { socket.close(1000); } catch (e) { /* gone */ }
    };
    return Promise.resolve(session);
  }

  // --- 3. inside the extension: the background worker holds the request -----

  function fromExtension(options) {
    if (!ns.signer.isExtension()) return null;
    var id = params().get('id');
    if (!id) return null;
    return new Promise(function (resolve, reject) {
      chrome.runtime.sendMessage({ vela: 'take', id: id }, function (pending) {
        if (chrome.runtime.lastError || !pending) {
          reject(new Error((chrome.runtime.lastError && chrome.runtime.lastError.message) ||
            'the request is gone — the background worker may have restarted'));
          return;
        }
        var session = new Session('ext', false, options);
        session.push({
          id: id,
          intent: pending.intent,
          context: pending.context || {},
          channel: 'ext',
          // The background worker recorded sender.origin, which the browser
          // filled in when the page connected. That claim survives the hop.
          originVerified: !!pending.originVerified,
          requester: pending.requester,
        }, function (kind, body) {
          return new Promise(function (done) {
            chrome.runtime.sendMessage(kind === 'result'
              ? { vela: 'result', id: id, result: body.result, payload: body }
              : { vela: 'error', id: id, code: body.code }, done);
          });
        });
        session.finish('single');
        resolve(session);
      });
    });
  }

  // --- 4. cross-device: BLE ---------------------------------------------------

  function fromBle(options) {
    return ns.transport.ble.connect(options).then(function (channel) {
      var session = new Session('ble', true, options);
      session.comparisonCode = channel.session.code;
      // What the peer calls itself, from its hello. Nothing verifies it on
      // this channel, so it travels as a claim and is drawn as one.
      session.requesterApp = channel.session.peerApp || '';
      channel.onMessage(function (message) {
        if (message.t === 'bye') {
          session.finish('bye');
          channel.disconnect();
          return;
        }
        if (message.t !== 'intent') return;
        var id = message.id;
        session.push({
          id: id,
          intent: message.intent,
          context: message.context || {},
          channel: 'ble',
          originVerified: false, // proximity is proven, identity is not
          requester: message.intent && message.intent.origin,
        }, function (kind, body) {
          return channel.send(Object.assign({ t: kind, id: id }, body));
        });
      });
      if (channel.device && channel.device.addEventListener) {
        channel.device.addEventListener('gattserverdisconnected', function () { session.finish('closed'); });
      }
      session.close = function (reason) {
        channel.send({ t: 'bye', reason: reason || 'done' })
          .then(function () { channel.disconnect(); }, function () { channel.disconnect(); });
      };
      return session;
    });
  }

  // --- 5. cross-device: the relay ----------------------------------------------

  function fromRelay(options) {
    var link = ns.transport.relay.parseLink(location.hash);
    if (!link) return null;
    scrubFragment();

    var session = new Session('relay', true, options);
    if (!link.valid) {
      session.finish('error', 'ui.relayBadLink');
      return Promise.resolve(session);
    }
    var ENDS = {
      foreign: 'ui.relayForeign',
      expired: 'ui.relayExpired',
      taken: 'ui.relayTaken',
      bad: 'ui.relayBadLink',
      closed: 'ui.relayClosed',
    };
    var channel = ns.transport.relay.connect(link, {
      onOpen: function () { session.emit('onState', 'relayWaiting'); },
      onJoined: function () { session.emit('onState', 'relayJoined'); },
      onCode: function (code) {
        session.comparisonCode = code;
        session.emit('onCode', code);
      },
      onLeft: function () {
        // The keys that request was sealed under are gone with the wallet.
        session.comparisonCode = null;
        session.lose();
        session.emit('onState', 'relayLeft');
      },
      onMessage: function (message) {
        if (message.t === 'bye') {
          session.finish('bye');
          channel.close('done');
          return;
        }
        if (message.t !== 'intent') return;
        var id = message.id;
        var epoch = channel.epoch;
        session.push({
          id: id,
          intent: message.intent,
          context: message.context || {},
          channel: 'relay',
          // The wallet's key matched the link and the person compared the
          // code — but the site a wallet names is still its own word.
          originVerified: false,
          requester: message.intent && message.intent.origin,
        }, function (kind, body) {
          // An answer sealed for a session the wallet has since left would
          // reach nobody who asked for it.
          if (channel.epoch !== epoch) return Promise.resolve();
          return channel.send(Object.assign({ t: kind, id: id }, body));
        });
      },
      onEnd: function (reason) {
        if (reason === 'closed' && session.received) session.finish('closed');
        else session.finish('error', ENDS[reason] || 'ui.relayClosed');
      },
    }, options.relay || {});
    session.close = function (reason) { channel.close(reason || 'done'); };
    session.relay = channel;
    return Promise.resolve(session);
  }

  /**
   * Open a session. `?ch=` forces a channel; otherwise the first that
   * recognises the situation wins. BLE is never automatic: it needs a user
   * gesture. `options` carries the page's callbacks: onCode(code),
   * onGone(request), onWaiting(), onState(name), onEnd(session).
   */
  function open(options) {
    options = options || {};
    var forced = params().get('ch');
    var adapters = {
      post: function () { return fromPostMessage(options); },
      url: function () { return fromUrlFragment(options); },
      ws: function () { return fromWebSocket(options); },
      ext: function () { return fromExtension(options); },
      ble: function () { return fromBle(options); },
      relay: function () { return fromRelay(options); },
    };

    if (forced) {
      var chosen = adapters[forced];
      if (!chosen) return Promise.reject(new Error('unknown channel ' + forced));
      var forcedResult = chosen();
      return forcedResult || Promise.reject(new Error('channel ' + forced + ' found no request'));
    }

    var order = ['ext', 'relay', 'url', 'post'];
    for (var i = 0; i < order.length; i++) {
      var attempt = adapters[order[i]]();
      if (attempt) return attempt;
    }
    return Promise.reject(new Error('no signing request on any channel'));
  }

  /** The first request of a session (the 071 shape, for one-shot callers). */
  function receive(options) {
    return open(options).then(function (session) {
      return session.next().then(function (request) {
        if (!request) throw new Error(session.endDetail || 'the session ended before a request arrived');
        return request;
      });
    });
  }

  ns.intake = {
    open: open,
    receive: receive,
    Session: Session,
    fromPostMessage: fromPostMessage,
    fromUrlFragment: fromUrlFragment,
    fromWebSocket: fromWebSocket,
    fromExtension: fromExtension,
    fromBle: fromBle,
    fromRelay: fromRelay,
  };
})(window.VelaCS);
