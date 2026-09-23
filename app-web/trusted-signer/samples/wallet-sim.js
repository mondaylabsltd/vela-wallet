// What the web wallet does to run a session with the Trusted Signer in the SAME
// browser (075 §1.5 over 071 §4): open the page once, wait for `ready`, then
// send requests one after another — each answer before the next — and end
// with `bye`. Only answers from the page's origin and from the popup itself
// are taken.
(function () {
  'use strict';

  var params = new URLSearchParams(location.search);
  var SIGNER = params.get('signer') || 'https://getvela.app/src/sign.html?ch=post&lang=en';
  var signerOrigin = new URL(SIGNER).origin;
  var out = document.getElementById('out');
  var child = null;
  var ready = null;
  var pending = {};
  window.__answers = [];

  window.addEventListener('message', function (event) {
    if (event.origin !== signerOrigin || event.source !== child) return;
    var data = event.data || {};
    if (data.vela === 'ready' && ready) ready();
    if ((data.vela === 'result' || data.vela === 'error') && pending[data.id]) {
      window.__answers.push(data);
      out.textContent += JSON.stringify(data).slice(0, 200) + '\n';
      pending[data.id](data);
      delete pending[data.id];
    }
    if (data.vela === 'bye') window.__signerBye = data;
  });

  window.__open = function () {
    return new Promise(function (resolve) {
      ready = resolve;
      child = window.open(SIGNER, 'vela-signer', 'width=460,height=760');
    });
  };

  window.__request = function (id, intent, context) {
    return new Promise(function (resolve) {
      pending[id] = resolve;
      child.postMessage({ vela: 'intent', id: id, intent: intent, context: context || {} }, signerOrigin);
    });
  };

  window.__bye = function () {
    child.postMessage({ vela: 'bye', v: 1 }, signerOrigin);
    return true;
  };
})();
