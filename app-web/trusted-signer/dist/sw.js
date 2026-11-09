// The signing page's pin (spec 076 FR-008, §38 — review 0-5).
//
// A wallet asks for a page by its content address, `b/<sha256>/sign`, and the
// wallet's own check hashes what the host served it. A browser opening that
// address asks the host AGAIN, and a host that serves the check one thing and
// the navigation another is not caught by any check outside the browser.
//
// This worker closes that for every visit after the first. Registered by the
// page itself (`lib/place.js`) with the deployment's `b/` as its scope, it
// answers every navigation to a version:
//
//   · from its own cache, when it has verified that version before;
//   · otherwise it fetches the bytes, hashes them, and serves them ONLY when
//     the sha256 is the one in the address — the version the wallet asked for.
//     Anything else is a refusal page that signs nothing.
//
// So once a browser has met the page, the bytes it runs at `b/<h>/` are `h`,
// whatever the host does later — the content address is the pin, no list is
// needed here, and the wallet's own allow-set (`BUILD_ALLOWED`) decides which
// `h` it asks for.
//
// What it does NOT do (docs/security/hardening.md §38): the first visit is
// trust on first use; and the browser updates this worker from the host (a
// changed `sw.js` replaces this one on a later visit), so a host that serves a
// hostile worker defeats the pin from the visit after that. A browser that
// keeps no worker (a private window, an in-app browser without them, storage
// cleared) is unpinned. The web wallet does not depend on this: it opens the
// page from the bytes it hashed.
'use strict';

var CACHE = 'vela-signing-pages-v1';
var VERSION_PATH = /\/b\/([0-9a-f]{64})\/sign(?:\.html)?$/;

self.addEventListener('install', function () {
  self.skipWaiting();
});

// The version that registered this worker was served before the worker
// existed: pin it now — fetched again and kept only if it hashes to its
// address — so the next navigation to it is answered from here.
self.addEventListener('activate', function (event) {
  event.waitUntil(
    self.clients.claim().then(function () {
      return self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    }).then(function (windows) {
      return Promise.all(windows.map(function (client) {
        var url = new URL(client.url);
        var match = url.origin === self.location.origin && VERSION_PATH.exec(url.pathname);
        return match ? pinned(url.origin + url.pathname, match[1]).catch(function () {}) : null;
      }));
    })
  );
});

self.addEventListener('fetch', function (event) {
  var request = event.request;
  if (request.method !== 'GET') return;
  var url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  var match = VERSION_PATH.exec(url.pathname);
  if (!match) return;
  event.respondWith(pinned(url.origin + url.pathname, match[1]));
});

function hex(buffer) {
  var bytes = new Uint8Array(buffer);
  var out = '';
  for (var i = 0; i < bytes.length; i++) out += bytes[i].toString(16).padStart(2, '0');
  return out;
}

// A version, as the page is served: never inside another page, whatever the
// host's own headers say.
function headersFor(type) {
  return {
    'content-type': type || 'text/html; charset=utf-8',
    'content-security-policy': "frame-ancestors 'none'",
    'x-frame-options': 'DENY',
    'cache-control': 'no-transform',
  };
}

function refusal(expected, served) {
  var body =
    '<!doctype html><html lang="en"><head><meta charset="utf-8">' +
    '<meta name="viewport" content="width=device-width, initial-scale=1"><title>Vela</title></head>' +
    '<body style="font-family: system-ui, sans-serif; max-width: 34rem; margin: 3rem auto; padding: 0 1rem">' +
    '<h1 style="font-size: 1.25rem">This signing page was not opened</h1>' +
    '<p>The bytes served at this address are not the version it names, so nothing here can sign.</p>' +
    '<p style="font-family: ui-monospace, monospace; font-size: .8rem; word-break: break-all">' +
    'expected ' + expected + '<br>served ' + served + '</p></body></html>';
  var headers = headersFor('text/html; charset=utf-8');
  headers['content-security-policy'] = "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'";
  return new Response(body, { status: 409, headers: headers });
}

async function pinned(key, version) {
  var cache = await caches.open(CACHE);
  var kept = await cache.match(key);
  if (kept) return kept;
  var response;
  try {
    // The host's redirects are followed (`sign.html` → `sign`); the answer is
    // rebuilt below, so a navigation never receives a redirected response.
    response = await fetch(key, { cache: 'no-store', credentials: 'omit', redirect: 'follow' });
  } catch (e) {
    return Response.error();
  }
  if (!response.ok) return response;
  var bytes = await response.arrayBuffer();
  var served = hex(await crypto.subtle.digest('SHA-256', bytes));
  if (served !== version) return refusal(version, served);
  var page = new Response(bytes, { status: 200, headers: headersFor(response.headers.get('content-type')) });
  await cache.put(key, page.clone());
  return page;
}
