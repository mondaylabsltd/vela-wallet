// Where a signing intent comes in, and where the answer goes back.
//
// Five channels, one shape. Every adapter resolves to the same object:
//
//   { intent, context, originVerified, requester, respond(result), reject(code) }
//
// `originVerified` is the only claim this layer makes that the sheet acts on:
// it is true ONLY for channels where the browser itself vouches for the sender
// (postMessage, extension messaging). A URL, a QR or a radio link carries
// whatever the requester typed, and the sheet says so out loud.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

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

  // --- 1. same browser: opened by the dApp, answered through the opener ------

  function fromPostMessage() {
    if (!window.opener) return null;
    return new Promise(function (resolve, reject) {
      var settled = false;
      function onMessage(event) {
        var data = event.data;
        if (!data || data.vela !== 'intent' || settled) return;
        settled = true;
        window.removeEventListener('message', onMessage);
        resolve({
          intent: data.intent,
          context: data.context || {},
          // event.origin is filled in by the browser. This is the only channel
          // pair where "who is asking" is a fact rather than a claim.
          originVerified: true,
          requester: event.origin,
          respond: function (result) {
            event.source.postMessage(
              { vela: 'result', id: data.id, result: result }, event.origin,
            );
            return Promise.resolve();
          },
          reject: function (code) {
            event.source.postMessage(
              { vela: 'error', id: data.id, code: code || 'user_rejected' }, event.origin,
            );
            return Promise.resolve();
          },
        });
      }
      window.addEventListener('message', onMessage);
      // Tell the opener we are ready. It cannot know when our scripts finished.
      window.opener.postMessage({ vela: 'ready', v: 1 }, '*');
      setTimeout(function () {
        if (settled) return;
        window.removeEventListener('message', onMessage);
        reject(new Error('the opener never sent an intent'));
      }, 30000);
    });
  }

  // --- 2. same machine, native app: URL fragment in, loopback callback out ---

  function fromUrlFragment() {
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

      // A one-time token has no business sitting in browser history.
      try {
        history.replaceState(null, '', location.pathname + location.search);
      } catch (e) { /* not fatal */ }

      function callbackUrl(query) {
        return callback + (callback.indexOf('?') >= 0 ? '&' : '?') +
          't=' + encodeURIComponent(token) + '&' + query;
      }

      // A live page navigates; a page being closed cannot. `sendBeacon` exists
      // for exactly that moment, and without it "closing refuses" would be a
      // promise this channel silently breaks.
      function answer(query, closing) {
        if (!callback) return Promise.resolve();
        var url = callbackUrl(query);
        if (closing && navigator.sendBeacon && navigator.sendBeacon(url)) {
          return Promise.resolve();
        }
        location.href = url;
        return Promise.resolve();
      }

      return {
        intent: payload.intent,
        context: payload.context || {},
        originVerified: false, // any local program can open this URL
        requester: payload.intent && payload.intent.origin,
        respond: function (result) {
          return answer('result=' + encodeURIComponent(
            bytesToB64url(new TextEncoder().encode(JSON.stringify(result))),
          ));
        },
        reject: function (code, closing) {
          return answer('error=' + encodeURIComponent(code || 'user_rejected'), closing);
        },
      };
    });
  }

  // --- 2b. same device, native app: a WebSocket on the app's loopback --------
  //
  // A phone app cannot catch a redirect to 127.0.0.1 — it is suspended the
  // moment the browser tab covers it — but it can keep a loopback socket open
  // while it shows this page in its own browser tab (SFSafariViewController,
  // a Custom Tab). `#p=<port>&t=<token>`: the page connects, says hello with
  // the token (proof the app that listens is the app that opened it), gets ONE
  // intent and answers it once. A tab closed without answering closes the
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
    try {
      history.replaceState(null, '', location.pathname + location.search);
    } catch (e) { /* not fatal */ }

    return new Promise(function (resolve, reject) {
      var socket = new WebSocket('ws://127.0.0.1:' + port);
      var settled = false;
      var answered = false;
      var waiting = setTimeout(function () {
        if (options && options.onWaiting) options.onWaiting();
      }, 1500);
      function fail(message) {
        if (settled) return;
        settled = true;
        clearTimeout(waiting);
        reject(new Error(message));
      }
      socket.onopen = function () {
        clearTimeout(waiting);
        socket.send(JSON.stringify({ v: 1, t: 'hello', token: token }));
      };
      // Before the intent: the wallet is not there (or stopped waiting).
      // After it, unanswered: the wallet gave up — signing now would sign
      // into nothing, so the page says so instead of offering the slider.
      socket.onerror = function () { fail('ui.walletGone'); };
      socket.onclose = function () {
        if (settled && !answered && options && options.onGone) options.onGone();
        fail('ui.walletGone');
      };
      socket.onmessage = function (event) {
        var message = null;
        try { message = JSON.parse(event.data); } catch (e) { return; }
        if (!message || message.t !== 'intent' || settled) return;
        settled = true;
        function send(body) {
          if (answered) return Promise.resolve();
          answered = true;
          try { socket.send(JSON.stringify(Object.assign({ v: 1, id: message.id }, body))); } catch (e) { /* gone */ }
          return Promise.resolve();
        }
        resolve({
          intent: message.intent,
          context: message.context || {},
          // Any app on the device could listen on a port; the token proves
          // this one opened us, not who the requesting site is.
          originVerified: false,
          requester: message.intent && message.intent.origin,
          respond: function (result) { return send({ t: 'result', result: result }); },
          reject: function (code) { return send({ t: 'error', code: code || 'user_rejected' }); },
        });
      };
    });
  }

  // --- 3. inside the extension: the background worker holds the request -----

  function fromExtension() {
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
        resolve({
          intent: pending.intent,
          context: pending.context || {},
          // The background worker recorded sender.origin, which the browser
          // filled in when the page connected. That claim survives the hop.
          originVerified: !!pending.originVerified,
          requester: pending.requester,
          respond: function (result) {
            return new Promise(function (done) {
              chrome.runtime.sendMessage({ vela: 'result', id: id, result: result }, done);
            });
          },
          reject: function (code) {
            return new Promise(function (done) {
              chrome.runtime.sendMessage({ vela: 'error', id: id, code: code || 'user_rejected' }, done);
            });
          },
        });
      });
    });
  }

  // --- 4. cross-device: BLE ---------------------------------------------------

  function fromBle(options) {
    return ns.transport.ble.connect(options).then(function (channel) {
      return channel.next(60000).then(function (message) {
        if (message.t !== 'intent') throw new Error('expected an intent, got ' + message.t);
        return {
          intent: message.intent,
          context: message.context || {},
          originVerified: false, // proximity is proven, identity is not
          requester: message.intent && message.intent.origin,
          comparisonCode: channel.session.code,
          respond: function (result) {
            return channel.send({ t: 'result', id: message.id, result: result });
          },
          reject: function (code) {
            return channel.send({ t: 'error', id: message.id, code: code || 'user_rejected' });
          },
        };
      });
    });
  }

  /**
   * Pick a channel. `?ch=` forces one; otherwise the first that recognises the
   * situation wins. BLE is never automatic: it needs a user gesture.
   */
  function receive(options) {
    options = options || {};
    var forced = params().get('ch');
    var adapters = {
      post: fromPostMessage,
      url: fromUrlFragment,
      ws: function () { return fromWebSocket(options); },
      ext: fromExtension,
      ble: function () { return fromBle(options); },
    };

    if (forced) {
      var chosen = adapters[forced];
      if (!chosen) return Promise.reject(new Error('unknown channel ' + forced));
      var forcedResult = chosen();
      return forcedResult || Promise.reject(new Error('channel ' + forced + ' found no request'));
    }

    var order = ['ext', 'url', 'post'];
    for (var i = 0; i < order.length; i++) {
      var attempt = adapters[order[i]]();
      if (attempt) return attempt.then(function (request) {
        return request;
      });
    }
    return Promise.reject(new Error('no signing request on any channel'));
  }

  ns.intake = {
    receive: receive,
    fromPostMessage: fromPostMessage,
    fromUrlFragment: fromUrlFragment,
    fromWebSocket: fromWebSocket,
    fromExtension: fromExtension,
    fromBle: fromBle,
  };
})(window.VelaCS);
