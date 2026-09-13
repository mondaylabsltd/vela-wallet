// Signing intent → view model. One direction, no network, no I/O, no words.
//
// Two rules hold this file together:
//
//  1. It NEVER produces a human-readable string, only i18n keys and parameters.
//     All wording lives in lib/locales/, so it can be audited in one place.
//  2. It NEVER rewrites the intent. By the time a request reaches this page it
//     is fixed: sign it or refuse it. Anything that would edit an amount, a fee
//     token or a recipient belongs to whoever BUILT the intent — a page that
//     both displays and edits the payload is the exact bug this product exists
//     to prevent.
//
// The six-level ladder from the design spec also lives here: every level below
// the first drops a claim about MEANING while still showing every FACT we hold,
// and raises the warning a notch.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var abi = ns.abi;
  var reg = ns.registry;

  // Anything at or above 2^128 raw units is past every real token supply, so it
  // reads as unlimited. Permit2's own "unlimited" (type(uint160).max) is above
  // this line too — a higher threshold would quietly let it through as a
  // 22-digit number, which is exactly the kind of number nobody reads.
  var UNLIMITED_FLOOR = 1n << 128n;

  var PRICES = { ETH: 3400, WETH: 3400, USDC: 1, USDT: 1, DAI: 1, sDAI: 1.09, APE: 1.2 };

  var UNREADABLE = new RegExp('[\\u0000-\\u0008\\u000b\\u000c\\u000e-\\u001f\\u007f-\\u009f\\ufffd]');

  // --- formatting ------------------------------------------------------------

  function group(intPart) {
    return intPart.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  }

  function formatUnits(value, decimals, maxFrac) {
    var negative = value < 0n;
    var v = negative ? -value : value;
    var base = 10n ** BigInt(decimals);
    var whole = (v / base).toString();
    var frac = decimals ? (v % base).toString().padStart(decimals, '0') : '';
    if (maxFrac === undefined) maxFrac = decimals > 6 ? 6 : decimals;
    frac = frac.slice(0, maxFrac).replace(/0+$/, '');
    return (negative ? '-' : '') + group(whole) + (frac ? '.' + frac : '');
  }

  function fiat(symbol, value, decimals) {
    var price = PRICES[symbol];
    if (price === undefined) return null;
    var usd = (Number(value) / Math.pow(10, decimals)) * price;
    if (!isFinite(usd)) return null;
    return '≈ $' + (usd >= 1000 ? group(Math.round(usd).toString()) : usd.toFixed(2));
  }

  function shortAddress(address) {
    return address.slice(0, 6) + '…' + address.slice(-4);
  }

  function formatDate(seconds) {
    var d = new Date(Number(seconds) * 1000);
    var pad = function (n) { return String(n).padStart(2, '0'); };
    return d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate()) +
      ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
  }

  function text(key, params) {
    return { key: key, params: params };
  }

  // --- identity --------------------------------------------------------------

  /**
   * What we are willing to say about an address.
   *
   * Nothing here is taken on the requester's word. A name it supplied — a
   * contact label, a protocol title — is metadata it can choose freely, and a
   * poisoned label on an attacker's address is the oldest trick there is. So
   * the sheet shows the ADDRESS and the identicon derived from it locally, plus
   * only those tags we can establish ourselves:
   *
   *   · it is the account doing the signing (that address is inside the digest)
   *   · it is the contract this call is aimed at (read from the calldata)
   *   · it is a contract this page itself ships a reviewed descriptor for
   */
  function identify(address, ctx, roleHints) {
    var lower = (address || '').toLowerCase();
    var hints = roleHints || {};
    var known = reg.contract(lower);
    var tag = null;

    if (ctx.account && lower === String(ctx.account).toLowerCase()) tag = 'tag.self';
    else if (hints.isCallTarget) tag = 'tag.calledContract';
    else if (known && known.descriptor) tag = 'tag.localDescriptor';
    else if (known && known.verified) tag = 'tag.sourceVerified';

    return {
      address: address,
      short: shortAddress(address),
      // Local, deterministic, and impossible for a requester to influence.
      identicon: address ? ns.identicon.forAddress(address) : null,
      tag: tag,
      kind: tag === 'tag.self' ? 'own' : known ? 'contract' : 'unknown',
      // Kept for guards; never rendered as a name.
      contractName: known && known.name ? known.name : null,
    };
  }

  /**
   * A name is only ever shown when it comes from THIS page's own reviewed
   * table (lib/registry.js) — never from the request. Everything else is the
   * address, which cannot be relabelled by whoever is asking.
   */
  function label(identity) {
    return (identity && (identity.contractName || identity.short)) || '—';
  }

  // --- amounts ---------------------------------------------------------------

  function tokenAmount(value, tokenAddress, ctx) {
    var known = reg.token(tokenAddress);
    var token = known || { symbol: '?', nameKey: 'value.unknownToken', decimals: 18, tone: '#8a93a5' };
    var unlimited = value >= UNLIMITED_FLOOR;
    return {
      unlimited: unlimited,
      value: value,
      token: token,
      tokenAddress: tokenAddress,
      text: unlimited ? null : formatUnits(value, token.decimals),
      textKey: unlimited ? 'value.unlimited' : null,
      symbol: token.symbol,
      fiat: unlimited ? null : fiat(token.symbol, value, token.decimals),
      unverifiedDecimals: !known,
      // Decoration only — see lib/logos.js.
      logos: ctx ? ns.logos.token(ctx, ctx.chainId, tokenAddress) : [],
    };
  }

  function nativeAmount(value, ctx) {
    var symbol = ctx.nativeSymbol || 'ETH';
    return {
      value: value,
      token: { symbol: symbol, decimals: 18, tone: '#8a93a5' },
      symbol: symbol,
      text: formatUnits(value, 18),
      fiat: fiat(symbol, value, 18),
      unlimited: false,
    };
  }

  /** "1,000 USDC" / "Unlimited USDC" — for interpolation into a sentence. */
  function amountPhrase(amount) {
    var head = amount.textKey ? ns.i18n.t(amount.textKey) : amount.text;
    return head + ' ' + amount.symbol;
  }

  // --- view scaffold ---------------------------------------------------------

  function baseView(intent, ctx) {
    var origin = (intent && intent.origin) || '';
    var host = origin.replace(/^https?:\/\//, '').replace(/\/.*$/, '');
    var known = ctx.dapp;
    return {
      dapp: {
        name: (known && known.name) || null,
        nameKey: (known && known.name) ? null : (host ? 'tag.unknownSite' : 'tag.wallet'),
        origin: host || null,
        letter: ((known && known.name) || host || '?').charAt(0).toUpperCase(),
        tone: (known && known.tone) || '#8a93a5',
        // Display-only, and it never touches the digest: a logo cannot change
        // what gets signed, so an https URL from the request is allowed here.
        icon: (known && known.icon) || null,
        // Only a channel the browser itself vouches for (postMessage, extension
        // messaging) proves who is asking. Everything else is the requester's
        // own word, and has to be shown as such.
        originVerified: ctx.originVerified === true,
      },
      // Named from the chain id we are actually signing for. A requester's own
      // label is only used for a chain this page does not know, and is marked.
      chain: reg.chainName(ctx.chainId) || ctx.chainName || ('chain ' + (ctx.chainId || '?')),
      chainClaimed: !reg.chainName(ctx.chainId),
      chainLogos: ns.logos.chain(ctx, ctx.chainId),
      // The account whose key signs. For a transaction this is also inside
      // the digest (SafeOp.safe), so it is not merely a claim.
      account: (ctx.operation && ctx.operation.userOp && ctx.operation.userOp.sender) ||
        ctx.account || null,
      // The account's name IS shown, unlike a counterparty's. It exists to help
      // find the right passkey in the ceremony that follows, and it cannot
      // redirect anything: the money moves per the calldata, and the account
      // itself is `userOp.sender`, which sits inside the signed digest. The
      // address and identicon next to it remain the anchor.
      accountName: (ctx.signer && ctx.signer.name) || null,
      fields: [],
      warnings: [],
      legs: [],
      risk: 'normal',
      level: 1,
      signer: ctx.signer || { name: '—', letter: '?' },
      fee: null,
    };
  }

  function techFor(signature, values, calldata, extra) {
    return {
      signature: signature,
      params: (extra && extra.params) || [],
      addresses: (extra && extra.addresses) || [],
      sim: extra && extra.sim,
      raw: calldata && calldata !== '0x' ? { bytes: abi.byteLength(calldata), hex: calldata } : null,
      values: values,
    };
  }

  function renderRaw(v) {
    if (typeof v === 'bigint') return group(v.toString());
    if (Array.isArray(v)) return '[' + v.map(renderRaw).join(', ') + ']';
    if (v && typeof v === 'object') return JSON.stringify(v);
    return String(v);
  }

  function rank(risk) {
    return { safe: 0, normal: 1, caution: 2, danger: 3, 'hard-danger': 4 }[risk] || 1;
  }

  function raise(view, risk) {
    if (rank(risk) > rank(view.risk)) view.risk = risk;
  }

  // --- transactions ----------------------------------------------------------

  function resolveCall(call, ctx, view, isLeg) {
    var to = call.to || '';
    var data = call.data || '0x';
    var value = BigInt(call.value || '0x0');
    var selector = abi.selectorOf(data);
    var hasCalldata = !!data && data !== '0x';
    var meta = identify(to, ctx, { isCallTarget: hasCalldata });

    if (!to) {
      view.intentKey = 'intent.deploy';
      view.level = 3;
      view.risk = 'caution';
      view.hero = {
        kind: 'code',
        titleKey: 'value.newContract',
        subKey: 'value.codeBytes',
        subParams: { bytes: abi.byteLength(data) },
      };
      view.sentence = text('sentence.deploy');
      view.warnings.push({ tone: 'caution', key: 'warn.deploy' });
      view.tech = techFor(text('value.contractCreation'), null, data, {});
      return view;
    }

    if (!hasCalldata && value > 0n) {
      var amount = nativeAmount(value, ctx);
      view.intentKey = 'intent.send';
      view.hero = { kind: 'amount', amount: amount, direction: 'out' };
      view.fields.push({ label: 'field.recipient', identity: meta, format: 'addressName', role: 'recipient' });
      if (meta.kind === 'own') {
        view.risk = 'safe';
        view.sentence = text('sentence.sendSelf', { amount: amount.text + ' ' + amount.symbol });
      } else {
        // The sentence names the address, not a label: "to 0x9A8b…1a09".
        view.sentence = text('sentence.transfer', {
          amount: amount.text + ' ' + amount.symbol, to: meta.short,
        });
      }
      view.tech = techFor(text('value.noCalldata'), null, null, {
        params: [{ name: 'value', value: amount.text + ' ' + amount.symbol }],
        addresses: [{ roleKey: 'field.recipient', name: label(meta), address: to }],
        sim: text('ui.simNoOther', { delta: '-' + amount.text + ' ' + amount.symbol }),
      });
      return view;
    }

    var descriptor = selector ? reg.descriptorFor(selector) : null;

    // Level 1 — a verified descriptor gives us meaning.
    if (descriptor && !descriptor.abiOnly) {
      var values = abi.decode(descriptor.signature, data);
      if (values) return applyDescriptor(descriptor, values, call, ctx, view, meta, value, isLeg);
    }

    // Level 2 — source verified, ABI decodes, nobody wrote down what it means.
    if (descriptor && descriptor.abiOnly) {
      var abiValues = abi.decode(descriptor.signature, data);
      var parsed = abi.parseSignature(descriptor.signature);
      view.intentKey = 'intent.contractCall';
      view.level = 2;
      view.risk = 'caution';
      view.hero = { kind: 'code', title: parsed.name + '(…)', sub: label(meta) };
      view.sentence = text('sentence.abiOnly');
      view.fields = (abiValues || []).map(function (v, i) {
        return { labelRaw: parsed.types[i], value: renderRaw(v), format: 'raw' };
      });
      view.warnings.push({ tone: 'caution', key: 'warn.abiOnly' });
      view.tech = techFor(descriptor.signature, abiValues, data, {
        params: (abiValues || []).map(function (v, i) { return { name: parsed.types[i], value: renderRaw(v) }; }),
        addresses: [{ roleKey: 'ui.contract', name: label(meta), address: to }],
        sim: ctx.simulation && ctx.simulation.summary,
      });
      return view;
    }

    // Level 3 — only a 4-byte name.
    var known4 = selector && reg.fourbyte[selector];
    if (known4) {
      var p4 = abi.parseSignature(known4);
      var v4 = abi.decode(known4, data);
      view.intentKey = 'intent.unknownCall';
      view.level = 3;
      view.risk = 'caution';
      view.hero = { kind: 'code', title: p4.name + '(…)', sub: label(meta) };
      view.sentence = text('sentence.fourbyte', { name: p4.name });
      view.fields = (v4 || []).map(function (v, i) {
        return { labelRaw: p4.types[i], value: renderRaw(v), format: 'raw' };
      });
      view.warnings.push({ tone: 'caution', key: 'warn.fourbyte' });
      view.tech = techFor(known4, v4, data, {
        params: (v4 || []).map(function (v, i) { return { name: p4.types[i], value: renderRaw(v) }; }),
        addresses: [{ roleKey: 'ui.contract', name: label(meta), address: to }],
        sim: ctx.simulation && ctx.simulation.summary,
      });
      return view;
    }

    // Levels 4/5/6 — nothing decodes. The simulation becomes the subject.
    var sim = ctx.simulation;
    view.intentKey = 'intent.blind';
    view.hero = {
      kind: 'code',
      title: selector || '0x',
      subKey: 'value.unrecognized',
      subParams: { contract: label(meta) },
    };
    if (sim && sim.undeclared) {
      view.level = 5;
      view.risk = 'danger';
      view.sentence = text('sentence.blindUndeclared');
      view.warnings.push({ tone: 'danger', key: 'warn.blindUndeclared' });
    } else if (sim) {
      view.level = 4;
      view.risk = 'caution';
      view.sentence = text('sentence.blindSim');
      view.warnings.push({ tone: 'caution', key: 'warn.blindSim' });
    } else {
      view.level = 6;
      view.risk = 'danger';
      view.sentence = text('sentence.blindNothing');
      view.warnings.push({ tone: 'danger', key: 'warn.blindNothing' });
    }
    if (sim && sim.rows) view.balance = { rows: sim.rows, note: sim.note, claimed: true };
    view.tech = techFor(
      selector ? text('value.unknownFunction', { selector: selector }) : text('value.emptyCalldata'),
      null, data,
      { addresses: [{ roleKey: 'ui.contract', name: label(meta), address: to }], sim: sim && sim.summary },
    );
    return view;
  }

  function resolveToken(field, values, call, ctx) {
    if (field.tokenPath) {
      var path = field.tokenPath;
      if (path[path.length - 1] === 'last') {
        var list = abi.at(values, path.slice(0, -1));
        return list && list[list.length - 1];
      }
      return abi.at(values, path);
    }
    if (field.token === 'vaultAsset') return ctx.vaultAsset || call.to;
    return call.to;
  }

  function applyDescriptor(descriptor, values, call, ctx, view, meta, callValue, isLeg) {
    view.intentKey = 'intent.' + descriptor.kind;
    view.kind = descriptor.kind;
    view.level = 1;
    view.contract = meta;

    if (descriptor.nested) return applyNested(descriptor, values, call, ctx, view, meta);

    var hero = null;
    var rows = [];
    var approval = null;

    descriptor.fields.forEach(function (field) {
      var raw = abi.at(values, field.path);
      if (field.format === 'tokenAmount') {
        var amount = tokenAmount(raw, resolveToken(field, values, call, ctx), ctx);
        if (field.role === 'allowance' || field.role === 'allowance-delta') {
          approval = { amount: amount, mode: descriptor.approval };
          if (!hero) hero = { kind: 'amount', amount: amount, direction: 'allowance' };
        } else if (!hero && (field.role === 'send-amount' || field.role === 'receive-amount')) {
          hero = { kind: 'amount', amount: amount, direction: field.role === 'send-amount' ? 'out' : 'in' };
        }
        rows.push({ label: field.label, amount: amount, format: 'tokenAmount', detail: field.detail, role: field.role });
        if (amount.unverifiedDecimals) view.warnings.push({ tone: 'caution', key: 'warn.unverifiedDecimals' });
      } else if (field.format === 'addressName') {
        rows.push({
          label: field.label,
          identity: identify(raw, ctx),
          format: 'addressName',
          detail: field.detail,
          role: field.role,
        });
      } else if (field.format === 'nftId') {
        hero = { kind: 'nft', collection: reg.token(call.to), id: raw.toString() };
        rows.push({ label: field.label, value: '#' + raw.toString(), format: 'raw' });
      } else if (field.format === 'date') {
        var expired = Number(raw) * 1000 < (ctx.now || Date.now());
        rows.push({ label: field.label, value: formatDate(raw), format: 'date', expired: expired });
        if (expired) {
          view.warnings.push({ tone: 'caution', key: 'warn.expired' });
          raise(view, 'caution');
        }
      } else if (field.format === 'approvalScope') {
        approval = { all: raw === true, mode: 'all' };
        rows.push({
          label: field.label,
          valueKey: raw ? 'value.allNfts' : 'value.revokeAll',
          format: 'raw',
          warning: raw === true,
        });
      } else if (field.format === 'nativeAmount') {
        var na = nativeAmount(raw, ctx);
        rows.push({ label: field.label, value: na.text + ' ' + na.symbol, format: 'raw' });
      }
    });

    if (descriptor.nativeIn && callValue > 0n) {
      var paid = nativeAmount(callValue, ctx);
      hero = { kind: 'amount', amount: paid, direction: 'out' };
      rows.unshift({ label: 'field.pay', amount: paid, format: 'tokenAmount', role: 'send-amount' });
    }

    view.fields = rows;
    view.hero = hero || { kind: 'code', title: abi.parseSignature(descriptor.signature).name + '(…)', sub: label(meta) };

    if (hero && hero.amount) {
      // The hero already shows this amount in 32px type; repeating it as a row
      // below is noise, and noise is where a wrong digit hides.
      var promoted = rows.find(function (r) { return r.amount === hero.amount; });
      if (promoted) promoted.consumedByHero = true;
    }

    if (approval) applyApprovalGuard(approval, view, ctx);
    applyRecipientGuards(call, view, rows);

    if (!view.sentence) view.sentence = sentenceFor(descriptor, rows, meta, view);

    var parsed2 = abi.parseSignature(descriptor.signature);
    view.tech = techFor(descriptor.signature, values, call.data, {
      params: values.map(function (v, i) { return { name: parsed2.types[i], value: renderRaw(v) }; }),
      addresses: addressesIn(rows, call, meta),
      sim: ctx.simulation && ctx.simulation.summary,
    });
    if (ctx.simulation && ctx.simulation.rows && !isLeg) {
      view.balance = { rows: ctx.simulation.rows, note: ctx.simulation.note, claimed: true };
    }
    return view;
  }

  function addressesIn(rows, call, meta) {
    var out = [{ roleKey: 'ui.contract', name: label(meta), address: call.to }];
    rows.forEach(function (r) {
      if (r.identity) out.push({ roleKey: r.label, name: label(r.identity), address: r.identity.address });
      if (r.amount && r.amount.tokenAddress) {
        out.push({
          roleKey: 'ui.tokenOf',
          roleParams: { name: r.amount.token.name || r.amount.symbol },
          name: r.amount.symbol,
          address: r.amount.tokenAddress,
        });
      }
    });
    return out;
  }

  // The never-unlimited mandate, under the no-editing rule: this page cannot
  // rewrite the cap, so an unlimited request is REFUSED here and the person is
  // pointed back at whoever built it. Offering an editor would mean signing
  // bytes other than the ones that arrived.
  function applyApprovalGuard(approval, view, ctx) {
    if (approval.mode === 'all') {
      if (approval.all) {
        view.risk = 'danger';
        view.warnings.push({ tone: 'danger', key: 'warn.approveAll' });
        view.warnings.push({ tone: 'danger', key: 'warn.approveAllLocked' });
        view.refuse = true;
      } else {
        view.risk = 'safe';
        view.intentKey = 'intent.revoke';
      }
      return;
    }
    var amount = approval.amount;
    if (approval.mode === 'set' && amount.value === 0n) {
      view.intentKey = 'intent.revoke';
      view.risk = 'safe';
      view.sentence = text('sentence.revoke', { token: amount.token.name || amount.symbol });
      return;
    }
    if (amount.unlimited) {
      view.risk = 'danger';
      view.warnings.push({ tone: 'danger', key: 'warn.unlimited', params: { symbol: amount.symbol } });
      view.warnings.push({ tone: 'danger', key: 'warn.unlimitedLocked' });
      view.refuse = true;
    } else if (approval.mode === 'increase') {
      var current = ctx.currentAllowance || 0n;
      var after = current + amount.value;
      view.fields.push({
        label: 'field.allowanceAfter',
        value: formatUnits(after, amount.token.decimals) + ' ' + amount.symbol,
        format: 'raw',
        emphasis: true,
      });
      view.sentence = text('sentence.increase', {
        current: formatUnits(current, amount.token.decimals) + ' ' + amount.symbol,
        after: formatUnits(after, amount.token.decimals) + ' ' + amount.symbol,
      });
      raise(view, 'caution');
    } else {
      view.badge = 'tag.capped';
    }
  }

  function applyRecipientGuards(call, view, rows) {
    rows.forEach(function (row) {
      if (!row.identity || row.role !== 'recipient') return;
      var address = (row.identity.address || '').toLowerCase();
      if (address === (call.to || '').toLowerCase()) {
        view.risk = 'danger';
        view.sentence = text('sentence.burn');
        view.warnings.push({ tone: 'danger', key: 'warn.burn' });
        view.refuse = true;
      } else if (row.identity.kind === 'own') {
        view.risk = 'safe';
      }
    });
  }

  function sentenceFor(descriptor, rows, meta, view) {
    var send = rows.find(function (r) { return r.amount && r.role === 'send-amount'; });
    var to = rows.find(function (r) { return r.identity && (r.role === 'recipient' || r.role === 'spender'); });
    var kind = descriptor.kind;

    if (kind === 'nftTransfer') {
      return text('sentence.nftTransfer', {
        collection: label(meta),
        id: view.hero && view.hero.id,
        to: to ? label(to.identity) : '—',
      });
    }
    if (!send) {
      if (kind === 'approve') {
        var cap = rows.find(function (r) { return r.amount; });
        if (cap && to) {
          return cap.amount.unlimited
            ? text('sentence.approveUnlimited', { spender: label(to.identity), symbol: cap.amount.symbol })
            : text('sentence.approve', { spender: label(to.identity), amount: amountPhrase(cap.amount) });
        }
      }
      return null;
    }
    var amount = amountPhrase(send.amount);
    if (kind === 'transfer' || kind === 'transferFrom') {
      return text('sentence.transfer', { amount: amount, to: to ? label(to.identity) : '—' });
    }
    if (kind === 'swap') {
      var receive = rows.find(function (r) { return r.role === 'receive-amount'; });
      return text('sentence.swap', { amount: amount, receive: receive ? amountPhrase(receive.amount) : '—' });
    }
    if (kind === 'vaultDeposit') return text('sentence.vaultDeposit', { amount: amount, vault: label(meta) });
    if (kind === 'vaultWithdraw') return text('sentence.vaultWithdraw', { amount: amount, vault: label(meta) });
    return null;
  }

  // --- nested calldata -------------------------------------------------------

  function applyNested(descriptor, values, call, ctx, view, meta) {
    var inner = descriptor.nested === 'multiSend'
      ? decodeMultiSend(values[0])
      : [{ to: values[0], value: values[1], data: values[2] }];
    view.hero = null;
    view.legs = inner.map(function (leg, i) {
      var legView = baseView(null, ctx);
      resolveCall({ to: leg.to, value: '0x' + BigInt(leg.value || 0n).toString(16), data: leg.data }, ctx, legView, true);
      return { index: i + 1, view: legView };
    });
    view.risk = view.legs.reduce(function (worst, leg) {
      return rank(leg.view.risk) > rank(worst) ? leg.view.risk : worst;
    }, 'caution');
    view.refuse = view.legs.some(function (leg) { return leg.view.refuse; });
    view.sentence = descriptor.nested === 'multiSend'
      ? text('sentence.multiSend', { count: inner.length })
      : text('sentence.safeExec');
    view.warnings.push({ tone: 'caution', key: 'warn.nested' });
    view.tech = techFor(descriptor.signature, values, call.data, {
      addresses: [{ roleKey: 'ui.outerContract', name: label(meta), address: call.to }],
    });
    return view;
  }

  // Safe's multiSend packs legs as: operation(1) ‖ to(20) ‖ value(32) ‖ len(32) ‖ data
  function decodeMultiSend(packed) {
    var body = abi.strip(packed);
    var legs = [];
    var i = 0;
    while (i + 170 <= body.length) {
      var length = Number(BigInt('0x' + body.slice(i + 106, i + 170)));
      legs.push({
        to: '0x' + body.slice(i + 2, i + 42),
        value: BigInt('0x' + body.slice(i + 42, i + 106)),
        data: '0x' + body.slice(i + 170, i + 170 + length * 2),
      });
      i += 170 + length * 2;
    }
    return legs;
  }

  // --- messages --------------------------------------------------------------

  function hexToText(hex) {
    var body = abi.strip(hex);
    var bytes = new Uint8Array(body.length / 2);
    for (var i = 0; i < bytes.length; i++) bytes[i] = parseInt(body.substr(i * 2, 2), 16);
    try {
      var decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
      // Control characters (tab/newline/CR excepted) mean these are not words.
      return UNREADABLE.test(decoded) ? null : decoded;
    } catch (e) {
      return null;
    }
  }

  function isHexPayload(payload) {
    if (typeof payload !== 'string' || !payload.startsWith('0x')) return false;
    var body = payload.slice(2);
    return body.length % 2 === 0 && /^[0-9a-fA-F]*$/.test(body);
  }

  function parseSiwe(body) {
    var lines = body.split('\n');
    var m = lines[0].match(/^(\S+) wants you to sign in with your Ethereum account:$/);
    if (!m) return null;
    return {
      domain: m[1],
      address: lines[1] || '',
      statement: lines[3] || '',
      nonce: (body.match(/^Nonce: (.*)$/m) || [])[1] || '',
    };
  }

  function resolvePersonalSign(intent, ctx, view) {
    var payload = intent.params[0];
    var body = isHexPayload(payload) ? hexToText(payload) : payload;
    view.fee = null; // nothing is charged for an off-chain signature

    if (body === null) {
      view.intentKey = 'intent.signData';
      view.level = 4;
      view.risk = 'danger';
      view.hero = { kind: 'hash', hex: payload };
      view.sentence = text('sentence.hexData');
      view.warnings.push({ tone: 'danger', key: 'warn.hexData' });
      view.tech = techFor('personal_sign', null, payload, {});
      return view;
    }

    var siwe = parseSiwe(body);
    if (siwe) {
      var origin = (intent.origin || '').replace(/^https?:\/\//, '').replace(/\/.*$/, '');
      var mismatch = siwe.domain.toLowerCase() !== origin.toLowerCase();
      view.intentKey = 'intent.login';
      view.hero = { kind: 'siwe', domain: siwe.domain, statement: siwe.statement, mismatch: mismatch };
      view.fields = [
        { label: 'field.claimedDomain', value: siwe.domain, format: 'raw', warning: mismatch },
        { label: 'field.actualSite', value: origin, format: 'raw', warning: mismatch },
        { label: 'field.nonce', value: siwe.nonce, format: 'raw', detail: true },
      ];
      if (mismatch) {
        view.risk = 'danger';
        view.sentence = text('sentence.siwePhish', { claimed: siwe.domain, actual: origin });
        view.warnings.push({ tone: 'danger', key: 'warn.siweMismatch' });
      } else {
        view.risk = 'safe';
        view.sentence = text('sentence.siweOk', { domain: siwe.domain });
      }
      view.tech = techFor('personal_sign (SIWE)', null, payload, {});
      return view;
    }

    view.intentKey = 'intent.message';
    view.hero = { kind: 'message', text: body };
    view.sentence = text('sentence.message');
    view.tech = techFor('personal_sign (UTF-8)', null, payload, {});
    return view;
  }

  function resolveEthSign(intent, ctx, view) {
    view.intentKey = 'intent.blind';
    view.risk = 'hard-danger';
    view.level = 6;
    view.fee = null; // nothing is charged for an off-chain signature
    view.hero = { kind: 'hash', hex: intent.params[1] || intent.params[0] };
    view.sentence = text('sentence.ethSign');
    view.warnings.push({ tone: 'danger', key: 'warn.ethSign' });
    view.tech = techFor('eth_sign (raw 32-byte hash)', null, intent.params[1] || intent.params[0], {});
    return view;
  }

  function resolveTypedData(intent, ctx, view) {
    var raw = intent.params[1];
    var data = typeof raw === 'string' ? JSON.parse(raw) : raw;
    var primary = data.primaryType;
    var message = data.message || {};
    var domain = data.domain || {};
    view.fee = null; // nothing is charged for an off-chain signature
    view.contract = identify(domain.verifyingContract || '', ctx);

    if (primary === 'Permit') {
      var amount = tokenAmount(BigInt(message.value), domain.verifyingContract, ctx);
      var spender = identify(message.spender, ctx);
      var deadline = Number(message.deadline);
      var expired = deadline * 1000 < (ctx.now || Date.now());
      view.intentKey = 'intent.permit';
      view.hero = { kind: 'amount', amount: amount, direction: 'allowance' };
      view.fields = [
        { label: 'field.spender', identity: spender, format: 'addressName', role: 'spender' },
        { label: 'field.validUntil', value: formatDate(deadline), format: 'date', expired: expired },
        { label: 'field.token', value: amount.token.name || amount.symbol, format: 'raw' },
      ];
      view.sentence = text('sentence.permit', {
        spender: label(spender),
        deadline: formatDate(deadline),
        amount: amountPhrase(amount),
      });
      if (amount.unlimited) {
        view.risk = 'danger';
        view.warnings.push({ tone: 'danger', key: 'warn.unlimitedOffline' });
        view.refuse = true;
      } else {
        view.risk = 'caution';
        view.warnings.push({ tone: 'caution', key: 'warn.offchainNoTrace' });
      }
      if (expired) view.warnings.push({ tone: 'caution', key: 'warn.expired' });
      view.tech = techFor('EIP-712 · ' + primary, null, null, {
        params: Object.keys(message).map(function (k) { return { name: k, value: renderRaw(message[k]) }; }),
        addresses: [{ roleKey: 'ui.verifyingContract', name: label(view.contract), address: domain.verifyingContract }],
      });
      return view;
    }

    if (primary === 'PermitSingle' || primary === 'PermitTransferFrom') {
      var details = message.details || message.permitted || {};
      var pAmount = tokenAmount(BigInt(details.amount), details.token, ctx);
      var pSpender = identify(message.spender, ctx);
      view.intentKey = 'intent.permit2';
      view.hero = { kind: 'amount', amount: pAmount, direction: 'allowance' };
      view.fields = [
        { label: 'field.spender', identity: pSpender, format: 'addressName', role: 'spender' },
        { label: 'field.token', value: pAmount.token.name || pAmount.symbol, format: 'raw' },
        { label: 'field.validUntil', value: formatDate(details.expiration || message.deadline), format: 'date' },
      ];
      view.risk = pAmount.unlimited ? 'danger' : 'caution';
      view.sentence = pAmount.unlimited
        ? text('sentence.permit2Unlimited', { spender: label(pSpender), symbol: pAmount.symbol })
        : text('sentence.permit2Limited', { spender: label(pSpender), amount: amountPhrase(pAmount) });
      if (pAmount.unlimited) {
        view.warnings.push({ tone: 'danger', key: 'warn.unlimitedOffline' });
        view.refuse = true;
      }
      view.tech = techFor('EIP-712 · ' + primary, null, null, {
        params: [
          { name: 'token', value: details.token },
          { name: 'amount', value: String(details.amount) },
          { name: 'spender', value: message.spender },
          { name: 'expiration', value: String(details.expiration || message.deadline) },
        ],
        addresses: [{ roleKey: 'ui.verifyingContract', name: 'Permit2', address: domain.verifyingContract }],
      });
      return view;
    }

    view.intentKey = 'intent.typed';
    view.level = 2;
    view.risk = 'caution';
    view.hero = { kind: 'code', title: primary, sub: domain.name || null };
    view.fields = Object.keys(message).map(function (key) {
      return { labelRaw: key, value: renderRaw(message[key]), format: 'raw' };
    });
    view.sentence = text('sentence.typedUnknown', { type: primary });
    view.warnings.push({ tone: 'caution', key: 'warn.typedUnknown' });
    view.tech = techFor('EIP-712 · ' + primary, null, null, {
      params: Object.keys(message).map(function (k) { return { name: k, value: renderRaw(message[k]) }; }),
      addresses: domain.verifyingContract
        ? [{ roleKey: 'ui.verifyingContract', name: label(view.contract), address: domain.verifyingContract }]
        : [],
    });
    return view;
  }

  // --- entry point -----------------------------------------------------------

  /**
   * The call the site asked for must actually be inside the operation about to
   * be signed. A wallet that quietly swapped the recipient while assembling
   * would otherwise pass unnoticed.
   */
  function containsRequested(calls, intent) {
    if (intent.method === 'eth_sendTransaction') {
      return calls.some(function (call) { return ns.safeop.sameCall(call, intent.params[0]); });
    }
    if (intent.method === 'wallet_sendCalls') {
      return (intent.params[0].calls || []).every(function (wanted) {
        return calls.some(function (call) { return ns.safeop.sameCall(call, wanted); });
      });
    }
    return true;
  }

  /**
   * When an assembled operation is present, IT is the subject: the digest
   * covers its calldata, so that is what must be on screen. The intent the
   * site sent is then only used to check that its call is really in there.
   */
  function operationSubject(intent, ctx, view) {
    var operation = ctx.operation && ctx.operation.userOp;
    if (!operation) return intent;

    var decoded = ns.safeop.decodeCallData(operation.callData);
    if (!decoded) {
      view.refuse = true;
      view.warnings.push({ tone: 'danger', key: 'refuse.opUnreadable' });
      return intent;
    }
    view.fee = ns.fee.derive(ctx, decoded.calls);
    if (!containsRequested(decoded.calls, intent)) {
      view.refuse = true;
      view.warnings.push({ tone: 'danger', key: 'refuse.opMismatch' });
    }
    return decoded.calls.length === 1
      ? { method: 'eth_sendTransaction', origin: intent.origin, params: [decoded.calls[0]] }
      : { method: 'wallet_sendCalls', origin: intent.origin, params: [{ calls: decoded.calls }] };
  }

  function resolve(requested, ctx) {
    ctx = ctx || {};
    var view = baseView(requested, ctx);
    var intent = operationSubject(requested, ctx, view);
    view.method = intent.method;

    if (intent.method === 'eth_sendTransaction') {
      resolveCall(intent.params[0], ctx, view, false);
    } else if (intent.method === 'wallet_sendCalls') {
      var payload = intent.params[0];
      view.intentKey = 'intent.batch';
      view.hero = null;
      view.legs = payload.calls.map(function (leg, i) {
        var legView = baseView(intent, ctx);
        resolveCall(leg, ctx, legView, true);
        return { index: i + 1, view: legView };
      });
      view.sentence = text('sentence.batch', { count: payload.calls.length });
      view.risk = view.legs.reduce(function (worst, leg) {
        return rank(leg.view.risk) > rank(worst) ? leg.view.risk : worst;
      }, 'normal');
      view.refuse = view.legs.some(function (leg) { return leg.view.refuse; });
      view.legs.forEach(function (leg) {
        leg.view.warnings.forEach(function (w) {
          if (w.tone === 'danger') view.warnings.push(w);
        });
      });
      if (ctx.simulation) view.balance = { rows: ctx.simulation.rows, note: ctx.simulation.note, claimed: true };
      view.tech = techFor('wallet_sendCalls (EIP-5792)', null, null, {});
    } else if (intent.method === 'personal_sign') {
      resolvePersonalSign(intent, ctx, view);
    } else if (intent.method === 'eth_sign') {
      resolveEthSign(intent, ctx, view);
    } else if (intent.method.indexOf('signTypedData') >= 0) {
      resolveTypedData(intent, ctx, view);
    } else {
      view.intentKey = 'intent.unknownRequest';
      view.risk = 'danger';
      view.sentence = text('sentence.unknownMethod', { method: intent.method });
      view.refuse = true;
    }

    if (!view.dapp.originVerified && view.dapp.origin) {
      view.warnings.push({ tone: 'caution', key: 'warn.claimedOrigin' });
    }
    return view;
  }

  ns.resolve = resolve;
  ns.format = { units: formatUnits, date: formatDate, short: shortAddress, group: group, amountPhrase: amountPhrase };
})(window.VelaCS);
