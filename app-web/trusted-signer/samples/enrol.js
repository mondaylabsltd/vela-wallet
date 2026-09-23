// Enrolment for the desktop demo: create a key, then hand the requester its id
// and public key. A separate page from sign.html on purpose.
(function (ns) {
  'use strict';

  var status = document.getElementById('status');
  var hash = new URLSearchParams(location.hash.replace(/^#/, ''));
  var callback = hash.get('cb') ? atob(hash.get('cb').replace(/-/g, '+').replace(/_/g, '/')) : null;
  var token = hash.get('t') || '';

  window.__enrol = function () {
    status.textContent = '等待验证器…';
    // The same create the signing page runs for a wallet's vela_createPasskey
    // (lib/ceremony.js); it records the key on this device as it goes.
    return ns.ceremony.create().then(function (answer) {
      var record = ns.signer._stored() || { credentialId: answer.registration.credentialId, publicKey: null };
      status.textContent = '已创建：' + record.credentialId.slice(0, 12) + '…';
      window.__record = record;
      if (callback) {
        location.href = callback + (callback.indexOf('?') >= 0 ? '&' : '?') +
          't=' + encodeURIComponent(token) +
          '&credentialId=' + encodeURIComponent(record.credentialId) +
          '&publicKey=' + encodeURIComponent(record.publicKey);
      }
      return record;
    }).catch(function (error) {
      status.textContent = '失败：' + (error.message || error);
    });
  };

  document.getElementById('go').addEventListener('click', window.__enrol);
})(window.VelaCS);
