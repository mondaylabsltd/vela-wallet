// What a dApp does to reach the signing page in the SAME browser: open a
// window, wait for it to say it is ready, hand over the intent, wait for the
// answer. No server, no extension, and the origin on both sides is checked by
// the browser rather than claimed by either party.
(function () {
  'use strict';

  var SIGNER = 'https://getvela.app/sign.html?ch=post&lang=en';
  var out = document.getElementById('out');

  function buildIntent() {
    return {
      method: 'personal_sign',
      origin: location.origin,
      params: [
        '0x' + Array.from(new TextEncoder().encode(
          'getvela.app wants you to sign in with your Ethereum account:\n' +
          '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894\n\n' +
          'Sign in to the dApp simulator.\n\n' +
          'URI: https://getvela.app\nVersion: 1\nChain ID: 1\nNonce: 5f2a91c0\n' +
          'Issued At: 2026-09-07T02:00:00Z',
        )).map(function (b) { return b.toString(16).padStart(2, '0'); }).join(''),
        '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894',
      ],
    };
  }

  window.__openSigner = function () {
    var intent = buildIntent();
    var child = window.open(SIGNER, 'vela-signer', 'width=460,height=760');
    window.addEventListener('message', function (event) {
      if (event.origin !== new URL(SIGNER).origin) return; // never trust a stray sender
      if (event.data && event.data.vela === 'ready') {
        child.postMessage({ vela: 'intent', id: 'sim-1', intent: intent, context: {
          chainName: 'Ethereum',
          signer: { name: 'Vela', letter: 'V' },
        } }, event.origin);
      }
      if (event.data && event.data.vela === 'result') {
        window.__answer = event.data.result;
        out.textContent = JSON.stringify(event.data.result, null, 2);
      }
      if (event.data && event.data.vela === 'error') {
        window.__answer = { error: event.data.code };
        out.textContent = 'refused: ' + event.data.code;
      }
    });
    return true;
  };

  // The same intent, handed to the EXTENSION instead. A port rather than a
  // one-shot message: the port keeps the service worker alive while the person
  // reads the sheet, and if the worker dies anyway the dApp sees a disconnect
  // instead of waiting forever.
  window.__openViaExtension = function (extensionId) {
    if (!window.chrome || !chrome.runtime || !chrome.runtime.connect) {
      window.__answer = { error: 'no chrome.runtime on this page' };
      return false;
    }
    var port = chrome.runtime.connect(extensionId, { name: 'vela-sign' });
    port.onMessage.addListener(function (message) {
      if (message.vela === 'result') window.__answer = message.result;
      if (message.vela === 'error') window.__answer = { error: message.code };
      out.textContent = JSON.stringify(window.__answer, null, 2);
    });
    port.onDisconnect.addListener(function () {
      if (!window.__answer) window.__answer = { error: 'port closed before an answer' };
    });
    port.postMessage({ vela: 'intent', intent: buildIntent(), context: {
      chainName: 'Ethereum', signer: { name: 'Vela', letter: 'V' },
    } });
    return true;
  };

  document.getElementById('go').addEventListener('click', window.__openSigner);
})();
