// The real entry. Requests in, answers out, one session at a time.
//
//   intake (a session) → next request → resolve → render → (the person taps)
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
  document.title = t('ui.pageName');

  /**
   * What this page is, in one calm line under everything (spec 102): its
   * version, and how it is built — zero dependencies, open source, and
   * yours to host.
   *
   * The version is the content hash in this page's own address
   * (`/b/<sha256>/`), shortened to the 8 characters the wallet's hand-off card
   * shows, so a person can see the two agree. It is read from the address the
   * browser opened — which is what the wallet checked before opening it — and
   * it is a label, not a proof: a page cannot vouch for its own bytes (076).
   * The wallet's check is the proof; this line says which page that was.
   */
  function versionOf(path) {
    var match = /\/b\/([0-9a-f]{64})\//.exec(path || '');
    return match ? match[1].slice(0, 8) : null;
  }

  (function trustLine() {
    var line = document.getElementById('trust');
    if (!line) return;
    var version = versionOf(location.pathname);
    var parts = [
      t('ui.pageName'),
      version ? t('ui.version', { version: version }) : null,
      t('ui.pageFacts'),
    ].filter(Boolean).join(' · ').split(' · ');
    // Each phrase whole: a narrow screen wraps between them, never inside.
    parts.forEach(function (part, i) {
      if (i) line.appendChild(document.createTextNode(' · '));
      var span = document.createElement('span');
      span.className = 'trust-part';
      span.textContent = part;
      line.appendChild(span);
    });
  })();

  function say(key, params) {
    status.textContent = typeof key === 'string' && key.indexOf('.') > 0 ? t(key, params) : key;
  }

  // Automation hooks, harmless in production: tests read where the page is.
  var state = window.__velaState = { phase: 'starting', received: 0, answered: 0, kind: null, endReason: null };
  function phase(name, extra) {
    state.phase = name;
    if (extra) Object.assign(state, extra);
  }

  // --- tap to confirm ---------------------------------------------------------
  //
  // The only way to accept: one button (spec 102 — every confirmation in Vela
  // is a tap). There is deliberately no reject button: closing the page is the
  // refusal, which is also what happens if the person walks away.
  //
  // `onConfirm` runs INSIDE the click handler and stays synchronous all the
  // way to navigator.credentials.get / create: Safari grants the passkey
  // prompt only to code running in the person's own gesture, and an await or
  // a timer in between would lose it. A second tap while the prompt is up does
  // nothing (`busy`, and each request's own `busy` flag).
  //
  // `window.__slider` / `__confirm` / `__reset` keep their names: the desktop
  // e2e and every suite here drive the page through them.

  function arm(button, onConfirm) {
    button.addEventListener('click', function () {
      if (button.disabled || button.classList.contains('busy')) return;
      onConfirm();
    });
    button.__confirm = onConfirm;
    button.__reset = function () { idle(button); };
  }

  // Waiting for the passkey: the button stays the colour it was — busy must
  // never read as "disabled" — and stops taking taps.
  function busy(button) {
    button.classList.add('busy');
    button.setAttribute('aria-busy', 'true');
  }

  function idle(button) {
    button.classList.remove('busy');
    button.removeAttribute('aria-busy');
  }

  function off(button, labelKey) {
    if (!button) return;
    idle(button);
    button.disabled = true;
    button.classList.add('confirm-off');
    if (labelKey) button.textContent = t(labelKey);
    var hint = button.parentNode && button.parentNode.querySelector('.confirm-hint');
    if (hint) hint.remove();
  }

  function done(button) {
    off(button, 'button.done');
    button.classList.remove('confirm-off');
    button.classList.add('confirm-done');
  }

  function draw(sheet) {
    slot.innerHTML = '';
    slot.appendChild(sheet);
  }

  function ready(button, onConfirm) {
    arm(button, onConfirm);
    // The hint is on the card, under the button; the status line is for what
    // happens next.
    say('');
    window.__slider = button;
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
      || !!(session && ns.resolve.answersToWallet({ channel: session.channel, callback: session.callback }));
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
    off(slot.querySelector('.confirm'), 'button.cannotSign');
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
      view.confirmKey = 'button.cannotSign';
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
    var button = sheet.querySelector('.confirm');
    if (view.refuse) {
      refused(request);
      return;
    }
    ready(button, function () { confirmSigning(request, digest, button, context, view.key); });
  }

  function confirmSigning(request, digest, button, context, key) {
    if (request.answered || request.busy) return;
    // Spec 102 R7 once more, at the last moment before a passkey prompt: the
    // card refused already if the answer would not reach the wallet, so this
    // is unreachable unless that refusal is ever lost on the way here.
    if (!ns.resolve.answersToWallet(context)) {
      refused(request);
      return;
    }
    request.busy = true;
    busy(button);
    say('ui.waitingAuthenticator');
    phase('busy');

    // The account's own key set, as the requester declared it — or the one
    // key of it the wallet signs with, when it said which (spec 102 R5;
    // resolve.js checked that key is one of them). Signing never creates a
    // key: if none of these is on this device, the answer is "sign it
    // somewhere else", not "here, make a new account".
    var allowed = key && key.credentialId ? [key.credentialId] : (context.allowCredentials || []);

    ns.signer.sign(digest.hash, { allowCredentials: allowed, key: key })
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
        return request.respond({ result: assertion }).then(function () {
          done(button);
          answered('ui.signed');
        });
      })
      .catch(function (error) {
        request.busy = false;
        if (request.gone) return;
        idle(button);
        phase('card');
        if (error && (error.name === 'NotAllowedError' || error.name === 'AbortError')) {
          // Cancelled, timed out, or this device simply holds no key for the
          // account. WebAuthn deliberately does not say which — so the words
          // cover all three in everyday language (spec 079: no "仪式").
          say('ui.ceremonyFailed');
          return;
        }
        // Anything else: the same everyday sentence first; the engine's own
        // words after it, for whoever reports it.
        say(t('ui.notSigned') + ' (' + String((error && error.message) || error) + ')');
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
      // The member challenge, computed HERE from the facts on screen — the
      // chain, the registry contract, the relying party, the key and its
      // binding. No fetch: the published page reaches no network at all
      // (076), and the page never signed a challenge it was handed anyway.
      var computed = null;
      try {
        computed = ns.ceremony.memberChallengeFor(c.member, context.rpId);
      } catch (error) {
        computed = null;
      }
      if (computed) {
        view.challenge = {
          text: '0x' + ns.ceremony._hex(computed.challenge),
          noteKey: 'ui.challengeComputedHere',
        };
        challenge = computed.challenge;
      } else {
        view.refuse = true;
        view.risk = 'danger';
        view.warnings.push({ tone: 'danger', key: 'refuse.noDeployment' });
        view.confirmKey = 'button.cannotSign';
      }
    }

    var sheet = ns.render(view, {});
    draw(sheet);
    if (view.refuse) {
      refused(request, 'refused');
      return;
    }
    var button = sheet.querySelector('.confirm');
    ready(button, function () { confirmCeremony(request, c, button, challenge, view.key); });
  }

  function confirmCeremony(request, c, button, challenge, key) {
    if (request.answered || request.busy) return;
    request.busy = true;
    busy(button);
    say('ui.waitingAuthenticator');
    phase('busy');

    // Where the key is, or where the new one goes (spec 102 R5), so the
    // browser asks for that place only. Synchronous to the WebAuthn call.
    var work;
    if (c.kind === 'create') {
      work = ns.ceremony.create({ name: c.name, excludeCredentialIds: c.excludeCredentialIds, key: key });
    } else {
      var allow = c.kind === 'signIn' ? [] : c.credentialId ? [c.credentialId] : [];
      work = ns.ceremony.assert(challenge, allow, key);
    }

    work.then(function (payload) {
      if (request.gone) return null;
      window.__result = payload;
      return request.respond(payload).then(function () {
        done(button);
        answered(c.kind === 'create' ? 'ui.created' : 'ui.ceremonyDone');
      });
    }).catch(function (error) {
      request.busy = false;
      if (request.gone) return;
      idle(button);
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
      // would sign into nothing, so the button goes.
      onGone: function (request) {
        if (request !== current) return;
        current = null;
        lastGone = true;
        off(slot.querySelector('.confirm'));
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
        var message = String((error && error.message) || error);
        // A reload (or a link opened twice) finds no request in the address —
        // the fragment is wiped once read. Say what that means (spec 079).
        say(/found no request/.test(message) ? 'ui.requestEnded' : message);
      });
  }

  /**
   * Inside another page this page does nothing at all (spec 102 R7).
   *
   * A frame lets the page around it draw over this one — hide the card, cover
   * it with its own words, lay a "continue" exactly where the button is — while
   * the passkey prompt still says Vela's domain. The host forbids framing
   * (`frame-ancestors 'none'` and `X-Frame-Options: DENY` in `dist/_headers`);
   * this covers a host that does not send those headers, and a browser that
   * ignores them. Checked before the session opens, so the request in the
   * address is never read, shown or answered.
   */
  function framed() {
    try {
      return window.top !== window.self;
    } catch (e) {
      return true;
    }
  }

  if (framed()) {
    say('ui.framed');
    phase('framed');
  } else {
    startSession();
  }
})(window.VelaCS);
