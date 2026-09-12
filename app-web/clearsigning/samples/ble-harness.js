// Drives the BLE central against a simulated peripheral. Dev-only: the real
// entry point wires the same calls to the signing sheet.
(function (ns) {
  'use strict';

  var log = document.getElementById('log');
  window.__trace = [];

  function trace(line) {
    window.__trace.push(line);
    log.textContent += line + '\n';
  }

  window.__run = function () {
    window.__state = { code: null, intent: null, rendered: null, error: null, done: false };
    return ns.transport.ble
      .connect({
        onCode: function (code) {
          window.__state.code = code;
          trace('handshake done, comparison code ' + code);
        },
      })
      .then(function (channel) {
        window.__channel = channel;
        trace('waiting for an intent…');
        return channel.next(20000).then(function (message) {
          window.__state.intent = message;
          trace('got ' + message.t + ' id=' + message.id);
          if (message.t !== 'intent') throw new Error('expected an intent, got ' + message.t);

          // Exactly what the real page does with it.
          var view = ns.resolve(message.intent, message.context || {});
          var sheet = ns.render(view, {});
          document.body.appendChild(sheet);
          window.__state.rendered = {
            intentKey: view.intentKey,
            risk: view.risk,
            level: view.level,
            sentence: ns.i18n.t(view.sentence),
            refuse: !!view.refuse,
          };
          trace('rendered: ' + window.__state.rendered.sentence);

          return channel.send({
            t: 'result',
            id: message.id,
            signature: '0x' + 'ab'.repeat(64),
            account: '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894',
          });
        });
      })
      .then(function () {
        window.__state.done = true;
        trace('result sent');
        return window.__state;
      })
      .catch(function (error) {
        window.__state.error = error.message;
        trace('FAILED ' + error.message);
        return window.__state;
      });
  };

  document.getElementById('go').addEventListener('click', window.__run);
})(window.VelaCS);
