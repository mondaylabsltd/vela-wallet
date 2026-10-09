// TEST ONLY: for `samples/loopback-sign.html`, the desktop demo's page.
//
// The published page answers to the Vela wallet's own address and nowhere else
// (spec 102 R7, `resolve.answersToWallet`). `desktop-demo.mjs` plays a native
// app on this machine, and in its manual mode the page opens in the person's
// default browser, where a `velawallet://` answer would go to an installed
// Vela (or nowhere) and never back to the demo. So the demo listens on this
// machine's loopback, and this file lets the harness page answer there too.
//
// Why this cannot reach a published page:
//   · it lives outside `src/`, and `build-single.mjs` builds the published
//     page from exactly what `src/sign.html` names, so `dist/` never holds it;
//   · `samples/answer-test.mjs` checks it: the published build refuses a
//     loopback answer address, its bytes do not contain this file, and the
//     harness page names `src/sign.html`'s scripts plus this one and nothing
//     else.
//
// It widens the rule, never narrows it: whatever the wallet's address is
// allowed, plus `http://127.0.0.1:<port>/…` or `http://localhost:<port>/…`,
// on the URL channel only.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var LOOPBACK = /^http:\/\/(127\.0\.0\.1|localhost):\d{1,5}\/[^?#]*$/;
  var strict = ns.resolve.answersToWallet;

  ns.resolve.answersToWallet = function (ctx) {
    return strict(ctx) ||
      (!!ctx && ctx.channel === 'url' && typeof ctx.callback === 'string' && LOOPBACK.test(ctx.callback));
  };
})(window.VelaCS);
