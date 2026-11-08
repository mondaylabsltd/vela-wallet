// Where this page is — read one way whether it was opened at its own address
// or from the bytes a wallet checked (§38, review 0-5).
//
// The web wallet opens the page FROM THE BYTES ITS CHECK HASHED: it fetches
// `/b/<sha256>/sign`, hashes it, and opens those very bytes as a `blob:` URL,
// so the host has no second request in which to serve something else. A blob
// document has the WALLET's origin (`self.origin`, the browser's fact) and no
// host or path of its own, so what the page used to read from its address
// comes from two other places:
//
//   · its host — the origin's, for the passkey's relying party and the
//     loopback rules (`signer.js`, `resolve.js`). From `wallet.getvela.app`
//     that is the same `getvela.app` the published page folds to; the wallet
//     opens a page this way only when its own origin can use the account's
//     keys (`launch::CheckedPage::opens_from_bytes`, the core's rule);
//   · its query — the fragment (`#ch=post&lang=…&b=<sha256>`), because a blob
//     URL carries no query. `b=` is the version the wallet hashed: a label,
//     like the path it replaces, never a proof (`sign.js`, the trust line).
//
// Nothing else changes: the CSP inside these bytes governs a blob document as
// it governs the page at its address, and the page still answers only the
// wallet origins the browser reports for its opener.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var VERSION = /^[0-9a-f]{64}$/;
  var IN_PATH = /\/b\/([0-9a-f]{64})\//;

  function fromBytes() {
    return typeof location === 'object' && !!location && location.protocol === 'blob:';
  }

  function originHost() {
    try {
      return new URL(self.origin).hostname;
    } catch (e) {
      return '';
    }
  }

  ns.place = {
    /** Opened from the bytes a wallet checked, not at an address of its own. */
    fromBytes: fromBytes,
    /** This page's host: the address's, or the origin's when there is none. */
    host: function () {
      if (typeof location !== 'object' || !location) return '';
      return fromBytes() ? originHost() : String(location.hostname || '');
    },
    /** The page's parameters: the query, or the fragment of a blob URL. */
    params: function () {
      if (typeof location !== 'object' || !location) return new URLSearchParams('');
      return new URLSearchParams(fromBytes() ? location.hash.replace(/^#/, '') : location.search);
    },
    /** The full version this page was opened as (sha256 hex), or null. */
    version: function () {
      if (typeof location !== 'object' || !location) return null;
      if (fromBytes()) {
        var named = ns.place.params().get('b');
        return named && VERSION.test(named) ? named : null;
      }
      var match = IN_PATH.exec(location.pathname || '');
      return match ? match[1] : null;
    },
  };

  // The page pins itself (spec 076 FR-008): a Service Worker scoped to the
  // deployment's `b/` serves every version from the bytes it verified against
  // the version's own address, so after the first visit the host cannot hand
  // a navigation other bytes (`sw.js`). Only a page at its own content address
  // registers it — never a blob (that would be the wallet's origin), never
  // the unversioned root copy.
  (function pin() {
    if (fromBytes() || typeof navigator !== 'object' || !navigator.serviceWorker) return;
    var path = typeof location === 'object' && location ? location.pathname || '' : '';
    var at = path.search(IN_PATH);
    if (at < 0) return;
    var base = path.slice(0, at + 1);
    try {
      navigator.serviceWorker.register(base + 'sw.js', { scope: base + 'b/' }).catch(function () {});
    } catch (e) { /* no worker here: the page works as before, unpinned */ }
  })();
})(window.VelaCS);
