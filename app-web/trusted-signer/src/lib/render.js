// View model → DOM. Nothing here decides anything: every judgement was made in
// resolve.js and every word comes from lib/locales/. Keeping the decisions out
// of the renderer is what makes "what you read is what gets signed" checkable
// by reading one file.
//
// There is also nothing here that can EDIT the request. No amount editor, no
// fee-token picker. The intent arrives fixed; the only two outcomes are sign
// (tap) and refuse (close).
//
// The order is the page's argument (spec 102): WHAT will be signed first — the
// intent, the amount, the plain sentence, the facts behind it — then WHICH KEY
// signs it and where that key lives, then the one button.
//
// And it is drawn as the apps draw their own signing sheet (owner,
// 2026-10-09: 「要维持和 app 内签名的视觉感受」): the title row, the blocks,
// then a hairline and the app's footer — the folded technical details, the
// fee, the account, the button (SigningBody). Same parts, same order, same
// tokens (sheet.css), so the page reads as the sheet a person already knows.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  function t(key, params) {
    return ns.i18n.t(key, params);
  }

  function el(tag, className, textContent) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    if (textContent !== undefined && textContent !== null) node.textContent = textContent;
    return node;
  }

  // Line icons, drawn as the app's are (lucide paths, stroke 2): nothing here
  // can be fetched, and a glyph character would be drawn by whatever font has it.
  var ICONS = {
    'chevron-right': '<path d="m9 18 6-6-6-6"/>',
    'chevron-down': '<path d="m6 9 6 6 6-6"/>',
    'triangle-alert': '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
    check: '<path d="M20 6 9 17l-5-5"/>',
  };

  function icon(name, className) {
    var node = el('span', 'icon' + (className ? ' ' + className : ''));
    node.setAttribute('aria-hidden', 'true');
    node.innerHTML = '<svg viewBox="0 0 24 24">' + (ICONS[name] || '') + '</svg>';
    return node;
  }

  function avatar(letter, tone) {
    var node = el('span', 'avatar', letter);
    if (tone) node.style.setProperty('--tone', tone);
    return node;
  }

  // Locally computed from the address; a requester cannot influence it, which
  // is exactly why it is shown instead of a name a requester supplied.
  function identicon(svg, size) {
    var node = el('span', 'identicon');
    node.style.width = size + 'px';
    node.style.height = size + 'px';
    node.innerHTML = svg;
    return node;
  }

  // A logo the request asked us to show. Cosmetic only, sandboxed to an <img>,
  // and it silently disappears if it fails to load or is not an https URL.
  function remoteLogo(url, letter, tone) {
    // An inline mark a peer sent for itself is allowed (the transport has
    // already refused anything but a small raster `data:` URI); a remote URL
    // is only ever this page's own catalogue.
    var inline = typeof url === 'string' && url.indexOf('data:image/') === 0;
    if (!url || !(inline || /^https:\/\//.test(url))) return avatar(letter, tone);
    var wrap = el('span', 'avatar avatar-logo');
    if (tone) wrap.style.setProperty('--tone', tone);
    var img = document.createElement('img');
    img.alt = '';
    img.referrerPolicy = 'no-referrer';
    img.addEventListener('error', function () { wrap.textContent = letter; });
    img.src = url;
    wrap.appendChild(img);
    return wrap;
  }

  function tokenChip(token, urls) {
    var chip = el('span', 'token-chip', (token.symbol || '?').charAt(0));
    if (token.tone) chip.style.setProperty('--tone', token.tone);
    if (urls && urls.length) attachLogo(chip, urls.slice());
    return chip;
  }

  // Walks the candidate URLs and stops at the first that loads. On exhaustion
  // the drawn letter that was already there simply stays — a logo is never
  // allowed to leave a hole where a symbol should be.
  function attachLogo(host, urls) {
    if (!urls.length) return;
    var img = document.createElement('img');
    img.alt = '';
    img.referrerPolicy = 'no-referrer';
    img.addEventListener('load', function () {
      host.textContent = '';
      host.classList.add('has-logo');
      host.appendChild(img);
    });
    img.addEventListener('error', function () { attachLogo(host, urls.slice(1)); });
    img.src = urls[0];
  }

  // The identicon is the anti-poisoning signal, and a truncated address is
  // exactly what poisoning defeats (an attacker mines a lookalike prefix and
  // suffix). So every address can be opened: a large avatar and the full
  // 42 characters, grouped so they can be read aloud.
  function addressPanel(address) {
    var panel = el('div', 'address-open');
    panel.appendChild(identicon(ns.identicon.forAddress(address), 96));

    var side = el('div', 'address-side');
    // One unbroken string: grouping is easier on the eye but harder to compare
    // against another window, and comparing is the whole point.
    side.appendChild(el('code', 'address-full-text', address));

    var copy = el('button', 'copy-button', t('ui.copyAddress'));
    copy.type = 'button';
    copy.addEventListener('click', function (event) {
      event.stopPropagation();
      var done = function () {
        copy.textContent = t('ui.copied');
        setTimeout(function () { copy.textContent = t('ui.copyAddress'); }, 1600);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(address).then(done, fallback);
      } else {
        fallback();
      }
      function fallback() {
        // execCommand is deprecated but still the only path in some embedded
        // views; a copy button that silently does nothing is worse.
        var field = document.createElement('textarea');
        field.value = address;
        document.body.appendChild(field);
        field.select();
        try { document.execCommand('copy'); done(); } catch (e) { /* give up quietly */ }
        document.body.removeChild(field);
      }
    });
    side.appendChild(copy);
    panel.appendChild(side);
    return panel;
  }

  function expandableIdentity(identity, size) {
    var wrap = el('div', 'identity-wrap');
    var button = el('button', 'identity');
    button.type = 'button';
    if (identity.identicon) button.appendChild(identicon(identity.identicon, size || 18));
    var text = el('span', 'identity-text');
    if (identity.contractName) text.appendChild(el('span', 'identity-name', identity.contractName));
    text.appendChild(el('span', 'identity-address', identity.short));
    button.appendChild(text);
    var more = el('span', 'identity-more');
    more.appendChild(icon('chevron-down', 'chevron'));
    button.appendChild(more);
    wrap.appendChild(button);

    var panel = addressPanel(identity.address);
    panel.hidden = true;
    wrap.appendChild(panel);
    button.addEventListener('click', function () {
      panel.hidden = !panel.hidden;
      button.classList.toggle('open', !panel.hidden);
    });
    return wrap;
  }

  function amountText(amount) {
    return amount.textKey ? t(amount.textKey) : amount.text;
  }

  function heroAmount(hero) {
    var box = el('div', 'hero-amount');
    var line = el('div', 'amount-line');
    // Every digit stays on screen. An exact figure (a plain send's, 082 RC5)
    // can run to 18 decimals, and its last digit is the one that differs:
    // "1,000.000000000000000001" clipped at the card's edge reads 1,000. A
    // long figure steps down a size; one that still does not fit wraps.
    var figure = amountText(hero.amount);
    var size = figure.length > 16 ? ' amount-longer' : figure.length > 10 ? ' amount-long' : '';
    line.appendChild(el('span', 'amount' + size + (hero.amount.unlimited ? ' amount-danger' : ''), figure));
    line.appendChild(tokenChip(hero.amount.token, hero.amount.logos));
    line.appendChild(el('span', 'amount-symbol', hero.amount.symbol));
    box.appendChild(line);
    if (hero.amount.fiat) box.appendChild(el('div', 'amount-fiat', hero.amount.fiat));
    return box;
  }

  function hero(view) {
    var h = view.hero;
    if (!h) return null;
    if (h.kind === 'amount') return heroAmount(h);
    if (h.kind === 'nft') {
      var nft = el('div', 'hero-amount');
      var line = el('div', 'amount-line');
      line.appendChild(el('span', 'amount', '#' + h.id));
      line.appendChild(el('span', 'amount-symbol', (h.collection && h.collection.name) || 'NFT'));
      nft.appendChild(line);
      return nft;
    }
    if (h.kind === 'hash') return el('pre', 'block block-hash', h.hex);
    if (h.kind === 'message') return el('pre', 'block block-message', h.text);
    if (h.kind === 'siwe') {
      var siwe = el('div', 'hero-siwe');
      siwe.appendChild(el('div', 'siwe-domain' + (h.mismatch ? ' bad' : ''), h.domain));
      if (h.statement) siwe.appendChild(el('div', 'siwe-statement', h.statement));
      return siwe;
    }
    if (h.kind === 'code') {
      var code = el('div', 'hero-call');
      code.appendChild(el('code', 'call-name', h.titleKey ? t(h.titleKey) : h.title));
      var sub = h.subKey ? t(h.subKey, h.subParams) : h.sub;
      if (sub) code.appendChild(el('div', 'call-contract', sub));
      return code;
    }
    return null;
  }

  function fieldRow(field) {
    var row = el('div', 'row' + (field.warning || field.expired ? ' row-warn' : '') + (field.identity ? ' row-identity' : ''));
    row.appendChild(el('span', 'row-label', field.labelRaw || t(field.label)));
    var value = el('span', 'row-value');
    if (field.identity) {
      if (field.identity.tag) value.appendChild(el('div', 'row-tag', t(field.identity.tag)));
      value.appendChild(expandableIdentity(field.identity, 18));
    } else if (field.amount) {
      value.appendChild(el('span', field.amount.unlimited ? 'strong danger' : 'strong',
        amountText(field.amount) + ' ' + field.amount.symbol));
      if (field.amount.fiat) value.appendChild(el('div', 'row-sub', field.amount.fiat));
    } else {
      value.appendChild(el('span', field.emphasis ? 'strong' : '', field.valueKey ? t(field.valueKey) : field.value));
      if (field.expired) value.appendChild(el('span', 'tag tag-bad', t('tag.expired')));
    }
    row.appendChild(value);
    return row;
  }

  function legCard(leg) {
    var card = el('div', 'leg');
    var head = el('div', 'leg-head');
    head.appendChild(el('span', 'leg-index', t('ui.legTitle', { index: leg.index, intent: t(leg.view.intentKey) })));
    if (leg.view.risk === 'danger' || leg.view.risk === 'hard-danger') {
      head.appendChild(el('span', 'tag tag-bad', t('tag.danger')));
    }
    card.appendChild(head);
    leg.view.fields
      .filter(function (f) { return !f.detail && !f.consumedByHero; })
      .forEach(function (field) { card.appendChild(fieldRow(field)); });
    if (leg.view.hero && leg.view.hero.kind === 'amount') {
      card.insertBefore(fieldRow({
        label: leg.view.hero.direction === 'in' ? 'field.minReceive' : 'field.amount',
        amount: leg.view.hero.amount,
      }), card.children[1] || null);
    }
    return card;
  }

  function balanceCard(balance) {
    var card = el('div', 'balance');
    card.appendChild(el('div', 'balance-title', t('ui.balanceChange')));
    balance.rows.forEach(function (row) {
      var line = el('div', 'row');
      line.appendChild(el('span', 'row-label', row.symbol));
      line.appendChild(el('span', 'row-value ' + (String(row.delta).startsWith('-') ? 'neg' : 'pos'), row.delta));
      card.appendChild(line);
    });
    if (balance.note) card.appendChild(el('div', 'balance-note', balance.note));
    // The simulation was run by whoever asked for the signature. Its numbers may
    // well be right; they are still their numbers, and the sheet says so.
    if (balance.claimed) card.appendChild(el('div', 'balance-claimed', t('ui.simClaimed')));
    return card;
  }

  function warningBanner(warning) {
    var banner = el('div', 'warning warning-' + warning.tone);
    banner.appendChild(icon('triangle-alert', 'warning-mark'));
    banner.appendChild(el('span', 'warning-text', t(warning.key, warning.params)));
    return banner;
  }

  // The universal fallback renderer from the spec: five fixed layers, so a
  // request nobody understands still shows everything that is knowable.
  function techPanel(view, open) {
    var tech = view.tech;
    if (!tech) return null;
    var box = el('details', 'tech');
    if (open) box.open = true;
    var summary = el('summary', 'tech-summary');
    summary.appendChild(icon('chevron-right', 'chevron'));
    summary.appendChild(el('span', null, t('ui.techDetails')));
    box.appendChild(summary);

    var body = el('div', 'tech-body');
    body.appendChild(el('div', 'tech-label', t('ui.function')));
    body.appendChild(el('code', 'tech-signature',
      typeof tech.signature === 'string' ? tech.signature : t(tech.signature)));

    if (tech.params.length) {
      body.appendChild(el('div', 'tech-label', t('ui.params')));
      tech.params.forEach(function (param) {
        var row = el('div', 'row');
        row.appendChild(el('span', 'row-label mono', param.name));
        row.appendChild(el('span', 'row-value mono', param.valueKey ? t(param.valueKey) : param.value));
        body.appendChild(row);
      });
    }

    if (tech.addresses.length) {
      body.appendChild(el('div', 'tech-label', t('ui.addresses')));
      tech.addresses.forEach(function (entry) {
        var card = el('div', 'address-card');
        card.appendChild(el('div', 'address-role', t(entry.roleKey, entry.roleParams) + ' · ' + entry.name));
        card.appendChild(el('div', 'address-full mono', entry.address));
        body.appendChild(card);
      });
    }

    if (tech.sim) {
      body.appendChild(el('div', 'tech-label', t('ui.simulation')));
      body.appendChild(el('div', 'tech-sim', typeof tech.sim === 'string' ? tech.sim : t(tech.sim)));
    }

    if (view.digest) {
      var digestLabel = t('ui.digest', { kind: view.digest.kind }) +
        (view.digest.unwrapped ? ' · ' + t('ui.digestUnwrapped') : '');
      body.appendChild(el('div', 'tech-label', digestLabel));
      body.appendChild(el('pre', 'tech-raw', view.digest.hex));
    }

    // A ceremony's challenge: derived by this page (lib/ceremony.js), shown
    // exactly as it will be signed.
    if (view.challenge) {
      body.appendChild(el('div', 'tech-label', t('ui.challenge')));
      body.appendChild(el('pre', 'tech-raw challenge-text', view.challenge.text));
      if (view.challenge.noteKey) body.appendChild(el('div', 'tech-note', t(view.challenge.noteKey)));
    }

    if (tech.raw) {
      body.appendChild(el('div', 'tech-label', t('ui.rawData', { bytes: tech.raw.bytes })));
      body.appendChild(el('pre', 'tech-raw', tech.raw.hex));
    }

    box.appendChild(body);
    return box;
  }

  // Read-only by construction, and READ rather than quoted — see lib/fee.js.
  // Vela pays in band, so the usual case is a payment leg inside the calldata:
  // its amount and recipient are decoded facts, while "this leg is the fee" is
  // the requester's label and is shown as one.
  function feeRow(view) {
    var wrap = el('div', 'fee-wrap');
    // Nothing goes on chain: the app's quiet reassurance (PositiveNote) in
    // place of a fee line.
    if (!view.fee) {
      var ok = el('p', 'fee-ok fee-off');
      ok.appendChild(icon('check'));
      ok.appendChild(el('span', 'fee-value', t('ui.feeOffchain')));
      wrap.appendChild(ok);
      return wrap;
    }
    var row = el('div', 'fee');
    row.appendChild(el('span', 'fee-label', t('ui.fee')));
    if (view.fee.unknown) {
      row.appendChild(el('span', 'fee-value', t('ui.feeUnstated')));
      wrap.appendChild(row);
      return wrap;
    }

    // Spec 079: the explanation folds under the row — the row itself is the
    // summary a person taps. What must stay in sight stays on the row: the
    // amount read from the calldata and, for a leg, that "this is the fee" is
    // the requester's word — said in muted text beside the label (自述), the
    // same word the network row uses for a chain the page does not know.
    wrap = el('details', 'fee-wrap');
    row = el('summary', 'fee');
    row.appendChild(el('span', 'fee-label', t('ui.fee')));
    if (view.fee.leg) row.appendChild(el('span', 'tag', t('tag.claimed')));
    var headline = view.fee.leg
      ? t('ui.feeLegAmount', { amount: view.fee.leg.amount, symbol: view.fee.leg.symbol })
      : t('ui.feeCeiling', { amount: view.fee.gas.max, symbol: view.fee.gas.symbol });
    var fiat = (view.fee.leg && view.fee.leg.fiat) || (view.fee.gas && view.fee.gas.fiat);
    // The amount and its chevron stay together when a long label wraps.
    var end = el('span', 'fee-end');
    end.appendChild(el('span', 'fee-value', headline + (fiat ? ' ' + fiat : '')));
    end.appendChild(icon('chevron-right', 'chevron fee-chevron'));
    row.appendChild(end);
    wrap.appendChild(row);

    var note = el('div', 'fee-note');
    if (view.fee.leg) {
      note.appendChild(el('div', null, t('ui.feeLegClaim', { index: view.fee.leg.index + 1 })));
      var paid = el('div', 'fee-recipient');
      paid.appendChild(document.createTextNode(t('ui.feePaidTo') + ' '));
      paid.appendChild(el('code', null, view.fee.leg.to));
      note.appendChild(paid);
      if (view.fee.leg.unverifiedDecimals) {
        note.appendChild(el('div', null, t('warn.unverifiedDecimals')));
      }
    }
    if (view.fee.gas) note.appendChild(el('div', null, t('ui.feeGas', { gas: view.fee.gas.gas })));
    var rate = (view.fee.leg && view.fee.leg.rate) || (view.fee.gas && view.fee.gas.rate);
    var rateSymbol = view.fee.leg ? view.fee.leg.symbol : view.fee.gas && view.fee.gas.symbol;
    note.appendChild(el('div', null, rate
      ? t('ui.feeRate', {
        symbol: rateSymbol, currency: view.fee.currency, rate: rate.toLocaleString('en-US'),
      })
      : t('ui.feeNoRate')));
    wrap.appendChild(note);
    // Who pays changes the number's meaning: said on the row's level, never folded.
    if (view.fee.sponsored) wrap.appendChild(el('div', 'fee-sponsored', t('ui.feeSponsored')));
    return wrap;
  }

  /**
   * The one control: a button (spec 102 — every confirmation in Vela is a
   * tap, the apps' included). Its words are the action (`view.confirmKey`).
   * A request this page refuses still shows the button, disabled and saying
   * so, where the person expects it. sign.js arms it.
   */
  function confirmBar(view) {
    var bar = el('div', 'confirm-bar');
    var button = el('button', 'confirm', t(view.confirmKey || 'button.sign'));
    button.type = 'button';
    if (view.refuse) {
      button.disabled = true;
      button.classList.add('confirm-off');
    }
    bar.appendChild(button);
    if (!view.refuse) bar.appendChild(el('p', 'confirm-hint', t('ui.closeRefuses')));
    return bar;
  }

  // Line icons for a key's place, drawn here (no image can be fetched).
  var PLACE_ICONS = {
    platform: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6.5" y="2.5" width="11" height="19" rx="2.5"/><path d="M10.5 18.5h3"/></svg>',
    hybrid: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="2.5" y="4.5" width="9" height="15" rx="2"/><path d="M6 16.5h2"/><rect x="14.5" y="8.5" width="7" height="7" rx="1"/><path d="M17 11h2v2h-2z"/></svg>',
    security_key: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="7.5" y="7.5" width="9" height="14" rx="2"/><path d="M9.5 7.5v-5h5v5"/><circle cx="12" cy="14.5" r="1.6"/></svg>',
  };

  /** Where the person confirms: the key's place, from the wallet's route. */
  function keyRow(view) {
    if (!view.key || !view.key.place) return null;
    var row = el('div', 'row key-row');
    row.appendChild(el('span', 'row-label', t(view.keyLabel || 'field.confirmWith')));
    var value = el('span', 'row-value key-place');
    var icon = el('span', 'key-icon');
    icon.innerHTML = PLACE_ICONS[view.key.place] || '';
    value.appendChild(icon);
    value.appendChild(el('span', 'key-place-name', t('place.' + view.key.place)));
    row.appendChild(value);
    return row;
  }

  /**
   * Who is asking, as one quiet line under the title — or nothing, for the
   * wallet's own request (`view.dapp.own`). The words and the mark are
   * resolve.js's: a verified requester, the destination of the answer, or a
   * name the requester gave for itself, said as such.
   */
  function requester(view, ceremony) {
    if (!ceremony && view.dapp.own) return null;
    // A key ceremony is the wallet asking for itself, as the apps' own sheets
    // are: no site to put a face to, so the line is a quiet subtitle under
    // the title. A mark is drawn only where a requester sent one — the line
    // beside it says it is its own.
    var line = el('div', 'requester' + (ceremony ? ' requester-quiet' : ''));
    if (!ceremony || view.dapp.icon) line.appendChild(remoteLogo(view.dapp.icon, view.dapp.letter, view.dapp.tone));
    var identity = el('div', 'sheet-identity');
    identity.appendChild(el('div', 'dapp-name', view.dapp.name || t(view.dapp.nameKey)));
    if (ceremony && view.dapp.nameClaimed) {
      identity.appendChild(el('div', 'dapp-claimed', t('tag.selfReported')));
    }
    // `originShown`, not `origin`: a host the name above already says is not
    // drawn twice (resolve.js decides; 082 L-HOST).
    if (ceremony && (view.dapp.originShown || view.dapp.originKey)) {
      identity.appendChild(el('div', 'dapp-origin', view.dapp.originShown ? view.dapp.origin : t(view.dapp.originKey)));
    } else if (!ceremony && view.dapp.originShown) {
      identity.appendChild(el('div', 'dapp-origin', view.dapp.origin));
    }
    var note = noteFor(view, 'requester');
    if (note) identity.appendChild(note);
    line.appendChild(identity);
    return line;
  }

  /**
   * The title row: the intent, as the first thing read — in the sheet's title
   * type, as the apps title the wallet's own request. The network is not a
   * chip beside it but a row with the others (`networkRow`), as in the apps.
   */
  function titleRow(view) {
    var head = el('header', 'sheet-top');
    var label = el('h1', 'intent-label tone-' + view.risk, t(view.intentKey));
    if (view.badge) label.appendChild(el('span', 'tag', t(view.badge)));
    head.appendChild(label);
    return head;
  }

  /**
   * 网络 · Base. A chain this page does not know is named by the requester,
   * and the row says so (自述) — quietly, in the row's own muted type.
   */
  function networkRow(view) {
    if (!view.chain) return null;
    var row = el('div', 'row network-row');
    row.appendChild(el('span', 'row-label', t('field.network')));
    var value = el('span', 'row-value');
    value.appendChild(el('span', 'network-name', view.chain));
    if (view.chainClaimed) value.appendChild(el('span', 'tag network-claimed', t('tag.claimed')));
    row.appendChild(value);
    return row;
  }

  // Refusals and dangers first, right under the title — a card that will not
  // sign says why before anything else; cautions sit with the facts they are
  // about.
  function warningsOf(view, danger) {
    return view.warnings.filter(function (w) { return !w.near && (w.tone === 'danger') === danger; });
  }

  // A caution about one line of the card, drawn as a quiet note under it.
  function noteFor(view, near) {
    var notes = view.warnings.filter(function (w) { return w.near === near; });
    if (!notes.length) return null;
    var box = el('div', 'warning warning-note');
    notes.forEach(function (w) { box.appendChild(el('span', 'warning-text', t(w.key, w.params))); });
    return box;
  }

  /**
   * The calm screen between requests: who this page is talking to.
   * `state` = { titleKey, noteKey, originKey | origin }. Nothing here can be
   * confirmed; there is no button.
   */
  function renderWaiting(state) {
    var sheet = el('article', 'sheet sheet-waiting');
    var head = el('header', 'sheet-head');
    // Only a channel that vouches for the peer gets Vela's mark; over a
    // loopback socket this is whatever connected, and it is drawn as that.
    var named = state.requesterApp || '';
    head.appendChild(
      state.requesterVerified
        ? avatar('V', '#ff6a1a')
        : remoteLogo(
          state.requesterIcon || null,
          (named || '?').slice(0, 1).toUpperCase(),
          ns.resolve.toneFor(named),
        ),
    );
    var identity = el('div', 'sheet-identity');
    identity.appendChild(el('div', 'dapp-name',
      state.requesterVerified ? t('tag.velaWallet') : (named || t('tag.someWallet'))));
    if (!state.requesterVerified && named) {
      identity.appendChild(el('div', 'dapp-claimed', t('tag.selfReported')));
    }
    if (state.origin || state.originKey) {
      identity.appendChild(el('div', 'dapp-origin', state.origin || t(state.originKey)));
    }
    head.appendChild(identity);
    sheet.appendChild(head);
    sheet.appendChild(el('div', 'waiting-title', t(state.titleKey)));
    if (state.noteKey) sheet.appendChild(el('p', 'sentence waiting-note', t(state.noteKey, state.noteParams)));
    return sheet;
  }

  // The app's footer (SigningBody): a hairline, then the folded technical
  // details, the fee, the account, the button — in that order, as there.
  function footer() {
    return el('div', 'sheet-foot');
  }

  function renderCeremony(view, options) {
    // No hero: the wallet it is for is a row, and a second mark beside the
    // title only said it again (owner, 2026-10-09: 「+ savings 看着很奇怪」).
    // The title, who asks, one sentence, the rows, the button.
    var sheet = el('article', 'sheet sheet-ceremony risk-' + view.risk);
    var head = titleRow(view);
    head.appendChild(requester(view, true));
    sheet.appendChild(head);

    var blocks = el('div', 'blocks');
    warningsOf(view, true).forEach(function (w) { blocks.appendChild(warningBanner(w)); });

    var h = hero(view);
    if (h) blocks.appendChild(h);
    if (view.sentence) blocks.appendChild(el('p', 'sentence', t(view.sentence)));

    var rows = el('div', 'rows');
    view.fields.filter(function (f) { return !f.detail; })
      .forEach(function (field) { rows.appendChild(fieldRow(field)); });
    var place = keyRow(view);
    if (place) rows.appendChild(place);
    if (rows.children.length) blocks.appendChild(rows);

    warningsOf(view, false).forEach(function (w) { blocks.appendChild(warningBanner(w)); });
    sheet.appendChild(blocks);

    var foot = footer();
    var tech = techPanel(view, options.techOpen);
    if (tech) foot.appendChild(tech);
    foot.appendChild(confirmBar(view));
    sheet.appendChild(foot);
    return sheet;
  }

  /** Build the whole sheet. `options.techOpen` expands the fallback panel. */
  function render(view, options) {
    options = options || {};
    if (view.kind === 'ceremony') return renderCeremony(view, options);
    var sheet = el('article', 'sheet risk-' + view.risk);

    // 1. What: the intent, who asks (if anyone), and why not, when not.
    sheet.appendChild(titleRow(view));
    var blocks = el('div', 'blocks');
    var who = requester(view, false);
    if (who) blocks.appendChild(who);
    warningsOf(view, true).forEach(function (w) { blocks.appendChild(warningBanner(w)); });

    var h = hero(view);
    if (h) blocks.appendChild(h);
    if (view.sentence) blocks.appendChild(el('p', 'sentence', t(view.sentence)));

    view.legs.forEach(function (leg) { blocks.appendChild(legCard(leg)); });

    // The facts, as the app's label / value rows — the network first, as the
    // apps list it.
    var rows = el('div', 'rows');
    var network = networkRow(view);
    if (network) rows.appendChild(network);
    view.fields.filter(function (f) { return !f.detail && !f.consumedByHero; })
      .forEach(function (field) { rows.appendChild(fieldRow(field)); });
    if (rows.children.length) blocks.appendChild(rows);

    if (view.balance) blocks.appendChild(balanceCard(view.balance));
    warningsOf(view, false).forEach(function (w) { blocks.appendChild(warningBanner(w)); });
    sheet.appendChild(blocks);

    var foot = footer();
    // The details a person needs only when something looks wrong, folded —
    // where the apps fold theirs, above the fee.
    var tech = techPanel(view, options.techOpen);
    if (tech) foot.appendChild(tech);

    var fee = feeRow(view);
    if (fee) foot.appendChild(fee);

    // 2. With which key. The signing account is an ADDRESS with its locally
    // derived avatar; a name beside it is only ever one the wallet keeps for
    // its own account — it helps pick the right passkey, and the address and
    // identicon next to it remain the anchor.
    var signer = el('div', 'signer');
    var account = el('div', 'row signer-row');
    account.appendChild(el('span', 'row-label signer-label', t('ui.signer')));
    var who2 = el('span', 'row-value');
    if (view.account) {
      who2.appendChild(expandableIdentity({
        address: view.account,
        short: ns.format.short(view.account),
        identicon: ns.identicon.forAddress(view.account),
        contractName: view.accountName || null,
      }, 18));
    } else {
      who2.appendChild(el('span', 'signer-address', t('ui.accountUnknown')));
    }
    account.appendChild(who2);
    signer.appendChild(account);
    var place = keyRow(view);
    if (place) signer.appendChild(place);
    foot.appendChild(signer);

    // 3. The button, and the line under it.
    foot.appendChild(confirmBar(view));
    sheet.appendChild(foot);
    return sheet;
  }

  ns.render = render;
  ns.render.waiting = renderWaiting;
})(window.VelaCS);
