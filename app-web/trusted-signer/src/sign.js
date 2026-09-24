// The real entry. Requests in, answers out, one session at a time.
//
//   intake (a session) → next request → resolve → render → (the person slides)
//     · a signing intent: digest → passkey assertion → respond
//     · a key ceremony:   the page's own challenge → create / assertion → respond
//   → back to "waiting for the wallet" → next request … until bye / close / idle
//
// Invariants the rest of the app depends on:
//   · what gets signed is derived here from the same request the sheet
//     rendered — a digest (lib/digest.js) or a ceremony challenge
//     (lib/ceremony.js) — never bytes the requester supplied;
//   · a key is created only for a `vela_createPasskey` request that
//     resolve.js let through (a Vela wallet asking), on its own card;
//   · closing the page with a request unanswered refuses it, and the
//     requester is told.
(function (ns) {
  'use strict';

  var t = ns.i18n.t;
  var slot = document.getElementById('sheet-slot');
  var status = document.getElementById('status');
  var session = null;
  var current = null;      // the request on screen, until answered
  var waitingState = null; // what the waiting card says between requests
  var lastGone = false;    // the request on screen lost its wallet

  ns.i18n.setLocale(ns.i18n.detect(new URLSearchParams(location.search).get('lang')));

  function say(key, params) {
    status.textContent = typeof key === 'string' && key.indexOf('.') > 0 ? t(key, params) : key;
  }

  // Automation hooks, harmless in production: tests read where the page is.
  var state = window.__velaState = { phase: 'starting', received: 0, answered: 0, kind: null, endReason: null };
  function phase(name, extra) {
    state.phase = name;
    if (extra) Object.assign(state, extra);
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

  function draw(sheet) {
    slot.innerHTML = '';
    slot.appendChild(sheet);
  }

  function ready(slider, onConfirm, sayKey) {
    attachSlider(slider, onConfirm);
    say(sayKey || 'ui.dragToSign');
    window.__slider = slider;
    phase('card');
  }

  // --- between requests --------------------------------------------------------

  function waiting(update) {
    waitingState = Object.assign({}, waitingState || {}, update || {});
    // Who is on the other end, and whether anything vouches for it. The
    // the postMessage channel is verified by the browser itself;
    // a loopback socket and a URL fragment are not.
    waitingState.requesterApp = (session && session.requesterApp) || '';
    waitingState.requesterIcon = (session && session.requesterIcon) || '';
    // Vela's mark only where something vouches for the other end. On the url
    // channel that is the callback's scheme: the answer goes to a Vela wallet
    // and to nothing else, which is a fact about this page's own behaviour
    // rather than a name the requester handed over.
    waitingState.requesterVerified = !!(session && session.channel === 'post')
      || !!(session && session.channel === 'ext')
      || !!(session && session.channel === 'url' && ns.resolve.answersToWallet({ callback: session.callback }));
    window.__slider = null;
    draw(ns.render.waiting(waitingState));
    phase('waiting');
  }

  function channelLine() {
    return session && { ws: 'value.viaApp' }[session.channel];
  }

  // A request was answered. A session that carries more goes back to a calm
  // "waiting for the wallet"; the answer's own words stay on the status line.
  function answered(doneKey) {
    current = null;
    state.answered = session.answered;
    say(doneKey);
    if (session.persistent && !session.ended) {
      waiting({ titleKey: 'ui.waitingForWallet', noteKey: 'ui.waitingNote', originKey: channelLine() });
    } else {
      phase('answered');
    }
    loop();
  }

  // This page's rules said no. On a session the requester hears it at once and
  // the reasons stay on screen until the next request; on a one-shot channel
  // closing the page sends it, as it always has.
  function refused(request, code) {
    var slider = slot.querySelector('.slide');
    if (slider) slider.classList.add('slide-off');
    request.refusalCode = 'refused';
    window.__refused = true;
    window.__slider = null;
    // On a session the wallet is told at once; elsewhere, closing tells it.
    say(session.persistent ? 'ui.refusedSent' : 'ui.cannotSign');
    phase('refused');
    if (session.persistent) {
      request.reject(code || 'refused');
      current = null;
      state.answered = session.answered;
      loop();
    }
  }

  // --- a signing intent (071) ----------------------------------------------------

  function showSigning(request, view, context) {
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
    draw(sheet);
    var slider = sheet.querySelector('.slide');
    if (view.refuse) {
      refused(request);
      return;
    }
    ready(slider, function () { confirmSigning(request, digest, slider, context); });
  }

  function confirmSigning(request, digest, slider, context) {
    if (request.answered || request.busy) return;
    request.busy = true;
    slider.classList.add('slide-off');
    say('ui.waitingAuthenticator');
    phase('busy');

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
        if (request.gone) return null;
        assertion.digest = ns.digest.toHex(digest.hash);
        assertion.digestKind = digest.describes;
        window.__result = assertion;
        return request.respond({ result: assertion }).then(function () { answered('ui.signed'); });
      })
      .catch(function (error) {
        request.busy = false;
        if (request.gone) return;
        slider.classList.remove('slide-off');
        phase('card');
        if (error && error.name === 'NotAllowedError') {
          // Cancelled, timed out, or this device simply holds no key for the
          // account. WebAuthn deliberately does not say which.
          say('ui.ceremonyFailed');
          return;
        }
        say(String((error && error.message) || error));
      });
  }

  // --- a key ceremony (075) ------------------------------------------------------

  function showCeremony(request, view, context) {
    var c = view.ceremony;
    var challenge = null; // bytes this page derived, for sign-in / proof / member proof

    if (!view.refuse && c.kind === 'signIn') {
      var signIn = ns.ceremony.signInChallenge();
      view.challenge = { text: signIn, noteKey: 'ui.challengeFromClock' };
      challenge = ns.ceremony._utf8(signIn);
    } else if (!view.refuse && c.kind === 'proof') {
      var proof = ns.ceremony.proofChallenge(c.purpose);
      view.challenge = { text: proof, noteKey: 'ui.challengeFromClock' };
      challenge = ns.ceremony._utf8(proof);
    } else if (!view.refuse && c.kind === 'memberProof') {
      view.challenge = { pending: true };
    }

    var sheet = ns.render(view, {});
    draw(sheet);
    if (view.refuse) {
      refused(request);
      return;
    }

    if (c.kind !== 'memberProof') {
      arm(request, view, sheet.querySelector('.slide'), challenge);
      return;
    }

    // The member challenge: this page's own fetch, for the inputs on screen,
    // checked against the page's own computation before anything is signable.
    sheet.querySelector('.slide').classList.add('slide-off');
    say('ui.fetchingChallenge');
    phase('fetching');
    ns.ceremony.fetchMemberChallenge(c.member, context.rpId).then(function (fetched) {
      if (current !== request || request.gone) return;
      view.challenge = { text: '0x' + ns.ceremony._hex(fetched.challenge), noteKey: 'ui.challengeFromRegistry' };
      var redrawn = ns.render(view, {});
      draw(redrawn);
      arm(request, view, redrawn.querySelector('.slide'), fetched.challenge);
    }, function (error) {
      if (current !== request || request.gone) return;
      view.refuse = true;
      view.risk = 'danger';
      view.challenge = null;
      view.warnings.push({ tone: 'danger', key: error.refusal || 'refuse.registryUnavailable' });
      draw(ns.render(view, {}));
      refused(request, error.code || 'refused');
    });
  }

  function arm(request, view, slider, challenge) {
    var c = view.ceremony;
    ready(slider, function () { confirmCeremony(request, c, slider, challenge); },
      c.kind === 'create' ? 'ui.slideCreate' : 'ui.dragToSign');
  }

  function confirmCeremony(request, c, slider, challenge) {
    if (request.answered || request.busy) return;
    request.busy = true;
    slider.classList.add('slide-off');
    say('ui.waitingAuthenticator');
    phase('busy');

    var work;
    if (c.kind === 'create') {
      work = ns.ceremony.create({ name: c.name, excludeCredentialIds: c.excludeCredentialIds });
    } else {
      var allow = c.kind === 'signIn' ? [] : c.credentialId ? [c.credentialId] : [];
      work = ns.ceremony.assert(challenge, allow);
    }

    work.then(function (payload) {
      if (request.gone) return null;
      window.__result = payload;
      return request.respond(payload).then(function () {
        answered(c.kind === 'create' ? 'ui.created' : 'ui.ceremonyDone');
      });
    }).catch(function (error) {
      request.busy = false;
      if (request.gone) return;
      slider.classList.remove('slide-off');
      phase('card');
      if (error && error.wrongCredential) say('ui.wrongCredential');
      else if (error && error.name === 'InvalidStateError') say('ui.keyExists');
      else if (error && error.name === 'NotAllowedError') say(c.kind === 'create' ? 'ui.createFailed' : 'ui.proofFailed');
      else say(String((error && error.message) || error));
    });
  }

  // --- the loop ------------------------------------------------------------------

  function show(request) {
    current = request;
    lastGone = false;
    window.__refused = false;
    window.__slider = null;
    state.received = session.received;
    // Facts from the channel override anything the requester put in its
    // context under the same names: who is asking is not the asker's to say.
    var context = Object.assign({}, request.context, {
      originVerified: request.originVerified,
      channel: request.channel,
      requester: request.requester,
      // The requester's own name for itself, where the channel carried one
      // (the handshake's hello). A CLAIM, and drawn as one: this page is a
      // signer anything can connect to, so painting Vela's mark on whatever
      // dialled in would be the page vouching for something it cannot check.
      requesterApp: session.requesterApp || '',
      requesterIcon: session.requesterIcon || '',
      // Where the answer will go. The channel's, never the requester's: it is
      // what this page will DO, and on the url channel it is the only thing
      // the page can say about the other end truthfully.
      callback: session.callback || '',
      rpId: ns.signer.relyingPartyId(),
    });
    var view = ns.resolve(request.intent, context);
    state.kind = view.kind === 'ceremony' ? view.ceremony.kind : 'sign';
    if (view.kind === 'ceremony') showCeremony(request, view, context);
    else showSigning(request, view, context);
  }

  function loop() {
    session.next().then(function (request) {
      if (request) show(request);
      else ended();
    });
  }

  // The session is over. Say why, in the page's own words.
  function ended() {
    state.endReason = session.endReason;
    window.__slider = null;
    if (session.endReason === 'single') return; // a one-shot channel: its answer's words stand
    try { closing(session.endReason); } finally { phase('ended'); }
  }

  function closing(reason) {
    if (reason === 'error') {
      say(session.endDetail || 'ui.walletGone');
      if (session.persistent) {
        waiting({ titleKey: 'ui.sessionEnded', noteKey: session.endDetail || 'ui.walletGone' });
        slot.firstChild.classList.add('sheet-ended');
      }
      return;
    }
    if (reason === 'idle') {
      say('ui.sessionIdle');
      waiting({ titleKey: 'ui.sessionEnded', noteKey: 'ui.sessionIdle' });
      slot.firstChild.classList.add('sheet-ended');
      return;
    }
    // bye, or the channel closed. A card whose wallet went away, or a
    // refusal, keeps its reasons on screen.
    if (lastGone) {
      say('ui.walletGone');
      return;
    }
    if (window.__refused) {
      say('ui.sessionOver');
      return;
    }
    if (session.answered) {
      say('ui.sessionDone');
      waiting({ titleKey: 'ui.sessionEnded', noteKey: 'ui.sessionDone' });
      slot.firstChild.classList.add('sheet-ended');
      return;
    }
    // Nothing was answered: the wallet stopped waiting (or never came).
    if (!slot.querySelector('.sheet') || slot.querySelector('.sheet-waiting')) say('ui.walletGone');
  }

  // Walking away is a refusal, and the requester deserves to hear it rather
  // than hang forever.
  window.addEventListener('pagehide', function () {
    if (current && !current.answered && !current.gone) {
      // `true` = we are unloading, so the answer must go out by beacon.
      try { current.reject(current.refusalCode || 'user_rejected', true); } catch (e) { /* best effort */ }
    }
  });

  say('ui.waitingRequest');

  /**
   * Start the session — but BLE first asks the person to press something.
   *
   * `navigator.bluetooth.requestDevice` refuses without user activation, so a
   * page that calls it on load cannot ever pair: the first radio pass (spec
   * 075 T043) found this page showing Chrome's own exception text where the
   * device chooser should have been. The button IS the gesture; nothing else
   * on this page needs one.
   */
  function startSession() {
    ns.intake
      .open({
      // The loopback socket is slow to open only when the browser is asking
      // the person first (Local Network Access).
      onWaiting: function () { say('ui.waitingWallet'); },
      // The wallet stopped waiting for the request on screen: signing now
      // would sign into nothing, so the slider goes.
      onGone: function (request) {
        if (request !== current) return;
        current = null;
        lastGone = true;
        var slider = slot.querySelector('.slide');
        if (slider) slider.classList.add('slide-off');
        window.__slider = null;
        say('ui.walletGone');
        phase('gone');
        // The session may carry on; listen again. `lose()` runs before a
        // session is marked ended, so wait a tick.
        setTimeout(loop, 0);
      },
    })
      .then(function (opened) {
        session = opened;
        state.channel = opened.channel;
        loop();
      })
      .catch(function (error) {
        say(String(error.message || error));
      });
  }

  startSession();
})(window.VelaCS);
