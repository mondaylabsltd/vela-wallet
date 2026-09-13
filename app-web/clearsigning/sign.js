// The real signing entry. Intent in, signature out.
//
//   intake → resolve → render → (the user drags) → digest → passkey → respond
//
// Two invariants the rest of the app depends on:
//   · the digest is derived here from the same intent the sheet rendered;
//   · closing the page without dragging is a refusal, and the requester is told.
(function (ns) {
  'use strict';

  var t = ns.i18n.t;
  var slot = document.getElementById('sheet-slot');
  var status = document.getElementById('status');
  var current = null;      // the live request, so a close can reject it
  var answered = false;
  // 'refused' (this page's rules said no) and 'user_rejected' (the person
  // said no) are very different facts for whoever asked. Don't blur them.
  var refusalCode = 'user_rejected';

  ns.i18n.setLocale(ns.i18n.detect(new URLSearchParams(location.search).get('lang')));

  function say(key, params) {
    status.textContent = typeof key === 'string' && key.indexOf('.') > 0 ? t(key, params) : key;
  }

  // --- slide to confirm ------------------------------------------------------
  //
  // The only way to accept. There is deliberately no reject button: closing the
  // sheet is the refusal, which is also what happens if the user walks away.

  function attachSlider(element, onConfirm) {
    var knob = element.querySelector('.slide-knob');
    var label = element.querySelector('.slide-label');
    var dragging = false;
    var startX = 0;
    var travel = 0;

    function span() {
      return element.clientWidth - knob.offsetWidth - 12;
    }

    function moveTo(x) {
      travel = Math.max(0, Math.min(span(), x));
      knob.style.transform = 'translateX(' + travel + 'px)';
      label.style.opacity = String(1 - (travel / span()) * 0.9);
    }

    function release() {
      if (!dragging) return;
      dragging = false;
      if (travel / span() >= 0.88) {
        knob.style.transition = 'transform 120ms ease';
        moveTo(span());
        onConfirm();
      } else {
        knob.style.transition = 'transform 220ms cubic-bezier(.2,1.2,.3,1)';
        moveTo(0);
      }
      setTimeout(function () { knob.style.transition = ''; }, 260);
    }

    knob.addEventListener('pointerdown', function (event) {
      if (element.classList.contains('slide-off')) return;
      dragging = true;
      startX = event.clientX - travel;
      knob.setPointerCapture(event.pointerId);
    });
    knob.addEventListener('pointermove', function (event) {
      if (dragging) moveTo(event.clientX - startX);
    });
    knob.addEventListener('pointerup', release);
    knob.addEventListener('pointercancel', release);

    // Keyboard and automation: End, or a programmatic confirm.
    element.tabIndex = 0;
    element.addEventListener('keydown', function (event) {
      if (event.key === 'End' || event.key === 'Enter') onConfirm();
    });
    element.__confirm = onConfirm;
  }

  // --- the flow --------------------------------------------------------------

  function show(request) {
    current = request;
    var context = Object.assign({}, request.context, { originVerified: request.originVerified });

    // resolve() takes it from here: when the request carries an assembled
    // operation it renders THAT operation's calldata, derives the fee from the
    // same legs, and refuses if the site's call is not among them.
    var view = ns.resolve(request.intent, context);

    // This device's own record wins when it has one; otherwise the requester's
    // name for the account stands, because its job is to point at a passkey.
    view.accountName = ns.signer.knownName(context.allowCredentials || []) || view.accountName;

    // The digest is derived from the same bytes the sheet rendered — never
    // taken from the request. If we cannot derive it, we refuse instead of
    // signing bytes whose meaning we could not check.
    var digest = ns.digest.of(request.intent, context);
    if (digest.refuse) {
      view.refuse = true;
      view.warnings.push({ tone: 'danger', key: digest.refuse });
    }
    if (digest.warn) view.warnings.push({ tone: 'danger', key: digest.warn });
    if (digest.hash) {
      view.digest = {
        hex: ns.digest.toHex(digest.hash),
        kind: digest.describes,
        unwrapped: !!digest.unwrapped,
      };
    }

    var sheet = ns.render(view, {});
    slot.innerHTML = '';
    slot.appendChild(sheet);

    if (request.comparisonCode) {
      var code = document.createElement('div');
      code.className = 'pairing-code';
      code.innerHTML = '<span>' + t('ui.comparisonCode') + '</span><b>' + request.comparisonCode + '</b>';
      sheet.insertBefore(code, sheet.children[1]);
    }

    var slider = sheet.querySelector('.slide');
    if (view.refuse) {
      slider.classList.add('slide-off');
      refusalCode = 'refused';
      say('ui.cannotSign');
      window.__refused = true;
      return;
    }
    attachSlider(slider, function () { confirm(request, digest, slider, context); });
    say('ui.dragToSign');
    window.__slider = slider; // automation hook, harmless in production
  }

  function confirm(request, digest, slider, context) {
    if (answered) return;
    slider.classList.add('slide-off');
    say('ui.waitingAuthenticator');

    // The account's own key set, as the requester declared it. Signing never
    // creates a key: if none of these is on this device, the answer is "sign
    // it somewhere else", not "here, make a new account".
    var allowed = context.allowCredentials || [];

    ns.signer.sign(digest.hash, { allowCredentials: allowed })
      .then(function (assertion) {
        // Whichever key answered must be one the account actually owns —
        // otherwise this is a valid signature for somebody else's wallet.
        if (allowed.length && allowed.indexOf(assertion.credentialId) < 0) {
          throw new Error(t('ui.wrongCredential'));
        }
        answered = true;
        assertion.digest = ns.digest.toHex(digest.hash);
        assertion.digestKind = digest.describes;
        return request.respond(assertion).then(function () {
          say('ui.signed');
          window.__result = assertion;
        });
      })
      .catch(function (error) {
        slider.classList.remove('slide-off');
        if (error && error.name === 'NotAllowedError') {
          // Cancelled, timed out, or this device simply holds no key for the
          // account. WebAuthn deliberately does not say which.
          say('ui.ceremonyFailed');
          return;
        }
        say(String((error && error.message) || error));
      });
  }

  // Walking away is a refusal, and the requester deserves to hear it rather
  // than hang forever.
  window.addEventListener('pagehide', function () {
    if (!answered && current) {
      answered = true;
      // `true` = we are unloading, so the answer must go out by beacon.
      try { current.reject(refusalCode, true); } catch (e) { /* best effort */ }
    }
  });

  say('ui.waitingRequest');
  ns.intake
    .receive({ onCode: function (code) { say('ui.comparisonCode', { code: code }); } })
    .then(show)
    .catch(function (error) {
      say(String(error.message || error));
    });
})(window.VelaCS);
