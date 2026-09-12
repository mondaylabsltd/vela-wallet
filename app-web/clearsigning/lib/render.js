// View model → DOM. Nothing here decides anything: every judgement was made in
// resolve.js and every word comes from lib/locales/. Keeping the decisions out
// of the renderer is what makes "what you read is what gets signed" checkable
// by reading one file.
//
// There is also nothing here that can EDIT the request. No amount editor, no
// fee-token picker. The intent arrives fixed; the only two outcomes are sign
// (slide) and refuse (close).
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
    if (!url || !/^https:\/\//.test(url)) return avatar(letter, tone);
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
    if (identity.identicon) button.appendChild(identicon(identity.identicon, size || 22));
    var text = el('span', 'identity-text');
    if (identity.contractName) text.appendChild(el('span', 'identity-name', identity.contractName));
    text.appendChild(el('span', 'identity-address', identity.short));
    button.appendChild(text);
    button.appendChild(el('span', 'identity-more', '⌄'));
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
    line.appendChild(el('span', 'amount' + (hero.amount.unlimited ? ' amount-danger' : ''), amountText(hero.amount)));
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
    var row = el('div', 'row' + (field.warning || field.expired ? ' row-warn' : ''));
    row.appendChild(el('span', 'row-label', field.labelRaw || t(field.label)));
    var value = el('span', 'row-value');
    if (field.identity) {
      if (field.identity.tag) value.appendChild(el('div', 'row-tag', t(field.identity.tag)));
      value.appendChild(expandableIdentity(field.identity, 22));
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
    banner.appendChild(el('span', 'warning-mark', '▲'));
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
    box.appendChild(el('summary', 'tech-summary', t('ui.techDetails')));

    var body = el('div', 'tech-body');
    body.appendChild(el('div', 'tech-label', t('ui.function')));
    body.appendChild(el('code', 'tech-signature',
      typeof tech.signature === 'string' ? tech.signature : t(tech.signature)));

    if (tech.params.length) {
      body.appendChild(el('div', 'tech-label', t('ui.params')));
      tech.params.forEach(function (param) {
        var row = el('div', 'row');
        row.appendChild(el('span', 'row-label mono', param.name));
        row.appendChild(el('span', 'row-value mono', param.value));
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
    var row = el('div', 'fee' + (view.fee ? '' : ' fee-off'));
    row.appendChild(el('span', 'fee-label', t('ui.fee')));

    if (!view.fee) {
      row.appendChild(el('span', 'fee-value', t('ui.feeOffchain')));
      wrap.appendChild(row);
      return wrap;
    }
    if (view.fee.unknown) {
      row.appendChild(el('span', 'fee-value', t('ui.feeUnstated')));
      wrap.appendChild(row);
      return wrap;
    }

    var headline = view.fee.leg
      ? t('ui.feeLegAmount', { amount: view.fee.leg.amount, symbol: view.fee.leg.symbol })
      : t('ui.feeCeiling', { amount: view.fee.gas.max, symbol: view.fee.gas.symbol });
    var fiat = (view.fee.leg && view.fee.leg.fiat) || (view.fee.gas && view.fee.gas.fiat);
    row.appendChild(el('span', 'fee-value', headline + (fiat ? ' ' + fiat : '')));
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
    if (view.fee.sponsored) note.appendChild(el('div', 'fee-sponsored', t('ui.feeSponsored')));
    wrap.appendChild(note);
    return wrap;
  }

  function slider(view) {
    var track = el('div', 'slide' + (view.refuse ? ' slide-off' : ''));
    track.appendChild(el('span', 'slide-knob', '→'));
    track.appendChild(el('span', 'slide-label', t('ui.slide', { intent: sliderIntent(view) })));
    return track;
  }

  // The spec's rule: name the intent on the slider, and fall back to a bare
  // "confirm" only when the name is too long to read at a glance.
  function sliderIntent(view) {
    var name = t(view.intentKey);
    return name.length > 12 ? t('ui.confirm') : name;
  }

  /** Build the whole sheet. `options.techOpen` expands the fallback panel. */
  function render(view, options) {
    options = options || {};
    var sheet = el('article', 'sheet risk-' + view.risk);
    sheet.appendChild(el('div', 'grabber'));

    var head = el('header', 'sheet-head');
    head.appendChild(remoteLogo(view.dapp.icon, view.dapp.letter, view.dapp.tone));
    var identity = el('div', 'sheet-identity');
    identity.appendChild(el('div', 'dapp-name', view.dapp.name || t(view.dapp.nameKey)));
    if (view.dapp.origin) identity.appendChild(el('div', 'dapp-origin', view.dapp.origin));
    head.appendChild(identity);
    var chain = el('span', 'chain-pill');
    var dot = el('i', 'chain-dot');
    if (view.chainLogos && view.chainLogos.length) attachLogo(dot, view.chainLogos.slice());
    chain.appendChild(dot);
    chain.appendChild(document.createTextNode(view.chain));
    if (view.chainClaimed) chain.appendChild(el('span', 'tag', t('tag.claimed')));
    head.appendChild(chain);
    sheet.appendChild(head);

    var label = el('div', 'intent-label tone-' + view.risk, t(view.intentKey));
    if (view.badge) label.appendChild(el('span', 'tag', t(view.badge)));
    sheet.appendChild(label);

    var h = hero(view);
    if (h) sheet.appendChild(h);

    if (view.sentence) sheet.appendChild(el('p', 'sentence', t(view.sentence)));

    view.legs.forEach(function (leg) { sheet.appendChild(legCard(leg)); });

    var visible = view.fields.filter(function (f) { return !f.detail && !f.consumedByHero; });
    if (visible.length) {
      var rows = el('div', 'rows');
      visible.forEach(function (field) { rows.appendChild(fieldRow(field)); });
      sheet.appendChild(rows);
    }

    if (view.balance) sheet.appendChild(balanceCard(view.balance));

    view.warnings.forEach(function (warning) { sheet.appendChild(warningBanner(warning)); });

    var tech = techPanel(view, options.techOpen);
    if (tech) sheet.appendChild(tech);

    var fee = feeRow(view);
    if (fee) sheet.appendChild(fee);

    // The signing account is an ADDRESS with its locally derived avatar. A
    // nickname here would be the one thing on the sheet a requester could
    // choose, and it would sit next to the button that spends the money.
    var signer = el('div', 'signer');
    signer.appendChild(el('span', 'signer-label', t('ui.signer')));
    if (view.account) {
      // A name here is only ever one THIS DEVICE recorded when the key was
      // enrolled — it helps pick the right passkey. The requester's idea of
      // what to call your account is precisely the label an attacker would
      // choose, and it would sit next to the button that spends the money.
      signer.appendChild(expandableIdentity({
        address: view.account,
        short: ns.format.short(view.account),
        identicon: ns.identicon.forAddress(view.account),
        contractName: view.accountName || null,
      }, 26));
    } else {
      signer.appendChild(el('span', 'signer-address', t('ui.accountUnknown')));
    }
    sheet.appendChild(signer);

    sheet.appendChild(slider(view));
    return sheet;
  }

  ns.render = render;
})(window.VelaCS);
