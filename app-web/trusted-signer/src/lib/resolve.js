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

  // Where "unlimited" starts — the wallet's own two lines
  // (vela-core `approval_guard::UNLIMITED_CAP_256` / `_160`), so this page and
  // the app call the same amount unlimited. 2^200 for a uint256 amount: past
  // every real supply (about 2^128) and below every "max" sentinel (2^255,
  // 2^256-1). 2^152 for a Permit2 uint160 amount, whose own max is 2^160-1.
  var UNLIMITED_256 = 1n << 200n;
  var UNLIMITED_160 = 1n << 152n;


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

  // Fiat only at a rate the REQUESTER gave (`context.rates`), in its
  // currency — the fee's rule (lib/fee.js). A price this page carried itself
  // would be a guess shown as a fact, and wrong for every coin it forgot.
  //
  // Both halves are the requester's, so the figure is drawn as its claim
  // (render.js puts `tag.rateByRequester` beside every fiat figure), and the
  // currency must be an ISO 4217 code: free text there was a second line of
  // words the requester could write onto the card ("≈ <anything>1,000").
  var ISO_CURRENCY = /^[A-Z]{3}$/;

  function fiat(ctx, symbol, value, decimals) {
    var currency = ctx && ctx.currency;
    if (typeof currency !== 'string' || !ISO_CURRENCY.test(currency)) return null;
    var rates = ctx.rates;
    var rate = rates && typeof rates === 'object' && Object.prototype.hasOwnProperty.call(rates, symbol)
      ? rates[symbol] : undefined;
    if (typeof rate !== 'number' || !isFinite(rate) || rate <= 0) return null;
    var amount = (Number(value) / Math.pow(10, decimals)) * rate;
    if (!isFinite(amount)) return null;
    return '≈ ' + currency + ' ' +
      (amount >= 1000 ? group(Math.round(amount).toString()) : amount.toFixed(2));
  }

  // Characters that change how text reads without being seen: direction
  // overrides and isolates ("bob" U+202E "evil" reads "bob live") and zero-width marks. They
  // are drawn as their code points instead, and the card says why.
  var INVISIBLE = /[\u00ad\u061c\u180e\u200b-\u200f\u202a-\u202e\u2060-\u2064\u2066-\u2069\ufeff]/g;

  /** `{text, hidden}`: the text with every invisible character made visible. */
  function reveal(value) {
    if (typeof value !== 'string') return { text: value, hidden: false };
    var hidden = false;
    var shown = value.replace(INVISIBLE, function (c) {
      hidden = true;
      return '[U+' + c.charCodeAt(0).toString(16).toUpperCase().padStart(4, '0') + ']';
    });
    return { text: shown, hidden: hidden };
  }

  /** `value` made visible; a caution on `view` when anything was hidden in it. */
  function visible(value, view) {
    var out = reveal(value);
    if (out.hidden && !view.warnings.some(function (w) { return w.key === 'warn.invisibleChars'; })) {
      view.warnings.push({ tone: 'caution', key: 'warn.invisibleChars' });
      raise(view, 'caution');
    }
    return out.text;
  }

  /**
   * THE short form of an address (vela-core `multisig_rules::names::
   * short_address`, r12-7): EIP-55 casing, `0x` + 6, `…`, 6 — the same on
   * this page as in every Vela shell. Six and six because a 4+4 look-alike
   * is a 2^32 search for an address-poisoning sender and 6+6 is 2^48.
   */
  function eip55(address) {
    var raw = String(address == null ? '' : address);
    if (!/^0x[0-9a-fA-F]{40}$/.test(raw)) return raw;
    var body = raw.slice(2).toLowerCase();
    var hash = ns.keccak.hex(new TextEncoder().encode(body)).slice(2);
    var out = '0x';
    for (var i = 0; i < body.length; i++) {
      out += parseInt(hash[i], 16) >= 8 ? body[i].toUpperCase() : body[i];
    }
    return out;
  }

  function shortAddress(address) {
    var full = eip55(address);
    var chars = Array.from(full);
    if (chars.length <= 14) return full;
    return chars.slice(0, 8).join('') + '…' + chars.slice(-6).join('');
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

  // --- provenance ------------------------------------------------------------
  //
  // One line under the intent says where its reading came from, in the words
  // the web wallet uses (owner, 2026-10-28), so a person can always tell what
  // is CERTAIN from what is INTERPRETED from what is CLAIMED:
  //
  //   read      ✓ read by this page from the bytes that get signed
  //   described △ a descriptor's reading — a description, not a guarantee
  //               that the contract does what it says
  //   fourbyte  △ only a function name, from a public database
  //   shape     △ the call has the shape of something it may not be
  //   claimed   ○ the only reading is the app's own word (its simulation)
  //
  // `whatKey` names the resemblance; `contract` the descriptor's contract.

  function provenance(view, kind, extra) {
    view.provenance = Object.assign({ kind: kind }, extra || {});
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
    // On the chain the signature is for: a name from another network's table
    // is a different contract, or none (cs-13).
    var known = reg.contract(lower, ctx.chainId);
    var tag = null;
    var self = ctx.selfAccount;

    if (self && lower === String(self).toLowerCase()) tag = ctx.selfTag || 'tag.self';
    else if (hints.isCallTarget) tag = 'tag.calledContract';
    else if (known && known.descriptor) tag = 'tag.localDescriptor';
    else if (known && known.verified) tag = 'tag.sourceVerified';

    return {
      address: address,
      short: shortAddress(address),
      // Local, deterministic, and impossible for a requester to influence.
      identicon: address ? ns.identicon.forAddress(address) : null,
      tag: tag,
      kind: tag === 'tag.self' || (tag && tag === ctx.selfTag) ? 'own' : known ? 'contract' : 'unknown',
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

  /** `bits`: the amount field's width — 160 for Permit2's, otherwise 256. */
  function tokenAmount(value, tokenAddress, ctx, bits) {
    var known = reg.token(tokenAddress, ctx && ctx.chainId);
    // A token this page does not know on this chain has no decimals it can
    // vouch for. Guessing 18 drew 250 USDC as "0" (spec 102); the figure is
    // the integer in the calldata instead — every digit, in the token's own
    // smallest units — and `warn.unverifiedDecimals` says so.
    var token = known || { symbol: '?', nameKey: 'value.unknownToken', decimals: 0, tone: '#8a93a5' };
    var unlimited = value >= (bits === 160 ? UNLIMITED_160 : UNLIMITED_256);
    return {
      unlimited: unlimited,
      value: value,
      token: token,
      tokenAddress: tokenAddress,
      // r9-10: an unknown token's figure is its base units, said as such —
      // "1000000 base units · decimals unknown", as the app's review says
      // it — never "1,000,000 ? ?".
      text: unlimited ? null : known ? formatUnits(value, token.decimals) : value.toString(),
      textKey: unlimited ? 'value.unlimited' : null,
      symbol: known ? token.symbol : ns.i18n.t('value.baseUnitsUnknown'),
      fiat: unlimited ? null : fiat(ctx, token.symbol, value, token.decimals),
      unverifiedDecimals: !known,
      // Decoration only — see lib/logos.js.
      logos: ctx ? ns.logos.token(ctx, ctx.chainId, tokenAddress) : [],
    };
  }

  /**
   * `exact`: every one of the 18 decimals that is not a trailing zero, as the
   * wallet's plain send card states it (vela-core `exact_native_amount`, 082
   * RC5). Without it the figure stops at six decimals.
   */
  /**
   * The chain's own coin, from the chain id we sign for (it is in the
   * digest). Only for a chain this page does not know is the symbol the
   * requester's `nativeSymbol` — and then it is the app's word, marked as
   * such wherever it is drawn (`claimed`), exactly as the chain's name is.
   * It is not a fact the page can check: "ETH" on a chain whose coin is
   * worth nothing reads as a price it does not have. Nobody naming it is
   * "?", the page's mark for a coin it cannot name, never a guessed "ETH".
   * `resolve` puts the result on its own copy of the context (`ctx.coin`),
   * so the hero, the rows and the fee (lib/fee.js) all say the same.
   */
  function coinOf(ctx) {
    var known = reg.nativeSymbol(ctx.chainId);
    if (known) return { symbol: known, claimed: false, hidden: false };
    var given = typeof ctx.nativeSymbol === 'string' ? ctx.nativeSymbol.trim().slice(0, 12) : '';
    if (!given) return { symbol: '?', claimed: false, hidden: false };
    var shown = reveal(given);
    return { symbol: shown.text, claimed: true, hidden: shown.hidden };
  }

  function nativeAmount(value, ctx, exact) {
    var coin = ctx.coin || coinOf(ctx);
    var symbol = coin.symbol;
    return {
      value: value,
      token: { symbol: symbol, decimals: 18, tone: '#8a93a5' },
      symbol: symbol,
      symbolClaimed: coin.claimed,
      text: formatUnits(value, 18, exact ? 18 : undefined),
      fiat: fiat(ctx, symbol, value, 18),
      unlimited: false,
    };
  }

  /**
   * The account a "this account" tag may name: the one the signature is FOR,
   * as the digest fixes it — the operation's `userOp.sender` for a
   * transaction (SafeOp.safe), or the account a message is wrapped under
   * (SafeMessage, lib/digest.js). `context.account` alone is the requester's
   * word, and with an operation present it is ignored: an `account` naming
   * the recipient drew a stranger as "this account" (and its send as safe).
   * A transaction with no operation has no account in any digest, so nothing
   * is tagged.
   */
  function selfAccountOf(intent, ctx) {
    var operation = ctx.operation && ctx.operation.userOp;
    if (operation) return typeof operation.sender === 'string' ? operation.sender : null;
    var method = intent && intent.method;
    if (method === 'eth_sendTransaction' || method === 'wallet_sendCalls') return null;
    // digest.js wraps a message only when it has both.
    return typeof ctx.account === 'string' && ctx.chainId ? ctx.account : null;
  }

  /** "1,000 USDC" / "Unlimited USDC" — for interpolation into a sentence. */
  function amountPhrase(amount) {
    var head = amount.textKey ? ns.i18n.t(amount.textKey) : amount.text;
    return head + ' ' + amount.symbol;
  }

  // --- view scaffold ---------------------------------------------------------

  /**
   * Whether a site's name and its host are the same words, by the apps' rule
   * (vela-core `browser_load::site_label`, spec 079 F14): both trimmed, then
   * equal ignoring ASCII case. The wallet names a site it opened by its host,
   * so without this the head read "127.0.0.1:8137" over "127.0.0.1:8137".
   */
  function sameSiteWords(name, host) {
    var fold = function (text) {
      return String(text || '').trim().replace(/[A-Z]/g, function (c) { return c.toLowerCase(); });
    };
    return fold(name) === fold(host);
  }

  /**
   * Whether the site's name in `context.dapp` may stand as the head's own
   * words. Only when it says nothing the page cannot see for itself: the
   * name IS the host (what the wallet sends — vela-core `trusted_signer.rs`
   * names a site by its host), or a channel the browser vouches for shows a
   * Vela wallet asking. `source: 'vela_browser'` is a field anyone can write,
   * so it proves nothing: "Vela Wallet (verified)" over app.uniswap.org used
   * to be the head's title. Any other name is the site's claim, said as one
   * (`dapp.claimedName`).
   */
  function nameVouched(name, host, ctx) {
    if (host && sameSiteWords(name, host)) return true;
    return ctx.channel === 'post' && ctx.originVerified === true && walletRequester(ctx);
  }

  function baseView(intent, ctx) {
    var origin = (intent && intent.origin) || '';
    var host = origin.replace(/^https?:\/\//, '').replace(/\/.*$/, '');
    var known = ctx.dapp && typeof ctx.dapp === 'object' ? ctx.dapp : null;
    var given = known && typeof known.name === 'string' && known.name.trim() ? known.name : null;
    var vouched = !!given && nameVouched(given, host, ctx);
    var warnings = [];
    var shown = vouched ? reveal(given) : null;
    var claimed = given && !vouched ? reveal(given.trim().slice(0, 64)) : null;
    var accountName = reveal((ctx.signer && typeof ctx.signer.name === 'string' && ctx.signer.name) || null);
    var chainKnown = !!reg.chainName(ctx.chainId);
    var chainGiven = !chainKnown && typeof ctx.chainName === 'string' && ctx.chainName.trim()
      ? reveal(ctx.chainName.trim().slice(0, 40)) : null;
    var coin = ctx.coin || coinOf(ctx);
    if ((shown && shown.hidden) || (claimed && claimed.hidden) || accountName.hidden ||
      (chainGiven && chainGiven.hidden) || coin.hidden) {
      warnings.push({ tone: 'caution', key: 'warn.invisibleChars' });
    }
    var name = shown ? shown.text : null;
    return {
      dapp: {
        name: name,
        nameKey: name ? null : (host || claimed ? 'tag.unknownSite' : 'tag.wallet'),
        // A name the site gave that the page cannot vouch for: drawn under
        // the head as the site's own words, never as its title.
        claimedName: claimed ? claimed.text : null,
        // The host, for every judgement about it (`warn.claimedOrigin` reads
        // this). Whether the head draws it as a line of its own is
        // `originShown`: not when the name above already says it (082 L-HOST).
        origin: host || null,
        originShown: !!host && !sameSiteWords(name || '', host),
        // The wallet asking itself has no host: its initial is the word's.
        letter: (name || host || ns.i18n.t('tag.wallet') || '?').charAt(0).toUpperCase(),
        tone: (vouched && known.tone) || '#8a93a5',
        // Display-only, and it never touches the digest: a logo cannot change
        // what gets signed, so an https URL from the request is allowed here.
        icon: (known && known.icon) || null,
        // Only a channel the browser itself vouches for (postMessage
        // messaging) proves who is asking. Everything else is the requester's
        // own word, and has to be shown as such — a site a wallet relays
        // included (`relayedSite`).
        originVerified: ctx.originVerified === true && ctx.siteRelayed !== true,
        // No site at all: the wallet's own send. The card then names no
        // requester (the apps' `first_party`), rather than a generic
        // "Wallet" line that says nothing a person can check.
        own: !host && !given,
      },
      // Named from the chain id we are actually signing for (it is in the
      // digest's domain). The app's own label is only used for a chain this
      // page does not know, and is marked as the app's. A chain this page
      // does not know always shows its id (`chainId`) — the one thing about
      // it that is signed — and, when the app named its coin, that the
      // symbol is the app's word too (`coin.claimed`).
      chain: chainKnown ? reg.chainName(ctx.chainId) : chainGiven ? chainGiven.text : null,
      chainClaimed: !!chainGiven,
      chainId: chainKnown ? null : String(ctx.chainId || '?'),
      coin: coin,
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
      accountName: accountName.text,
      fields: [],
      warnings: warnings,
      legs: [],
      risk: warnings.length ? 'caution' : 'normal',
      level: 1,
      signer: ctx.signer || { name: '—', letter: '?' },
      fee: null,
      // Where the intent's reading came from (`provenance`); none for a
      // request nothing could be read from, and for a key ceremony.
      provenance: null,
    };
  }

  function techFor(signature, values, calldata, extra) {
    return {
      signature: signature,
      params: (extra && extra.params) || [],
      addresses: (extra && extra.addresses) || [],
      sim: extra && extra.sim,
      // A simulation summary from the context is the requester's, and the
      // details say so beside it; `ui.simNoOther` is this page's own words.
      simClaimed: !!(extra && extra.simClaimed && extra.sim),
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

  /**
   * Whether a call carries no calldata: absent, "", "0x" or "0X", after
   * trimming whitespace — the wallet's own test (vela-core
   * `clear_signing::is_empty_calldata`, 082 RC1). Such a call runs no
   * function: it is a plain send of the native coin, whatever the recipient
   * is (RC2) and whatever the value, 0 included (RC8). Only a string can
   * carry calldata; anything else that is not absent is read as calldata and
   * goes down the blind ladder, as before.
   */
  function isEmptyCalldata(data) {
    if (data === undefined || data === null) return true;
    if (typeof data !== 'string') return false;
    var body = data.trim();
    return body === '' || body.toLowerCase() === '0x';
  }

  /**
   * A call's value in wei (`abi.quantity`, the wallet's submit reading).
   * Absent, "" and "0x" are zero (vela-core RC4). `null` when the page cannot
   * read it ("abc", "-1", 1.5): no figure is printed for it, and it matches no
   * call of the operation. BigInt() throwing here used to take the whole page
   * down — no card, no refusal.
   */
  function callValue(value) {
    return abi.quantity(value);
  }

  function resolveCall(call, ctx, view, isLeg) {
    var to = call.to || '';
    var data = call.data || '0x';
    var value = callValue(call.value);
    var selector = abi.selectorOf(data);
    var hasCalldata = !isEmptyCalldata(call.data);
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
      provenance(view, 'read');
      view.warnings.push({ tone: 'caution', key: 'warn.deploy' });
      view.tech = techFor(text('value.contractCreation'), null, data, {});
      nativeOut(view, value, ctx);
      return view;
    }

    // Every call with no calldata is a send, value 0 included: "Send · 0 xDAI
    // · Recipient", never the blind ladder's red "nothing decodes" (082 G14,
    // RC8). The ladder below sees calldata — or a value no chain carries
    // (negative) or the page cannot read, which the wallet's own rule refuses
    // to print (RC4). The figure is exact (RC5): with "0" meaning nothing
    // moves, a few wei must not read "0" too.
    if (!hasCalldata && value !== null && value >= 0n) {
      var amount = nativeAmount(value, ctx, true);
      var moves = value > 0n;
      view.intentKey = 'intent.send';
      provenance(view, 'read');
      // Nothing leaves at 0, so the hero is not an outgoing amount.
      view.hero = { kind: 'amount', amount: amount, direction: moves ? 'out' : 'none' };
      view.fields.push({ label: 'field.recipient', identity: meta, format: 'addressName', role: 'recipient' });
      if (meta.kind === 'own') {
        view.risk = 'safe';
        view.sentence = text('sentence.sendSelf', { amount: amount.text + ' ' + amount.symbol });
      } else {
        // The sentence names the address, not a label: "to 0x9A8b…1a09".
        view.sentence = text(multisigFunds(ctx) ? 'sentence.msSends' : 'sentence.transfer', {
          amount: amount.text + ' ' + amount.symbol, to: meta.short,
        });
      }
      view.tech = techFor(text('value.noCalldata'), null, null, {
        params: [{ name: 'value', value: amount.text + ' ' + amount.symbol }],
        addresses: [{ roleKey: 'field.recipient', name: label(meta), address: to }],
        // The "−amount · nothing else changes" line is a statement about what
        // leaves. At 0 there is no minus to write, and "nothing else changes"
        // is the simulation's to say, not this page's: a recipient's code
        // still runs.
        sim: moves ? text('ui.simNoOther', { delta: '-' + amount.text + ' ' + amount.symbol }) : undefined,
      });
      return view;
    }

    resolveCalldata(call, ctx, view, isLeg, meta, data, selector, value);
    nativeOut(view, value, ctx);
    return view;
  }

  /**
   * Native coin sent along with calldata leaves the account exactly as a
   * send's does, whatever the function: `deposit()` with 5 ETH, a selector
   * nobody knows, a `transfer` that also carries value. Only a plain send
   * (no calldata) and a descriptor that already draws it (`nativeIn`, a swap
   * paid in the coin) had shown it; everywhere else it sat in the operation
   * and nowhere on the card. Now it is a row of its own in the amount's
   * strong type, with a caution in words — in every leg of a batch too,
   * which carries the caution up to the card (resolve, applyNested).
   */
  function nativeOut(view, value, ctx) {
    if (value === null || value <= 0n || view.nativeShown) return;
    var amount = nativeAmount(value, ctx, true);
    var phrase = amount.text + ' ' + amount.symbol;
    view.nativeShown = true;
    view.fields.unshift({ label: 'field.nativeAttached', amount: amount, format: 'tokenAmount', role: 'native-out' });
    view.warnings.push({ tone: 'caution', key: whose('warn.nativeValue', ctx), params: { amount: phrase } });
    raise(view, 'caution');
    if (view.tech) view.tech.params.unshift({ name: 'value', value: phrase });
  }

  /**
   * An undecoded call drawn as a leg of a batch (spec: the universal
   * fallback, L3 at least). A leg has no hero, so without these rows it read
   * "2 · Blind signature" and nothing else: the contract it calls, the
   * function, the coin it carries and the calldata itself are all facts the
   * page holds, and all of them go on the leg.
   */
  function legFacts(view, meta, functionText, data, value, ctx, raw) {
    var facts = [
      { label: 'ui.contract', identity: meta, format: 'addressName', role: 'contract' },
      { label: 'ui.function', value: functionText, format: 'raw', mono: true },
    ];
    // A coin it carries is `nativeOut`'s row; none is said as 0, not left out.
    if (raw && value !== null && value === 0n) {
      var none = nativeAmount(0n, ctx, true);
      facts.push({
        label: 'field.nativeAttached', value: none.text + ' ' + none.symbol, format: 'raw',
        tagKey: none.symbolClaimed ? 'tag.coinByApp' : undefined,
      });
    }
    if (raw) {
      facts.push({
        label: 'ui.rawData', labelParams: { bytes: abi.byteLength(data) }, value: data, format: 'raw', mono: true,
      });
    }
    view.fields = facts.concat(view.fields);
  }

  function resolveCalldata(call, ctx, view, isLeg, meta, data, selector, value) {
    var to = call.to || '';
    var descriptor = selector ? reg.descriptorFor(selector) : null;
    // What the descriptor knows about THIS contract on THIS chain — null when
    // it was written for another one (cs-4). A token standard's is `{}`.
    var binding = descriptor ? reg.binding(descriptor, to, ctx.chainId) : null;

    // Level 1 — a token standard, or a descriptor written for this contract,
    // gives us meaning.
    if (descriptor && !descriptor.abiOnly && binding) {
      var values = abi.decode(descriptor.signature, data);
      if (values) return applyDescriptor(descriptor, values, call, ctx, view, meta, value, isLeg, binding);
    }

    // Level 2 — source verified, ABI decodes, nobody wrote down what it means.
    if (descriptor && descriptor.abiOnly && binding) {
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
        sim: claimedSim(ctx, view), simClaimed: true,
      });
      if (isLeg) legFacts(view, meta, parsed.name + '(…) · ' + selector, data, value, ctx, false);
      return view;
    }

    // Level 3 — a protocol's function, on a contract its descriptor was not
    // written for: the shape of a swap is not a swap.
    if (descriptor && descriptor.shape && !descriptor.abiOnly) {
      var shaped = abi.decode(descriptor.signature, data);
      if (shaped) return applyShape(descriptor, shaped, call, ctx, view, meta, data, selector, value, isLeg);
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
      provenance(view, 'fourbyte');
      view.tech = techFor(known4, v4, data, {
        params: (v4 || []).map(function (v, i) { return { name: p4.types[i], value: renderRaw(v) }; }),
        addresses: [{ roleKey: 'ui.contract', name: label(meta), address: to }],
        sim: claimedSim(ctx, view), simClaimed: true,
      });
      if (isLeg) legFacts(view, meta, p4.name + '(…) · ' + selector, data, value, ctx, false);
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
      // The simulation is the app's, run where this page cannot see, so it
      // cannot make a call nobody can read any less blind (cs-6): the risk
      // stays where "nothing decodes" puts it. Its rows are drawn as its word.
      view.level = 4;
      view.risk = 'danger';
      view.sentence = text('sentence.blindSim');
      view.warnings.push({ tone: 'danger', key: 'warn.blindSim' });
    } else {
      view.level = 6;
      view.risk = 'danger';
      view.sentence = text('sentence.blindNothing');
      view.warnings.push({ tone: 'danger', key: 'warn.blindNothing' });
    }
    if (sim) provenance(view, 'claimed');
    if (sim && sim.rows) view.balance = claimedBalance(sim, view);
    view.tech = techFor(
      selector ? text('value.unknownFunction', { selector: selector }) : text('value.emptyCalldata'),
      null, data,
      { addresses: [{ roleKey: 'ui.contract', name: label(meta), address: to }], sim: claimedSim(ctx, view), simClaimed: true },
    );
    if (isLeg) legFacts(view, meta, selector || '0x', data, value, ctx, true);
    return view;
  }

  function resolveToken(field, values, call, binding) {
    if (field.tokenPath) {
      var path = field.tokenPath;
      if (path[path.length - 1] === 'last') {
        var list = abi.at(values, path.slice(0, -1));
        return list && list[list.length - 1];
      }
      return abi.at(values, path);
    }
    // The vault's asset is the descriptor's word for the vault it was written
    // for. Nothing else names it: `context.vaultAsset` is the app's (cs-4),
    // and without a binding the amount is raw units of an unknown token.
    if (field.token === 'vaultAsset') return (binding && binding.asset) || null;
    return call.to;
  }

  function applyDescriptor(descriptor, values, call, ctx, view, meta, callValue, isLeg, binding) {
    view.intentKey = 'intent.' + descriptor.kind;
    view.kind = descriptor.kind;
    view.level = 1;
    view.contract = meta;
    // A token standard is read from the bytes of a token this page knows on
    // this chain; on any other contract the standard's shape is all there
    // is. A protocol's descriptor is a description of the contract it was
    // written for — not a promise that contract keeps it.
    if (descriptor.standard) {
      if (reg.token(call.to, ctx.chainId)) provenance(view, 'read');
      else provenance(view, 'shape', { whatKey: descriptor.standard === 'erc721' ? 'shape.erc721' : 'shape.erc20' });
    } else {
      var known = reg.contract(call.to, ctx.chainId);
      provenance(view, 'described', { contract: (known && known.name) || label(meta) });
    }

    if (descriptor.nested) return applyNested(descriptor, values, call, ctx, view, meta, isLeg);

    var hero = null;
    var rows = [];
    var approval = null;

    descriptor.fields.forEach(function (field) {
      var raw = abi.at(values, field.path);
      if (field.format === 'tokenAmount') {
        var amount = tokenAmount(raw, resolveToken(field, values, call, binding), ctx);
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
        hero = { kind: 'nft', collection: reg.token(call.to, ctx.chainId), id: raw.toString() };
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
        rows.push({
          label: field.label, value: na.text + ' ' + na.symbol, format: 'raw',
          tagKey: na.symbolClaimed ? 'tag.coinByApp' : undefined,
        });
      }
    });

    if (descriptor.nativeIn && callValue > 0n) {
      view.nativeShown = true;
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

    if (approval) applyApprovalGuard(approval, view, ctx, rows);
    applyRecipientGuards(call, view, rows);
    outputGuards(descriptor, values, call, ctx, view, binding);

    if (!view.sentence) view.sentence = sentenceFor(descriptor, rows, meta, view, ctx);

    var parsed2 = abi.parseSignature(descriptor.signature);
    view.tech = techFor(descriptor.signature, values, call.data, {
      params: values.map(function (v, i) { return { name: parsed2.types[i], value: renderRaw(v) }; }),
      addresses: addressesIn(rows, call, meta),
      sim: claimedSim(ctx, view), simClaimed: true,
    });
    if (ctx.simulation && ctx.simulation.rows && !isLeg) view.balance = claimedBalance(ctx.simulation, view);
    return view;
  }

  /**
   * A protocol's function on a contract its descriptor was not written for
   * (cs-4): `swap(…)` with 1inch's shape on an attacker's contract, ERC-4626's
   * `deposit` on something that is no vault. The same bytes are read the
   * same way — by type, every one of them — but nothing says what the
   * contract does with them: the intent is a contract call, the button the
   * certain action ("Sign"), and the resemblance a line of its own ("Looks
   * like a 1inch swap — shape only, not checked"). No token amount is
   * guessed for it, least of all from the app's `context.vaultAsset`.
   */
  function applyShape(descriptor, values, call, ctx, view, meta, data, selector, value, isLeg) {
    var parsed = abi.parseSignature(descriptor.signature);
    view.intentKey = 'intent.contractCall';
    view.level = 3;
    view.risk = 'caution';
    view.contract = meta;
    provenance(view, 'shape', { whatKey: descriptor.shape });
    // A batch's legs are still read from its bytes — they are what the call
    // hands on — under the same "contract call" and the same line.
    if (descriptor.nested) return applyNested(descriptor, values, call, ctx, view, meta, isLeg);
    view.hero = { kind: 'code', title: parsed.name + '(…)', sub: label(meta) };
    view.sentence = text('sentence.shapeOnly');
    view.fields = values.map(function (v, i) {
      // A tuple's full type is a line too long for a label; the details
      // keep it.
      var type = parsed.types[i];
      return { labelRaw: type.charAt(0) === '(' ? 'tuple' : type, value: renderRaw(v), format: 'raw' };
    });
    // Even as a resemblance, an address its bytes put where the output goes
    // is checked against the account, and a minimum of nothing is said.
    outputGuards(descriptor, values, call, ctx, view, null);
    view.tech = techFor(descriptor.signature, values, data, {
      params: values.map(function (v, i) { return { name: parsed.types[i], value: renderRaw(v) }; }),
      addresses: [{ roleKey: 'ui.contract', name: label(meta), address: call.to }],
      sim: claimedSim(ctx, view), simClaimed: true,
    });
    if (isLeg) legFacts(view, meta, parsed.name + '(…) · ' + selector, data, value, ctx, false);
    return view;
  }

  /**
   * Where a call's output goes, and how little it may be (cs-2), from the
   * descriptor's own fields:
   *
   *  · a `returns` address (a swap's recipient, a vault's shares-to) that is
   *    not the account signing — "You receive…" in green had stood over a
   *    swap whose output went to a stranger;
   *  · a minimum received that is next to nothing (`isDust`), which lets any
   *    price move or trade-ahead take almost all of it.
   *
   * "This account" is the signature's own (`ctx.selfAccount`): with no
   * account in the digest, nothing can be said either way.
   */
  function outputGuards(descriptor, values, call, ctx, view, binding) {
    var self = ctx.selfAccount ? String(ctx.selfAccount).toLowerCase() : null;
    descriptor.fields.forEach(function (field) {
      var raw = abi.at(values, field.path);
      if (field.returns && self && typeof raw === 'string' && raw.toLowerCase() !== self) {
        view.returnsElsewhere = raw;
        if (!view.warnings.some(function (w) { return w.key === 'warn.notThisAccount'; })) {
          view.warnings.push({ tone: 'danger', key: 'warn.notThisAccount', params: { to: shortAddress(raw) } });
        }
        raise(view, 'danger');
      }
      if (field.role === 'receive-amount' && typeof raw === 'bigint') {
        var amount = tokenAmount(raw, resolveToken(field, values, call, binding), ctx);
        if (isDust(amount)) {
          view.warnings.push({
            tone: 'danger',
            key: 'warn.dustMinimum',
            params: { amount: formatUnits(amount.value, amount.token.decimals, amount.token.decimals) + ' ' + amount.symbol },
          });
          raise(view, 'danger');
        }
      }
    });
  }

  /**
   * A minimum that protects nothing: at most a millionth of one whole token
   * (1 wei of an 18-decimal coin, 1 unit of USDC), or 0 or 1 of a token whose
   * decimals this page cannot check.
   */
  function isDust(amount) {
    if (amount.unlimited) return false;
    if (amount.unverifiedDecimals) return amount.value <= 1n;
    return amount.value * 1000000n <= 10n ** BigInt(amount.token.decimals);
  }

  /** The requester's simulation summary, made visible — its words, not ours. */
  function claimedSim(ctx, view) {
    var summary = ctx.simulation && ctx.simulation.summary;
    return typeof summary === 'string' && summary ? visible(summary, view) : undefined;
  }

  /** The requester's simulated balance change, drawn as its claim. */
  function claimedBalance(sim, view) {
    var rows = Array.isArray(sim.rows) ? sim.rows : [];
    return {
      rows: rows.map(function (row) {
        return { symbol: visible(String(row && row.symbol), view), delta: visible(String(row && row.delta), view) };
      }),
      note: typeof sim.note === 'string' ? visible(sim.note, view) : null,
      claimed: true,
    };
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

  // Approvals, under the no-editing rule: this page cannot rewrite the cap —
  // offering an editor would mean signing bytes other than the ones that
  // arrived. An UNLIMITED amount is signable as it stands (owner, 2026-09-26:
  // Permit2 is built on a standing approve(Permit2, MAX), and a smart account's
  // bundle spends it in the same batch, so capping it breaks the dApp), but it
  // is drawn as the danger it is, with the cap's absence said in words: the
  // wallet's own sheet is where a cap is chosen, before the request gets here.
  // An approve-ALL stays refused — the wallet only grants one on a deliberate
  // tap, and this page has no such tap.
  /**
   * The allowance the app says the spender has now, or null. It is the
   * app's word (cs-16): the page reads no chain, so a figure built on it is
   * drawn as the app's, and without one the page says it cannot see it.
   */
  function claimedAllowance(ctx) {
    if (ctx.currentAllowance === undefined || ctx.currentAllowance === null) return null;
    return abi.quantity(ctx.currentAllowance);
  }

  /**
   * r10: whose coins a card speaks of. On a multisig's transaction — a
   * member's approval, an executor's run of it — they are the multisig's,
   * never the signer's: "all of the multisig's USDC", not "all of your USDC".
   * `selfTag` is set by this page only (resolve clears the requester's).
   */
  function multisigFunds(ctx) {
    return !!ctx && ctx.selfTag === 'tag.thisMultisig';
  }

  function whose(key, ctx) {
    return multisigFunds(ctx) ? key + 'Ms' : key;
  }

  function applyApprovalGuard(approval, view, ctx) {
    if (approval.mode === 'all') {
      if (approval.all) {
        view.risk = 'danger';
        view.warnings.push({ tone: 'danger', key: whose('warn.approveAll', ctx) });
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
      view.warnings.push({ tone: 'danger', key: whose('warn.unlimited', ctx), params: { symbol: amount.symbol } });
      view.warnings.push({ tone: 'danger', key: 'warn.unlimitedLocked' });
    } else if (approval.mode === 'increase') {
      var current = claimedAllowance(ctx);
      var delta = amountPhrase(amount);
      if (current === null) {
        view.sentence = text('sentence.increaseUnknown', { delta: delta });
      } else {
        var after = current + amount.value;
        view.fields.push({
          label: 'field.allowanceAfter',
          value: formatUnits(after, amount.token.decimals) + ' ' + amount.symbol,
          format: 'raw',
          emphasis: true,
          tagKey: 'tag.byApp',
        });
        view.sentence = text('sentence.increase', {
          delta: delta,
          current: formatUnits(current, amount.token.decimals) + ' ' + amount.symbol,
          after: formatUnits(after, amount.token.decimals) + ' ' + amount.symbol,
        });
      }
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
      } else if (row.identity.kind === 'own' && view.risk === 'normal') {
        // Only ever from "normal": a caution already said (unknown decimals,
        // coin sent along) is not taken back by where the tokens go.
        view.risk = 'safe';
      }
    });
  }

  function sentenceFor(descriptor, rows, meta, view, ctx) {
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
            ? text(whose('sentence.approveUnlimited', ctx), { spender: label(to.identity), symbol: cap.amount.symbol })
            : text(whose('sentence.approve', ctx), { spender: label(to.identity), amount: amountPhrase(cap.amount) });
        }
      }
      return null;
    }
    var amount = amountPhrase(send.amount);
    if (kind === 'transfer' || kind === 'transferFrom') {
      // r10: a multisig's transfer is the multisig's ("The multisig sends …").
      return text(multisigFunds(ctx) ? 'sentence.msSends' : 'sentence.transfer',
        { amount: amount, to: to ? label(to.identity) : '—' });
    }
    if (kind === 'swap') {
      var receive = rows.find(function (r) { return r.role === 'receive-amount'; });
      var receiveText = receive ? amountPhrase(receive.amount) : '—';
      // Not "you receive" when what comes back goes to someone else (cs-2).
      return view.returnsElsewhere
        ? text('sentence.swapElsewhere', { amount: amount, receive: receiveText, to: shortAddress(view.returnsElsewhere) })
        : text('sentence.swap', { amount: amount, receive: receiveText });
    }
    if (kind === 'vaultDeposit') return text('sentence.vaultDeposit', { amount: amount, vault: label(meta) });
    if (kind === 'vaultWithdraw') return text('sentence.vaultWithdraw', { amount: amount, vault: label(meta) });
    return null;
  }

  // --- nested calldata -------------------------------------------------------

  function applyNested(descriptor, values, call, ctx, view, meta, isLeg) {
    var inner = descriptor.nested === 'multiSend'
      ? decodeMultiSend(values[0])
      : [{ to: values[0], value: values[1], data: values[2] }];
    view.hero = null;
    var legs = inner.map(function (leg, i) {
      var legView = baseView(null, ctx);
      resolveCall({ to: leg.to, value: '0x' + BigInt(leg.value || 0n).toString(16), data: leg.data }, ctx, legView, true);
      return { index: i + 1, view: legView };
    });
    // r8-5: the outermost card lists every level (a leg keeps its own legs
    // for it to list); deeper than MAX_NESTING is refused there.
    view.legs = isLeg ? legs : flattenLegs(legs, view);
    view.risk = view.legs.reduce(function (worst, leg) {
      return rank(leg.view.risk) > rank(worst) ? leg.view.risk : worst;
    }, rank(view.risk) > rank('caution') ? view.risk : 'caution');
    // OR, never overwrite: a refusal decided before the legs were read (the
    // operation does not hold what was asked) must survive them.
    view.refuse = view.refuse || view.legs.some(function (leg) { return leg.view.refuse; });
    view.sentence = descriptor.nested === 'multiSend'
      ? (inner.length === 1 ? text('sentence.multiSendOne') : text('sentence.multiSend', { count: inner.length }))
      : text('sentence.safeExec');
    // r8-5 (vela-core `reading`: a batch inside a batch asks for "… anyway"):
    // every leg is listed below; the confirm is never the plain one.
    view.warnings.push({ tone: isLeg ? 'danger' : 'caution', key: 'warn.nested' });
    if (isLeg && rank(view.risk) < rank('danger')) view.risk = 'danger';
    liftLegWarnings(view);
    view.tech = techFor(descriptor.signature, values, call.data, {
      addresses: [{ roleKey: 'ui.outerContract', name: label(meta), address: call.to }],
    });
    return view;
  }

  /**
   * The levels of legs this page lists: the top batch and vela-core's
   * `reading::MAX_NESTING` (3) batches inside it. r10: its own name — it
   * shared `MAX_NESTING` with the takeover guard's (4) below, one `var` in
   * one scope, so the "3" written here never applied; 4 is the value that
   * agrees with the core level for level (multisig-test.mjs, r10 section).
   */
  var LISTED_LEVELS = 4;

  /**
   * r8-5: every leg of every level, in order — a leg that is itself a batch
   * is followed by its own legs ("1.1", "1.2"), never a card with nothing
   * under it. Deeper than LISTED_LEVELS, nothing is read: refused, as the
   * wallet's core refuses it.
   */
  function flattenLegs(legs, view, prefix, depth) {
    depth = depth || 1;
    var out = [];
    legs.forEach(function (leg) {
      var index = prefix ? prefix + '.' + leg.index : String(leg.index);
      out.push({ index: index, view: leg.view });
      var inner = leg.view.legs || [];
      if (!inner.length) return;
      if (depth >= LISTED_LEVELS) {
        refuseWith(view, 'refuse.msBatchUnreadable');
        return;
      }
      out.push.apply(out, flattenLegs(inner, view, index, depth + 1));
      leg.view.legs = [];
    });
    return out;
  }

  /**
   * A leg's card draws its rows, not its warnings, so the ones a person must
   * read are lifted onto the card: every danger, and every coin a leg sends
   * along with its calldata (`nativeOut`), each once.
   */
  function liftLegWarnings(view) {
    view.legs.forEach(function (leg) {
      leg.view.warnings.forEach(function (w) {
        if (w.tone === 'danger' || w.key === 'warn.nativeValue' || w.key === 'warn.nativeValueMs' ||
            w.key === 'warn.invisibleChars' || w.key === 'warn.msMembersClaimOff') {
          var seen = view.warnings.some(function (o) {
            return o.key === w.key && JSON.stringify(o.params || null) === JSON.stringify(w.params || null);
          });
          if (!seen) view.warnings.push(w);
        }
      });
    });
  }

  // Safe's multiSend packs legs as: operation(1) ‖ to(20) ‖ value(32) ‖ len(32) ‖ data
  function decodeMultiSend(packed) {
    var body = abi.strip(packed);
    var legs = [];
    var i = 0;
    while (i + 170 <= body.length) {
      var length = Number(BigInt('0x' + body.slice(i + 106, i + 170)));
      legs.push({
        operation: parseInt(body.slice(i, i + 2), 16),
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
      var siweDomain = visible(siwe.domain, view);
      view.intentKey = 'intent.login';
      provenance(view, 'readMessage');
      view.hero = { kind: 'siwe', domain: siweDomain, statement: visible(siwe.statement, view), mismatch: mismatch };
      view.fields = [
        { label: 'field.claimedDomain', value: siweDomain, format: 'raw', warning: mismatch },
        { label: 'field.actualSite', value: origin, format: 'raw', warning: mismatch },
        { label: 'field.nonce', value: siwe.nonce, format: 'raw', detail: true },
      ];
      if (mismatch) {
        view.risk = 'danger';
        view.sentence = text('sentence.siwePhish', { claimed: siweDomain, actual: origin });
        view.warnings.push({ tone: 'danger', key: 'warn.siweMismatch' });
      } else {
        if (view.risk !== 'caution') view.risk = 'safe';
        view.sentence = text('sentence.siweOk', { domain: siweDomain });
      }
      view.tech = techFor('personal_sign (SIWE)', null, payload, {});
      return view;
    }

    view.intentKey = 'intent.message';
    provenance(view, 'readMessage');
    view.hero = { kind: 'message', text: visible(body, view) };
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

  // The ONE typed-data document a request is read as — the wallet core's
  // `typed_data_request`, and the only reading this page's preview AND its
  // digest (`digest.js`) take: the four methods; exactly two params; legacy
  // `eth_signTypedData` / `_v1` = [typedData, address], `_v3` / `_v4` =
  // [address, typedData]; one EIP-712 document. `null` otherwise. The audit of
  // 2026-10-01: the preview read params[1] while the digest read params[0] for
  // the legacy names, so [malicious, benign] showed the benign one.
  function typedDocument(intent) {
    var method = intent && intent.method;
    var legacy = method === 'eth_signTypedData' || method === 'eth_signTypedData_v1';
    var modern = method === 'eth_signTypedData_v3' || method === 'eth_signTypedData_v4';
    var params = intent && intent.params;
    if ((!legacy && !modern) || !Array.isArray(params) || params.length !== 2) return null;
    var account = params[legacy ? 1 : 0];
    if (typeof account !== 'string' || !/^0x[0-9a-fA-F]{40}$/.test(account)) return null;
    var doc = params[legacy ? 0 : 1];
    if (typeof doc === 'string') {
      try { doc = JSON.parse(doc); } catch (e) { return null; }
    }
    function object(value) { return !!value && typeof value === 'object' && !Array.isArray(value); }
    if (!object(doc) || !object(doc.types) || typeof doc.primaryType !== 'string' ||
        !doc.primaryType || !object(doc.domain) || !object(doc.message)) return null;
    return doc;
  }

  function resolveTypedData(intent, ctx, view) {
    var data = typedDocument(intent);
    if (!data) {
      view.intentKey = 'intent.unknownRequest';
      view.risk = 'danger';
      view.sentence = text('refuse.typedShape');
      view.warnings.push({ tone: 'danger', key: 'refuse.typedShape' });
      view.refuse = true;
      return view;
    }
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
      // A token's own permit when the token is one this page knows here;
      // otherwise any contract can verify a struct of that shape.
      if (reg.token(domain.verifyingContract, ctx.chainId)) provenance(view, 'readMessage');
      else provenance(view, 'shape', { whatKey: 'shape.permit' });
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
      // An unlimited permit is signable as it arrived, like the wallet's own
      // sheet signs one (owner, 2026-09-26): the dApp redeems its own struct,
      // so a capped signature would only fail on-chain. Red, and said.
      if (amount.unlimited) {
        view.risk = 'danger';
        view.warnings.push({ tone: 'danger', key: 'warn.unlimitedOffline' });
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
      // PermitSingle's amount is a uint160; PermitTransferFrom's is a uint256.
      var pAmount = tokenAmount(BigInt(details.amount), details.token, ctx, primary === 'PermitSingle' ? 160 : 256);
      var pSpender = identify(message.spender, ctx);
      view.intentKey = 'intent.permit2';
      // Permit2's struct is Permit2's only when Permit2 verifies it.
      var isPermit2 = reg.contract(domain.verifyingContract, ctx.chainId);
      if (isPermit2 && isPermit2.name === 'Permit2') provenance(view, 'readMessage');
      else provenance(view, 'shape', { whatKey: 'shape.permit2' });
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
      // Uniswap's own Permit2 signatures are usually for the uint160 maximum;
      // refusing them here broke swaps the wallet's sheet signs (2026-09-26).
      if (pAmount.unlimited) {
        view.warnings.push({ tone: 'danger', key: 'warn.unlimitedOffline' });
      }
      view.tech = techFor('EIP-712 · ' + primary, null, null, {
        params: [
          { name: 'token', value: details.token },
          { name: 'amount', value: String(details.amount) },
          { name: 'spender', value: message.spender },
          { name: 'expiration', value: String(details.expiration || message.deadline) },
        ],
        // Named from the table, like every other address: a struct verified
        // by some other contract was labelled "Permit2" here.
        addresses: [{ roleKey: 'ui.verifyingContract', name: label(view.contract), address: domain.verifyingContract }],
      });
      return view;
    }

    view.intentKey = 'intent.typed';
    provenance(view, 'readMessage');
    view.level = 2;
    view.risk = 'caution';
    view.hero = { kind: 'code', title: visible(primary, view), sub: visible(domain.name || null, view) };
    view.fields = Object.keys(message).map(function (key) {
      return { labelRaw: visible(key, view), value: visible(renderRaw(message[key]), view), format: 'raw' };
    });
    view.sentence = text('sentence.typedUnknown', { type: view.hero.title });
    view.warnings.push({ tone: 'caution', key: 'warn.typedUnknown' });
    view.tech = techFor('EIP-712 · ' + primary, null, null, {
      params: Object.keys(message).map(function (k) { return { name: k, value: renderRaw(message[k]) }; }),
      addresses: domain.verifyingContract
        ? [{ roleKey: 'ui.verifyingContract', name: label(view.contract), address: domain.verifyingContract }]
        : [],
    });
    return view;
  }

  // --- key ceremonies (spec 075) -----------------------------------------------
  //
  // The page is a passkey route of its own, so besides signing intents it takes
  // four requests a Vela wallet builds for its own flows: create a key, sign
  // in, prove a key (verify / recover), and confirm a key's membership in the
  // registry. Each gets its own card. The challenge each one signs is derived
  // by the page (lib/ceremony.js) — a request that tries to hand one over is
  // refused outright, because a "sign-in" over bytes the requester chose could
  // be a transaction hash in disguise.

  // A 16-byte-or-longer credential id, base64url, as the page's wire carries it.
  var CREDENTIAL_ID = /^[A-Za-z0-9_-]{16,1400}$/;

  /**
   * A credential id this page can hand to the browser: base64url of the
   * right length — and one that DECODES. A length of 4n+1 is no base64 at
   * all, and `atob` threw on it inside the tap, after the button had gone
   * busy: "Waiting for your passkey…" for ever.
   */
  function credentialIdOk(id) {
    return typeof id === 'string' && CREDENTIAL_ID.test(id) && id.length % 4 !== 1;
  }
  var P256_POINT = /^(0x)?04[0-9a-fA-F]{128}$/;
  /** A 20-byte contract address, `0x`-prefixed as the wallet sends it. */
  var ADDRESS = /^0x[0-9a-fA-F]{40}$/;
  var LOOPBACK_HOST = /^(localhost|127\.0\.0\.1|\[::1\])$/;

  // --- where the key is (spec 102 R5) ------------------------------------------
  //
  // The wallet says which key signs and where it lives — the place the person
  // chose when they made the key or signed in — so the browser can go straight
  // to it instead of asking "where is your passkey?" all over again. Every
  // part is checked here and anything unrecognised is dropped: a route only
  // ever narrows what the browser offers, and the answer is still checked
  // against the account's keys (sign.js) and by the wallet.

  /** A place → its WebAuthn L3 hints (vela-core `signing_venue::hints_of`). */
  var PLACE_HINTS = { platform: ['client-device'], hybrid: ['hybrid'], security_key: ['security-key'] };
  /** A place → the transports to name when the wallet knows none for the key. */
  var PLACE_TRANSPORTS = { platform: ['internal'], hybrid: ['hybrid'], security_key: ['usb', 'nfc'] };
  var TRANSPORTS = ['usb', 'nfc', 'ble', 'smart-card', 'hybrid', 'internal'];
  var HINTS = ['client-device', 'hybrid', 'security-key'];

  function knownList(value, allowed) {
    var out = [];
    (Array.isArray(value) ? value : []).forEach(function (item) {
      if (typeof item === 'string' && allowed.indexOf(item) >= 0 && out.indexOf(item) < 0) out.push(item);
    });
    return out;
  }

  /**
   * `{credentialId, place, transports, hints}` as checked, or null.
   *
   * `raw` is the request's `context.keyRoute` (a signature) or a ceremony's
   * own params; `allow` the request's `allowCredentials`. A route that names
   * a key outside them, or a credential id that is not one, is no route at
   * all — the request then asks as it always did. Hints the wallet did not
   * send are the place's; transports it did not send are the place's too
   * (a key named with none makes some browsers guess — vela-core
   * `app::shell`).
   */
  function keyRoute(raw, allow) {
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null;
    var place = Object.prototype.hasOwnProperty.call(PLACE_HINTS, raw.place) ? raw.place : null;
    var id = null;
    if (raw.credentialId !== undefined && raw.credentialId !== null) {
      if (!credentialIdOk(raw.credentialId)) return null;
      id = raw.credentialId;
      if (Array.isArray(allow) && allow.length && allow.indexOf(id) < 0) return null;
    }
    if (!place && !id) return null;
    var hints = knownList(raw.hints, HINTS);
    if (!hints.length && place) hints = PLACE_HINTS[place].slice();
    var transports = id ? knownList(raw.transports, TRANSPORTS) : [];
    if (id && !transports.length && place) transports = PLACE_TRANSPORTS[place].slice();
    return { credentialId: id, place: place, transports: transports, hints: hints };
  }

  /**
   * The confirm button's words: the action, as the apps word it
   * (`ClearConfirm` — "Confirm send", "Approve", "Sign"…), never a bare
   * "OK". The intent above it says what; this says what tapping does.
   */
  var CONFIRM = {
    'intent.transfer': 'button.send', 'intent.send': 'button.send',
    'intent.transferFrom': 'button.send', 'intent.nftTransfer': 'button.send',
    'intent.swap': 'button.swap',
    'intent.vaultDeposit': 'button.deposit', 'intent.vaultWithdraw': 'button.withdraw',
    'intent.approve': 'button.approve', 'intent.increase': 'button.approve',
    'intent.permit': 'button.approve', 'intent.permit2': 'button.approve',
    'intent.approveAll': 'button.approveAll',
    'intent.revoke': 'button.revoke',
    'intent.login': 'button.signIn', 'intent.signIn': 'button.signIn',
    'intent.create': 'button.create',
    'intent.proofVerify': 'button.confirm', 'intent.proofRecover': 'button.confirm',
    'intent.memberProof': 'button.confirm',
    'intent.msReject': 'button.msReject',
    'intent.msExecute': 'button.msExecute',
  };

  function confirmKey(view) {
    if (view.refuse) return 'button.cannotSign';
    // D1: an execution the app's one review asks "… anyway" for.
    if (view.intentKey === 'intent.msExecute' && view.anyway) return 'button.msExecuteAnyway';
    return CONFIRM[view.intentKey] || 'button.sign';
  }

  function hostOf(url) {
    try { return new URL(url).hostname.toLowerCase(); } catch (e) { return ''; }
  }

  function schemeOf(url) {
    try { return new URL(url).protocol; } catch (e) { return ''; }
  }

  /**
   * The one address this page sends an answer to: the Vela wallet on this
   * device (`trusted_signer::CALLBACK_URL`, which every app passes). The whole
   * address, not just the scheme — `velawallet://open?url=…` is the wallet
   * too, and an answer appended to it would go wherever that link points.
   */
  var WALLET_CALLBACK = 'velawallet://sign-result';

  /**
   * Is the requester a Vela wallet? Three kinds of evidence count:
   *
   *  · an app channel: the loopback WebSocket (the app that opened this page
   *    proved it with its one-time token), the tunnel (the wallet's key matches
   *    the link's `rk`, and the person compares the code), BLE (proximity and
   *    the code) — all retired, kept because the shape of the rule is theirs;
   *  · an origin the BROWSER vouches for (postMessage)
   *    that is https://getvela.app or a subdomain — or this machine's loopback,
   *    where the web wallet runs in development and in tests;
   *  · **where the answer is going.** On the URL channel — now the only one the
   *    clients have — nothing can tell this page who opened it, and a fragment
   *    is no evidence: any site can open this page with one. But the page does
   *    know the address it will send the answer to, because sending it is what
   *    the page will do. If that address is the wallet's own
   *    (`answersToWallet`), then whoever asked, the answer reaches a Vela
   *    wallet and nobody else.
   *
   * Why that is protection and not a rubber stamp: somebody who wants the key
   * this page creates has to RECEIVE it, so they have to name a callback they
   * can receive on. Name the wallet's address and the key goes to the wallet,
   * which is no use to them; name their own and this returns false. The way
   * round it is to register `velawallet://` on the device and win the
   * collision — which is precisely the limit the transport documents, and
   * which no page can do anything about (`trusted_signer::CALLBACK_URL`).
   *
   * Note what this does NOT claim: not that Vela asked, only that Vela
   * answers. The card says it in those words, because that is the true one.
   */
  function walletRequester(ctx) {
    if (ctx.channel === 'ws' || ctx.channel === 'relay' || ctx.channel === 'ble') return true;
    if (ctx.channel === 'url') return ns.resolve.answersToWallet(ctx);
    if (ctx.originVerified !== true || typeof ctx.requester !== 'string') return false;
    // The Safari extension, by the same evidence a signature needs
    // (`isSafariExtension`). On Safari an extension page can run no passkey
    // ceremony of its own (WebKit refuses WebAuthn there), so this page is
    // where the Safari wallet creates, signs in and proves.
    if (isSafariExtension(ctx.requester, ctx.attestedExtensions)) return true;
    var host = hostOf(ctx.requester);
    if (LOOPBACK_HOST.test(host)) return true;
    return schemeOf(ctx.requester) === 'https:' && (host === 'getvela.app' || /\.getvela\.app$/.test(host));
  }

  /**
   * Will this request's answer reach the Vela wallet on this device, and
   * nothing else? (spec 102 R7)
   *
   * The URL channel can say yes when its answer address is exactly the
   * wallet's. postMessage answers whoever opened this page — any site can —
   * so it says yes only when the browser reports the opener as one of Vela's
   * own wallets, by its EXACT origin: the web wallet (`isWebWallet`) or the
   * published Chrome extension (`isExtension`). Neither can receive
   * `velawallet://`; the browser's own `event.origin` names the window the
   * answer goes to, which is a stronger fact than any callback address. And
   * only for what a wallet signs (`SIGNED_THERE`): a transaction, a batch, a
   * message, a multisig member's approval (owner, 2026-10-30: the page is
   * usable everywhere — until then, only the approval, and only for the web
   * wallet). Every signature over postMessage from any other opener stays
   * refused, a `getvela.app` subdomain included; `eth_sign` is refused
   * whoever asks. The loopback socket is retired (the published page's
   * `default-src 'none'` cannot open it).
   *
   * ONE rule, used everywhere the answer's destination matters: a signature
   * is refused without it (`resolve`), so is every key ceremony on the URL
   * channel (`walletRequester`), and lib/intake.js sends nothing — not even
   * a refusal code — to an address it rejects. Everyone calls it as
   * `ns.resolve.answersToWallet`, this file included, so the rule has one
   * name. The desktop demo's harness page (`samples/loopback-sign.html`)
   * replaces it with one that also accepts the demo's loopback listener;
   * that file is outside `src/`, so no published page contains it
   * (`build-single.mjs` reads only what `src/sign.html` names).
   */
  function answersToWallet(ctx) {
    if (!ctx) return false;
    if (ctx.channel === 'url') return ctx.callback === WALLET_CALLBACK;
    return ctx.channel === 'post' && ctx.originVerified === true &&
      SIGNED_THERE.indexOf(ctx.method) >= 0 &&
      walletOpener(ctx.requester, ctx.attestedExtensions);
  }

  /**
   * The opener the browser reports is one of Vela's own wallets. `attested`
   * is what `lib/intake.js` heard in THIS window (`isSafariExtension`) — a
   * channel fact, never the requester's context.
   */
  function walletOpener(origin, attested) {
    return isWebWallet(origin) || isExtension(origin) || isSafariExtension(origin, attested);
  }

  /**
   * Vela's Safari extension (native AUDIT §6.5), by the origin the BROWSER
   * reports for the opener AND the extension's own word, said inside this
   * page.
   *
   * The opener is either the window that opened this page (`event.origin`),
   * or — iOS Safari gives a page an extension opened NO opener (measured) —
   * the extension speaking through its content script in this window
   * (lib/intake.js `relayPeer`), whose origin is the one it attested.
   *
   * Safari names an extension's pages `safari-web-extension://<UUID>` with a
   * UUID it makes up per install and again at every Safari launch (WebKit's
   * fingerprinting defence; measured on iOS, extension/README.md § Safari) —
   * so there is no fixed origin to list the way the Chrome id is listed.
   * What can be checked is this: Vela's extension runs a content script on
   * this page (`safari-attest.js`, on the official page only), which posts
   * the extension's own current origin into THIS window
   * (`{vela:'extension', origin}`). Only code running in this window can post
   * a message whose source is this window: this page's own hashed scripts, or
   * a content script of an extension the person let run on this page. The
   * opener is answered when the browser's `event.origin` for it is exactly
   * one of the origins so attested.
   *
   * What it admits, and why that is no wider than before: a site cannot
   * attest (it is not in this window), nor can an extension that has no
   * access to this page. An extension that HAS access to this page could
   * attest itself — and it could equally rewrite this card or press its
   * button; no rule a page makes holds against code the browser runs inside
   * it. Several attestations are several candidates; the opener must still be
   * exactly one of them. Chrome's and the web wallet's origins stay exact and
   * unattested (`isExtension`, `isWebWallet`).
   */
  function isSafariExtension(origin, attested) {
    // Compared in lower case: Safari reports the origin with a lower-case
    // UUID and `runtime.getURL` spells it in upper case (measured).
    return typeof origin === 'string' && SAFARI_EXTENSION.test(origin) &&
      Array.isArray(attested) && attested.some(function (a) {
        return typeof a === 'string' && a.toLowerCase() === origin.toLowerCase();
      });
  }

  /** A Safari extension page's origin: the scheme and a UUID, nothing after. */
  var SAFARI_EXTENSION = /^safari-web-extension:\/\/[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}$/;

  /**
   * The web wallet, by the origin the BROWSER reports for the window that
   * opened this page (postMessage's `event.origin` — not a claim): Vela's own
   * web wallet, and nothing else on `getvela.app` (a subdomain is not the
   * wallet). A page served from this machine's loopback — a developer's — is
   * also answered by a wallet on the loopback: such a page can only use the
   * loopback's own passkeys, never `getvela.app`'s, so nothing it signs is of
   * use to anyone but that developer.
   */
  var WEB_WALLETS = ['https://wallet.getvela.app', 'https://getvela.app'];
  var MULTISIG_APPROVE = 'vela_multisigApprove';

  /**
   * The published Chrome extension, by its exact origin. The id is the one
   * the extension's manifest `key` derives (`extension/manifest.json`; the
   * development and the GitHub-release packages carry that key). The Chrome
   * Web Store assigns its own id to the store package, which carries no key:
   * it joins this list once the store has assigned it — until then a store
   * install reviews and signs in Vela (`docs/store-submission`).
   */
  var EXTENSIONS = ['chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl'];

  /** What a wallet hands this page to sign over postMessage. */
  var SIGNED_THERE = [
    'eth_sendTransaction',
    'wallet_sendCalls',
    'personal_sign',
    'eth_signTypedData_v4',
    'eth_signTypedData_v3',
    'eth_signTypedData',
    MULTISIG_APPROVE,
    // §16: a member's owner proof when a multisig is created — the Safari
    // extension's only way to make one (WebKit gives its pages no passkey).
    'vela_multisigCreate',
  ];

  /**
   * The extension, by the origin the BROWSER reports for the window that
   * opened this page: its published id, exactly. A page served from this
   * machine's loopback — a developer's — also answers any unpacked
   * extension (whose id is derived from its folder): such a page can only
   * use the loopback's own passkeys, never `getvela.app`'s.
   */
  function isExtension(origin) {
    if (typeof origin !== 'string') return false;
    if (EXTENSIONS.indexOf(origin) >= 0) return true;
    var here = ns.place ? ns.place.host() : typeof location === 'object' && location ? String(location.hostname || '') : '';
    return LOOPBACK_HOST.test(here) && /^chrome-extension:\/\/[a-p]{32}$/.test(origin);
  }

  /**
   * The site in a request a wallet relays (the opener is Vela's web wallet or
   * extension, and the site is not the opener itself) is the wallet's word:
   * the browser vouched for who opened this page, not for the site the
   * wallet says asked it. A tampered wallet is what this page is for, so its
   * word is drawn as a claim (`warn.claimedOrigin`).
   */
  function relayedSite(requested, ctx) {
    if (ctx.channel !== 'post' || ctx.originVerified !== true || !walletOpener(ctx.requester, ctx.attestedExtensions)) return false;
    var origin = requested && typeof requested.origin === 'string' ? requested.origin : '';
    if (!origin) return false;
    var parsed = null;
    try { parsed = new URL(origin); } catch (e) { return true; }
    return parsed.origin !== ctx.requester;
  }

  function isWebWallet(origin) {
    if (typeof origin !== 'string') return false;
    if (WEB_WALLETS.indexOf(origin) >= 0) return true;
    var here = ns.place ? ns.place.host() : typeof location === 'object' && location ? String(location.hostname || '') : '';
    var parsed = null;
    try { parsed = new URL(origin); } catch (e) { return false; }
    return LOOPBACK_HOST.test(here) && parsed.protocol === 'http:' && LOOPBACK_HOST.test(parsed.hostname) &&
      origin === parsed.origin;
  }

  /**
   * Where a refused answer would have gone, in words a person can check — the
   * host of a web address, the scheme and host of any other link, the site
   * that opened the page — or null when there is nothing to name.
   */
  function answerDestination(ctx) {
    var target = ctx.channel === 'url' ? ctx.callback
      : ctx.channel === 'post' && ctx.originVerified === true ? ctx.requester
      : null;
    if (typeof target !== 'string' || !target) return null;
    var parsed = null;
    try { parsed = new URL(target); } catch (e) { return null; }
    var shown = parsed.protocol === 'https:' || parsed.protocol === 'http:'
      ? parsed.host
      : parsed.protocol + '//' + parsed.host;
    return shown && shown.length <= 80 && !UNREADABLE.test(shown) ? shown : null;
  }

  // Anything that could be the bytes to sign, anywhere the requester controls:
  // the context, a second parameter, a non-object first one, or a field of it.
  function suppliedChallenge(intent, ctx) {
    if (ctx.challenge !== undefined || ctx.digest !== undefined) return true;
    var params = Array.isArray(intent.params) ? intent.params : [];
    if (params.length > 1) return true;
    var first = params[0];
    if (first === undefined || first === null) return false;
    if (typeof first !== 'object' || Array.isArray(first)) return true;
    return ['challenge', 'digest', 'hash', 'message'].some(function (name) {
      return Object.prototype.hasOwnProperty.call(first, name);
    });
  }

  // A name a person gave their wallet: shown, because it is how they recognise
  // it, but only if it is readable text of a sane length.
  function displayName(value) {
    if (typeof value !== 'string') return null;
    var trimmed = value.trim();
    if (!trimmed || trimmed.length > 64 || UNREADABLE.test(trimmed)) return null;
    // Bidirectional overrides can make a name read as something else.
    if (/[‪-‮⁦-⁩]/.test(trimmed)) return null;
    return trimmed;
  }

  function registryUrl(value) {
    if (value === undefined || value === null || value === '') return ns.ceremony.DEFAULT_REGISTRY;
    if (typeof value !== 'string') return null;
    var scheme = schemeOf(value);
    var host = hostOf(value);
    if (scheme === 'https:' && host) return value.replace(/\/+$/, '');
    if (scheme === 'http:' && LOOPBACK_HOST.test(host)) return value.replace(/\/+$/, '');
    return null;
  }

  function shortKey(hexKey) {
    var body = String(hexKey).replace(/^0x/i, '').toLowerCase();
    return body.slice(0, 8) + '…' + body.slice(-6);
  }

  // Who is asking, in words the person can check: the verified site, where the
  // answer will go, or the kind of channel the request came over.
  function requesterLine(ctx) {
    if (ctx.originVerified === true && ctx.requester) {
      return { origin: hostOf(ctx.requester) || ctx.requester, verified: true };
    }
    if (ctx.channel === 'url') {
      // Say the destination, not a name. The page cannot know who opened it,
      // and a self-reported name beside a mark reads as an identity the page
      // checked (owner, 2026-09-24: the page answers a `velawallet://` and
      // nothing else, so 「某个钱包」 and its logo were both inappropriate).
      return ns.resolve.answersToWallet(ctx)
        ? { originKey: 'value.answerOnlyToIt', verified: true }
        : { origin: schemeOf(ctx.callback) || null, originKey: schemeOf(ctx.callback) ? null : 'value.answerToNobody', verified: false, elsewhere: true };
    }
    var byChannel = { ws: 'value.viaApp', relay: 'value.viaTunnel', ble: 'value.viaBle' }[ctx.channel];
    return { originKey: byChannel || 'value.viaUnknown', verified: false };
  }

  /**
   * A stable colour for a name nobody verified: the same name always gets the
   * same one, and two different names almost never collide. Hues only, at one
   * saturation and lightness, so every unverified peer reads as the same KIND
   * of thing — a stranger who told us a name — and never as a brand.
   */
  function toneFor(name) {
    var text = String(name || '');
    var hash = 0;
    for (var i = 0; i < text.length; i++) {
      hash = ((hash << 5) - hash + text.charCodeAt(i)) | 0;
    }
    return 'hsl(' + (Math.abs(hash) % 360) + ', 34%, 46%)';
  }

  function resolveCeremony(kind, intent, ctx, view) {
    var p = (Array.isArray(intent.params) && intent.params[0]) || {};
    if (typeof p !== 'object' || Array.isArray(p)) p = {};
    var wallet = displayName(ctx.walletName);
    var rpId = ctx.rpId || null;
    var fromWallet = walletRequester(ctx);
    var bad = intent.params !== undefined && !Array.isArray(intent.params);
    /** A member proof whose request names no chain or registry contract. */
    var noDeployment = false;

    view.kind = 'ceremony';
    view.dapp.claimedName = null;
    view.intentKey = 'intent.' + kind;
    view.risk = 'normal';
    view.level = 1;
    view.account = null;
    view.ceremony = { kind: kind, walletName: wallet, fromWallet: fromWallet };

    // The head names whoever is asking — and only wears Vela's mark when the
    // channel vouches for it.
    //
    // This page is a signer anything can connect to: over BLE or the tunnel the
    // requester is whatever dialled in, and its hello's `app` is a CLAIM. The
    // page already refuses to launder a dApp's self-reported name into a
    // verified one; painting Vela's V and "Vela 钱包" on an unverified peer was
    // the same laundering one layer up, and the louder one, because the mark
    // is what a person reads before deciding to trust the screen.
    var who = requesterLine(ctx);
    var claimed = typeof ctx.requesterApp === 'string' ? ctx.requesterApp.trim().slice(0, 40) : '';
    view.dapp.name = null;
    view.dapp.nameKey = 'tag.velaWallet';
    view.dapp.letter = 'V';
    view.dapp.tone = '#ff6a1a';
    view.dapp.icon = null;
    if (who.elsewhere) {
      // The answer is going somewhere that is not a Vela wallet. Say the
      // destination and nothing else: no name, no mark, nothing borrowed from
      // whoever asked. The ceremony rules refuse this case anyway; the card's
      // job is to make the reason readable.
      view.dapp.name = null;
      view.dapp.nameKey = 'tag.notThisWallet';
      view.dapp.letter = '?';
      view.dapp.tone = toneFor(String(ctx.callback || ''));
    } else if (!who.verified) {
      // A name it gave for itself, said as such; otherwise no name at all.
      //
      // The mark is a monogram in a colour DERIVED from that name, never an
      // image the peer sent. A logo is the strongest thing a person reads
      // before trusting a screen and the easiest to forge — hand the peer a
      // picture slot and an impostor simply sends Vela's. Derived, the mark is
      // a fingerprint of the name that is printed right next to it: to look
      // the same, something has to BE called the same.
      view.dapp.name = claimed || null;
      view.dapp.nameKey = claimed ? null : 'tag.someWallet';
      view.dapp.nameClaimed = !!claimed;
      view.dapp.letter = (claimed || '?').slice(0, 1).toUpperCase();
      view.dapp.tone = toneFor(claimed);
      // The mark it sent for itself, if it sent one the transport allowed
      // (inline, raster, small). Drawn beside the line that says both the
      // name and the mark are its own claim — this channel cannot check
      // either, and the page must not look like it did.
      view.dapp.icon = typeof ctx.requesterIcon === 'string' && ctx.requesterIcon
        ? ctx.requesterIcon
        : null;
    }
    view.dapp.origin = who.origin || null;
    view.dapp.originShown = !!view.dapp.origin && !sameSiteWords(view.dapp.name || '', view.dapp.origin);
    view.dapp.originKey = who.originKey || null;
    view.dapp.originVerified = who.verified;

    // No hero: the wallet is this row, as the apps' own sheets list it.
    if (wallet) view.fields.push({ label: 'field.wallet', value: wallet });

    var params = [];
    if (kind === 'create') {
      var name = p.name === undefined ? (wallet || 'Vela') : displayName(p.name);
      var exclude = p.excludeCredentialIds === undefined ? [] : p.excludeCredentialIds;
      if (!name || !Array.isArray(exclude) || !exclude.every(function (id) {
        return credentialIdOk(id);
      })) bad = true;
      view.ceremony.name = name;
      view.ceremony.excludeCredentialIds = Array.isArray(exclude) ? exclude : [];
      view.sentence = text(wallet ? 'sentence.create' : 'sentence.createUnnamed', { wallet: wallet });
      if (name && name !== wallet) view.fields.push({ label: 'field.passkeyName', value: name });
      params.push({ name: 'challenge', valueKey: 'value.randomChallenge' });
      params.push({ name: 'pubKeyCredParams', value: 'ES256 (P-256)' });
      params.push({ name: 'residentKey', value: 'required' });
      params.push({ name: 'userVerification', value: 'required' });
      if (view.ceremony.excludeCredentialIds.length) {
        params.push({ name: 'excludeCredentials', value: String(view.ceremony.excludeCredentialIds.length) });
      }
    } else if (kind === 'signIn') {
      view.sentence = text('sentence.signIn');
      params.push({ name: 'allowCredentials', valueKey: 'value.anyKey' });
    } else if (kind === 'proof') {
      var purpose = p.purpose;
      if (!ns.ceremony.proofChallenge(purpose)) bad = true;
      if (p.credentialId !== undefined && !credentialIdOk(p.credentialId)) bad = true;
      view.ceremony.purpose = purpose;
      view.ceremony.credentialId = typeof p.credentialId === 'string' ? p.credentialId : null;
      view.intentKey = purpose === 'verify' ? 'intent.proofVerify' : 'intent.proofRecover';
      view.badge = purpose === 'recover_first' ? 'tag.stepOne' : purpose === 'recover_second' ? 'tag.stepTwo' : null;
      view.sentence = text(purpose === 'verify' ? 'sentence.proofVerify' : 'sentence.proofRecover', { wallet: wallet });
      if (view.ceremony.credentialId) {
        view.fields.push({ label: 'field.credential', value: view.ceremony.credentialId.slice(0, 10) + '…' });
      }
    } else if (kind === 'memberProof') {
      var member = {
        credentialId: p.credentialId,
        publicKey: p.publicKey,
        groupPublicKey: p.groupPublicKey,
        attestation: p.attestation === undefined || p.attestation === null ? '' : p.attestation,
        registry: registryUrl(p.registry),
        // The DEPLOYMENT the challenge is computed against. The wallet reads
        // these from the registry's `/api/health` and hands them over, because
        // the published page reaches no network at all (076). They are not
        // trusted: the page computes the challenge FROM them and signs only
        // what it computed, so a requester that lies about either gets a
        // challenge the wallet never asked for.
        chainId: p.chainId,
        registryContract: p.registryContract,
      };
      if (!credentialIdOk(member.credentialId)) bad = true;
      if (typeof member.publicKey !== 'string' || !P256_POINT.test(member.publicKey)) bad = true;
      if (typeof member.groupPublicKey !== 'string' || !P256_POINT.test(member.groupPublicKey)) bad = true;
      if (typeof member.attestation !== 'string' || !/^(0x)?([0-9a-fA-F]{40})?$/.test(member.attestation)) bad = true;
      if (!member.registry) bad = true;
      // Told apart from the rest, because the reason a person can act on is
      // different: the other fields being wrong is a malformed request, and
      // this one missing means the page has nothing to compute the challenge
      // from. `refuse.badCeremony` would send them looking at the keys.
      if (!Number.isSafeInteger(member.chainId) || member.chainId < 1 ||
        typeof member.registryContract !== 'string' || !ADDRESS.test(member.registryContract)) {
        bad = true;
        noDeployment = true;
      }
      if (!bad && member.publicKey.replace(/^0x/i, '').toLowerCase() ===
        member.groupPublicKey.replace(/^0x/i, '').toLowerCase()) bad = true;
      view.ceremony.credentialId = member.credentialId;
      view.ceremony.member = bad ? null : {
        publicKey: member.publicKey.replace(/^0x/i, '').toLowerCase(),
        groupPublicKey: member.groupPublicKey.replace(/^0x/i, '').toLowerCase(),
        attestation: member.attestation.replace(/^0x/i, '').toLowerCase(),
        registry: member.registry,
        chainId: member.chainId,
        registryContract: member.registryContract,
      };
      view.sentence = text(wallet ? 'sentence.memberProof' : 'sentence.memberProofUnnamed', { wallet: wallet });
      if (!bad) {
        view.fields.push({ label: 'field.key', value: shortKey(view.ceremony.member.publicKey) });
        view.fields.push({ label: 'field.groupKey', value: shortKey(view.ceremony.member.groupPublicKey) });
        view.fields.push({ label: 'field.registry', value: hostOf(member.registry) });
        view.fields.push({
          label: 'field.chain',
          value: reg.chainName(member.chainId)
            ? reg.chainName(member.chainId) + ' · ' + member.chainId
            : String(member.chainId),
        });
        params.push({ name: 'chainId', value: String(member.chainId) });
        params.push({ name: 'registryContract', value: member.registryContract });
        params.push({ name: 'publicKey', value: view.ceremony.member.publicKey });
        params.push({ name: 'groupPublicKey', value: view.ceremony.member.groupPublicKey });
        params.push({ name: 'attestation', value: view.ceremony.member.attestation || '—' });
        params.push({ name: 'registry', value: member.registry });
      }
    }

    // Where the key is, or where the new one goes (spec 102 R5): the place
    // the person chose in the wallet, so the browser asks for that one only.
    view.key = keyRoute({
      credentialId: kind === 'proof' || kind === 'memberProof' ? p.credentialId : undefined,
      place: p.place,
      transports: p.transports,
      hints: p.hints,
    }, null);
    view.keyLabel = kind === 'create' ? 'field.keyOn' : 'field.confirmWith';
    if (view.key && view.key.hints.length) params.push({ name: 'hints', value: view.key.hints.join(', ') });
    if (view.key && view.key.transports.length) params.push({ name: 'transports', value: view.key.transports.join(', ') });

    view.fields.push({ label: 'field.relyingParty', value: rpId || '—' });
    view.tech = techFor(kind === 'create' ? 'WebAuthn · navigator.credentials.create' : 'WebAuthn · navigator.credentials.get',
      null, null, { params: params });

    // The refusals, most fundamental first.
    if (suppliedChallenge(intent, ctx)) {
      view.refuse = true;
      view.risk = 'danger';
      view.warnings.push({ tone: 'danger', key: 'refuse.suppliedChallenge' });
    } else if (ctx.channel === 'url' && !fromWallet) {
      // On the url channel the answer goes to the address in the link, and
      // this page sends one only to the wallet's (spec 102 R7; lib/intake.js
      // sends nothing anywhere else). So EVERY ceremony is refused here, not
      // only a create: a sign-in or a proof would raise a passkey prompt and
      // then deliver nothing. The reason is where the answer would go, not
      // who asked — on this channel nothing can prove who asked.
      view.refuse = true;
      view.risk = 'danger';
      view.warnings.push({
        tone: 'danger',
        key: kind === 'create' ? 'refuse.createNotForWallet'
          : kind === 'memberProof' ? 'refuse.memberNotForWallet'
          : 'refuse.ceremonyNotForWallet',
      });
    } else if ((kind === 'create' || kind === 'memberProof') && !fromWallet) {
      view.refuse = true;
      view.risk = 'danger';
      view.warnings.push({ tone: 'danger', key: kind === 'create' ? 'refuse.createNotWallet' : 'refuse.memberNotWallet' });
    } else if (bad) {
      view.refuse = true;
      view.risk = 'danger';
      view.warnings.push({ tone: 'danger', key: noDeployment ? 'refuse.noDeployment' : 'refuse.badCeremony' });
    } else if (!fromWallet) {
      view.risk = 'caution';
      view.warnings.push({ tone: 'caution', key: 'warn.notWalletAsks' });
    }
    if (!view.refuse && kind === 'create' && rpId && rpId !== 'getvela.app') {
      view.warnings.push({ tone: 'caution', key: 'warn.foreignPageKey', params: { rpId: rpId } });
    }
    view.confirmKey = confirmKey(view);
    return view;
  }

  // --- who controls the account ---------------------------------------------
  //
  // The core refuses these before a request ever reaches this page
  // (`vela-core` `self_call_guard.rs`, spec 081). The page refuses them again,
  // because the whole point of it is not to trust the app that assembled the
  // operation: a call from the account to itself that rewrites its owners,
  // modules, guard or fallback handler hands the account over as completely as
  // the payload that drained Bybit, and so does any delegatecall.

  var SELF_CALL = {
    '0x0d582f13': 'addOwnerWithThreshold',
    '0xf8dc5dd9': 'removeOwner',
    '0xe318b52b': 'swapOwner',
    '0x694e80c3': 'changeThreshold',
    '0x610b5925': 'enableModule',
    '0xe009cfde': 'disableModule',
    '0xe19a9dd9': 'setGuard',
    '0xe068df37': 'setModuleGuard',
    '0xf08a0323': 'setFallbackHandler',
    '0xb63e800d': 'setup',
    '0x6a761202': 'execTransaction',
    '0x468721a7': 'execTransactionFromModule',
    '0x5229073f': 'execTransactionFromModule',
  };
  var MULTI_SEND_SELECTOR = '0x8d80ff0a';
  var EXEC_TRANSACTION_SELECTOR = '0x6a761202';
  var EXEC_TRANSACTION =
    'execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)';
  // Past anything legitimate; the cap keeps a crafted payload cheap to read.
  var MAX_NESTING = 4;

  /**
   * The function a call would run against the account's control, or null. It
   * follows `multiSend` and `execTransaction` payloads (a batch hides the same
   * call one level down), and any delegatecall inside them is refused whatever
   * its target — the same rule as the core's `inspect_call`.
   */
  function takeover(call, account, depth) {
    if (Number(call.operation) === 1) return 'delegatecall';
    var data = call.data || '0x';
    var selector = abi.selectorOf(data);
    if (!selector) return null; // empty calldata: the fee leg's own shape
    if (account && String(call.to || '').toLowerCase() === String(account).toLowerCase() && SELF_CALL[selector]) {
      return SELF_CALL[selector];
    }
    if (depth >= MAX_NESTING) return null;
    var inner = [];
    if (selector === MULTI_SEND_SELECTOR) {
      var packed = abi.decode('multiSend(bytes)', data);
      if (packed) inner = decodeMultiSend(packed[0]);
    } else if (selector === EXEC_TRANSACTION_SELECTOR) {
      var exec = abi.decode(EXEC_TRANSACTION, data);
      if (exec) inner = [{ to: exec[0], data: exec[2], operation: Number(exec[3]) }];
    }
    for (var i = 0; i < inner.length; i++) {
      var found = takeover(inner[i], account, depth + 1);
      if (found) return found;
    }
    return null;
  }

  function refuseTakeover(fn, view) {
    view.refuse = true;
    view.risk = 'danger';
    view.warnings.push(fn === 'delegatecall'
      ? { tone: 'danger', key: 'refuse.delegateCall' }
      : { tone: 'danger', key: 'refuse.selfCall', params: { fn: fn } });
  }

  // --- a Safe this account owns (§31, the parent-Safe guard) ----------------
  //
  // A Vela account can be a member — an owner — of other Safes: every Vela
  // multisig it belongs to. A site can make the account act for one of those
  // without ever pointing at the account itself: `execTransaction` with the
  // "approved by msg.sender" signature (a 1-of-N takeover), `approveHash` (one
  // counting approval), an owner, threshold, module, guard or fallback change,
  // the same hidden in a `multiSend` / `execTransaction` payload, or a
  // `SafeMessage` signature under that Safe's domain (a nested EIP-1271
  // approval). The core refuses these for every Safe this account owns — this
  // device's multisigs, or any other whose `isOwner` reads true
  // (`parent_safe_guard.rs`). This page cannot read the chain (its CSP has no
  // `connect-src`) and does not trust the app's word for which Safes those
  // are, so it refuses the shapes whatever their target, the account aside.
  // Vela's own multisig execution is read above (`execution`), never here.

  var PARENT_CALL = Object.assign({ '0xd4d9bdcd': 'approveHash' }, SELF_CALL);

  /** The Safe function a call would run for a Safe other than the account, or null. */
  function parentSafeCall(call, account, depth) {
    var data = call.data || '0x';
    var selector = abi.selectorOf(data);
    if (!selector) return null;
    var to = String(call.to || '').toLowerCase();
    if (to && to !== String(account || '').toLowerCase() && PARENT_CALL[selector]) {
      return PARENT_CALL[selector];
    }
    if (depth >= MAX_NESTING) return null;
    var inner = [];
    if (selector === MULTI_SEND_SELECTOR) {
      var packed = abi.decode('multiSend(bytes)', data);
      if (packed) inner = decodeMultiSend(packed[0]);
    } else if (selector === EXEC_TRANSACTION_SELECTOR) {
      var exec = abi.decode(EXEC_TRANSACTION, data);
      if (exec) inner = [{ to: exec[0], data: exec[2], operation: Number(exec[3]) }];
    }
    for (var i = 0; i < inner.length; i++) {
      var found = parentSafeCall(inner[i], account, depth + 1);
      if (found) return found;
    }
    return null;
  }

  /** Every typed-data document of the request, parsed (a string or an object). */
  function typedDocuments(intent) {
    var params = intent.params || [];
    var docs = [];
    for (var i = 0; i < params.length; i++) {
      var entry = params[i];
      if (typeof entry === 'string') {
        try { entry = JSON.parse(entry); } catch (e) { continue; }
      }
      if (entry && typeof entry === 'object' && !Array.isArray(entry)) docs.push(entry);
    }
    return docs;
  }

  /** A `SafeMessage` under another contract's domain: an approval for that Safe. */
  function isParentSafeMessage(intent, account) {
    return typedDocuments(intent).some(function (doc) {
      var contract = String((doc.domain && doc.domain.verifyingContract) || '').toLowerCase();
      return doc.primaryType === 'SafeMessage' && contract !== '' &&
        contract !== String(account || '').toLowerCase();
    });
  }

  /** A `SafeTx` is an instruction to a Safe, executable by anyone later. */
  function isSafeTx(intent) {
    var params = intent.params || [];
    for (var i = 0; i < params.length; i++) {
      var entry = params[i];
      if (typeof entry === 'string') {
        try { entry = JSON.parse(entry); } catch (e) { continue; }
      }
      if (entry && typeof entry === 'object' && entry.primaryType === 'SafeTx') return true;
    }
    return false;
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
   * When the operation stops being valid (§35), in the page's date form — or,
   * past what a date can hold (a `uint48` second reaches year 8.9 million),
   * the Unix second itself, so the card never says "NaN".
   */
  function expiryOf(until) {
    var shown = formatDate(until.toString());
    return shown.indexOf('NaN') >= 0 ? until.toString() : shown;
  }

  /**
   * When an assembled operation is present, IT is the subject: the digest
   * covers its calldata, so that is what must be on screen. The intent the
   * site sent is then only used to check that its call is really in there.
   */
  function operationSubject(intent, ctx, view) {
    var operation = ctx.operation && ctx.operation.userOp;
    if (!operation) return intent;

    // §35: the operation expires; one that never does is refused (the
    // digest's own rule), and when it does the card says until when. A
    // window that does not read is refused in its own words.
    var validity = ns.safeop.windowOf(operation);
    if (!validity || validity.until === BigInt(0)) {
      view.refuse = true;
      view.warnings.push({ tone: 'danger', key: validity ? 'refuse.noExpiry' : 'refuse.badWindow' });
    } else {
      view.expires = expiryOf(validity.until);
    }
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
    // Spec 079: the fee leg is the fee row, not a second leg of a "batch" — a
    // plain send read as 批量 of two. Only a leg the fee row shows in full
    // leaves the list: `inBandLeg` decoded it as a plain payment, and the row
    // draws its amount and its recipient from the calldata. Anything else the
    // requester calls the fee stays a leg, drawn like every other.
    var feeIndex = view.fee && view.fee.leg ? view.fee.leg.index : -1;
    var shown = decoded.calls.filter(function (_, i) { return i !== feeIndex; });
    if (!shown.length) shown = decoded.calls;
    return shown.length === 1
      ? { method: 'eth_sendTransaction', origin: intent.origin, params: [shown[0]] }
      : { method: 'wallet_sendCalls', origin: intent.origin, params: [{ calls: shown }] };
  }

  // --- a multisig member's approval (vela_multisigApprove) -----------------
  //
  // The app hands over a SafeTx, the multisig and the chain. Everything on
  // the card is read here from those bytes (lib/multisig.js, the core's own
  // rules): the hashes are recomputed, the reading is the core's `review`,
  // and the passkey signs the approval bytes lib/digest.js derives from the
  // hash THIS page computed. The app's words — the multisig's name, the
  // member's name, the network's name on a chain this page does not know —
  // are drawn as the app's.

  var MEMBER_INTENTS = {
    addOwner: 'intent.msAddOwner',
    removeOwner: 'intent.msRemoveOwner',
    swapOwner: 'intent.msSwapOwner',
    changeThreshold: 'intent.msThreshold',
  };

  function refuseWith(view, key, params) {
    view.refuse = true;
    raise(view, 'danger');
    view.warnings.push({ tone: 'danger', key: key, params: params });
  }

  /** A member, by its address and the avatar derived from it — never a name. */
  function memberIdentity(address, ctx) {
    return identify(address, ctx, {});
  }

  /**
   * r9-10: "1 approval", never "1 approvals" — the sentence's singular form
   * for a threshold of one (every catalog has it; where a language has one
   * form, it is the same words).
   */
  function one(threshold) {
    return String(threshold) === '1' ? 'One' : '';
  }

  /** The most members a claim may name: more is not a Vela multisig's, and not drawn. */
  var MAX_MEMBERS = 256;

  /**
   * r10: the multisig's members and the approvals it needs NOW, as the app
   * says (`context.multisig.owners` / `.threshold`, vela-core
   * `approval::request` / `approval::execution_context`). This page cannot
   * read the chain, so they are the app's word and drawn as such. Only a
   * well-formed claim is used — distinct addresses, a threshold within them;
   * anything else is no claim (the change is drawn as before, without the
   * "before").
   */
  function membersClaim(ctx) {
    var about = ctx.multisig && typeof ctx.multisig === 'object' && !Array.isArray(ctx.multisig) ? ctx.multisig : {};
    var owners = about.owners;
    var threshold = about.threshold;
    if (!Array.isArray(owners) || !owners.length || owners.length > MAX_MEMBERS) return null;
    if (!owners.every(function (o) { return typeof o === 'string' && ADDRESS.test(o); })) return null;
    var lower = owners.map(function (o) { return o.toLowerCase(); });
    if (lower.some(function (o, i) { return lower.indexOf(o) !== i; })) return null;
    if (typeof threshold !== 'number' || !Number.isSafeInteger(threshold) || threshold < 1 || threshold > lower.length) {
      return null;
    }
    return { owners: lower, threshold: BigInt(threshold), byApp: true };
  }

  /**
   * The members after `action` on `now`, or null when the change does not
   * fit them (adds a member it has, removes or replaces one it has not,
   * asks more approvals than members, or none) — the Safe itself would
   * refuse it, or the app's account of the members is wrong.
   */
  function membersAfter(now, action) {
    var owners = now.owners.slice();
    var threshold = now.threshold;
    var has = function (a) { return owners.indexOf(String(a).toLowerCase()) >= 0; };
    if (action.kind === 'addOwner') {
      if (has(action.owner)) return null;
      owners.push(action.owner.toLowerCase());
      threshold = BigInt(action.threshold);
    } else if (action.kind === 'removeOwner') {
      if (!has(action.owner)) return null;
      owners.splice(owners.indexOf(action.owner.toLowerCase()), 1);
      threshold = BigInt(action.threshold);
    } else if (action.kind === 'swapOwner') {
      if (!has(action.oldOwner) || has(action.newOwner)) return null;
      owners[owners.indexOf(action.oldOwner.toLowerCase())] = action.newOwner.toLowerCase();
    } else {
      threshold = BigInt(action.threshold);
    }
    if (threshold < 1n || threshold > BigInt(owners.length)) return null;
    return { owners: owners, threshold: threshold, byApp: now.byApp };
  }

  /**
   * r10: "Members 3 → 4 · Approvals 2 of 3 → 2 of 4" — the change against
   * the members now. The "after" is read from the bytes; the "before" is
   * the app's (`tag.byApp`) unless this very operation sets the multisig up
   * (its setup, verified here, says it). A batch's member changes run one
   * after another, so each leg's "before" is the leg before's "after".
   * Returns whether the rows were drawn.
   */
  function beforeAfter(action, ctx, view) {
    var now = ctx.membersNow;
    if (!now) return false;
    var after = membersAfter(now, action);
    if (!after) {
      // The change and the members the app names do not fit: no figures
      // this page cannot stand behind, and a caution saying so.
      ctx.membersNow = null;
      raise(view, 'caution');
      view.warnings.push({ tone: 'caution', key: 'warn.msMembersClaimOff' });
      return false;
    }
    var tag = now.byApp ? 'tag.byApp' : undefined;
    view.fields.push({
      label: 'field.msMembers',
      value: text('value.msCountChange', { before: now.owners.length, after: after.owners.length }),
      emphasis: true, tagKey: tag,
    });
    // "Approvals needed", not "Approvals": an execution's card already has
    // an "Approvals" row — the ones its signatures carry.
    view.fields.push({
      label: 'field.threshold',
      value: text('value.msApprovalsChange', {
        before: now.threshold.toString(), beforeOf: now.owners.length,
        after: after.threshold.toString(), afterOf: after.owners.length,
      }),
      emphasis: true, tagKey: tag,
    });
    ctx.membersNow = after;
    return true;
  }

  /** The rows and words of a member change, the multisig's own call. */
  function memberChange(action, ctx, view) {
    view.intentKey = MEMBER_INTENTS[action.kind];
    provenance(view, 'read');
    raise(view, 'caution');
    if (action.kind === 'addOwner' || action.kind === 'removeOwner') {
      var who = memberIdentity(action.owner, ctx);
      view.fields.push({ label: action.kind === 'addOwner' ? 'field.newMember' : 'field.member', identity: who });
      if (!beforeAfter(action, ctx, view)) {
        view.fields.push({ label: 'field.threshold', value: action.threshold, emphasis: true });
      }
      view.sentence = text(
        (action.kind === 'addOwner' ? 'sentence.msAddOwner' : 'sentence.msRemoveOwner') + one(action.threshold),
        { owner: who.short, threshold: action.threshold });
    } else if (action.kind === 'swapOwner') {
      var gone = memberIdentity(action.oldOwner, ctx);
      var next = memberIdentity(action.newOwner, ctx);
      view.fields.push({ label: 'field.newMember', identity: next });
      view.fields.push({ label: 'field.oldMember', identity: gone });
      beforeAfter(action, ctx, view);
      view.sentence = text('sentence.msSwapOwner', { newOwner: next.short, oldOwner: gone.short });
    } else {
      if (!beforeAfter(action, ctx, view)) {
        view.fields.push({ label: 'field.threshold', value: action.threshold, emphasis: true });
      }
      view.sentence = text('sentence.msThreshold' + one(action.threshold), { threshold: action.threshold });
    }
    if (!view.warnings.some(function (w) { return w.key === 'warn.msMembers'; })) {
      view.warnings.push({ tone: 'caution', key: 'warn.msMembers' });
    }
  }

  /**
   * One action of the core's reading, drawn onto `view`: a transfer or a
   * call through the page's own call ladder (the same as a transaction's —
   * token standards are read, descriptors described, the rest a contract
   * call), a member change, a rename. `call` is the call as packed.
   */
  function multisigAction(action, call, ctx, view, isLeg) {
    if (MEMBER_INTENTS[action.kind]) return memberChange(action, ctx, view);
    if (action.kind === 'rename') {
      view.intentKey = 'intent.msRename';
      provenance(view, 'read');
      view.fields.push({ label: 'field.newName', value: visible(action.name, view), emphasis: true });
      view.sentence = text('sentence.msRename', { name: visible(action.name, view) });
      return view;
    }
    // A transfer (native or ERC-20) or a call to another contract.
    return resolveCall(call, ctx, view, isLeg);
  }

  function hexValue(decimal) {
    return '0x' + BigInt(decimal).toString(16);
  }

  function resolveMultisig(intent, ctx) {
    var request = ns.multisig.request(intent);
    // The chain the approval is for is the request's own (it is in the
    // digest's domain): the card names that one, whatever else the context says.
    var named = ctx.chainId;
    if (!request.error) ctx.chainId = request.chainId;
    ctx.selfAccount = request.error ? null : request.safe;
    ctx.selfTag = 'tag.thisMultisig';
    // r10: the members and approvals now, as the app says — a member change
    // reads before → after against them.
    ctx.membersNow = membersClaim(ctx);
    ctx.coin = coinOf(ctx);
    var view = baseView(intent, ctx);
    view.method = intent.method;
    view.intentKey = 'intent.msApprove';
    view.badge = 'tag.multisig';
    view.hero = null;

    if (request.error) {
      refuseWith(view, request.error);
      view.account = null;
      return finishSigning(view, ctx);
    }
    var tx = request.tx;
    view.account = request.member;
    if (named !== undefined && named !== null && Number(named) !== request.chainId) {
      refuseWith(view, 'refuse.msChainMismatch');
    }
    // The member the approval is made as is in the digest (its domain): the
    // context's account — whose keys the browser is asked for — must be it.
    if (typeof ctx.account === 'string' && ctx.account.toLowerCase() !== request.member.toLowerCase()) {
      refuseWith(view, 'refuse.msMemberMismatch');
    }

    var hashes = ns.multisig.hashes(request.chainId, request.safe, tx);
    var matches = hashes.safeTxHash === request.claimed;
    if (!matches) {
      // Loud and first: the app describes one transaction and hands over the
      // fields of another (or a hash of something else entirely).
      view.refuse = true;
      view.risk = 'hard-danger';
      view.warnings.unshift({
        tone: 'danger', key: 'refuse.msHashMismatch',
        params: { claimed: request.claimed, computed: hashes.safeTxHash },
      });
    }

    // The multisig: its address and avatar are facts (the address is in the
    // safeTxHash's domain); its name is the app's.
    var name = ctx.multisig && typeof ctx.multisig.name === 'string' && ctx.multisig.name.trim()
      ? visible(ctx.multisig.name.trim().slice(0, 64), view) : null;
    var safeIdentity = identify(request.safe, ctx, {});
    safeIdentity.contractName = name;
    safeIdentity.tag = name ? 'tag.nameByApp' : null;
    view.multisig = {
      address: request.safe,
      name: name,
      nonce: tx.nonce.toString(),
      hashes: hashes,
      hashMatches: matches,
    };

    var reading = ns.multisig.read(request.safe, tx);
    var call = { to: tx.to, value: hexValue(tx.value), data: tx.data };
    if (reading.refusal) {
      refuseWith(view, reading.refusal, reading.params);
      view.tech = techFor('SafeTx (EIP-712)', null, tx.data, {
        addresses: [{ roleKey: 'field.target', name: shortAddress(tx.to), address: tx.to }],
      });
    } else if (reading.action.kind === 'cancel') {
      view.intentKey = 'intent.msReject';
      provenance(view, 'read');
      view.sentence = text('sentence.msReject', { nonce: tx.nonce.toString() });
    } else if (reading.action.kind === 'batch') {
      view.intentKey = 'intent.batch';
      view.legs = flattenLegs(reading.action.calls.map(function (leg, i) {
        var legView = baseView(intent, ctx);
        multisigAction(leg.action, { to: leg.to, value: hexValue(leg.value), data: leg.data }, ctx, legView, true);
        return { index: i + 1, view: legView };
      }), view);
      view.sentence = text('sentence.msBatch', { count: reading.action.calls.length });
      view.risk = view.legs.reduce(function (worst, leg) {
        return rank(leg.view.risk) > rank(worst) ? leg.view.risk : worst;
      }, view.risk);
      view.refuse = view.refuse || view.legs.some(function (leg) { return leg.view.refuse; });
      liftLegWarnings(view);
      provenance(view, 'read');
      view.tech = techFor('multiSend(bytes) · MultiSendCallOnly (delegatecall)', null, tx.data, {
        addresses: [{ roleKey: 'field.target', name: 'MultiSendCallOnly', address: tx.to }],
      });
    } else {
      var badge = view.badge;
      multisigAction(reading.action, call, ctx, view, false);
      view.badge = badge;
      if (!view.tech) view.tech = techFor('SafeTx (EIP-712)', null, tx.data, {});
    }

    // The rows every approval carries: which multisig, at which nonce.
    view.fields.unshift({ label: 'field.multisig', identity: safeIdentity });
    view.fields.push({ label: 'field.nonce', value: tx.nonce.toString(), mono: true });

    var finished = finishSigning(view, ctx);
    if (!finished.refuse) {
      var cancel = reading.action && reading.action.kind === 'cancel';
      // r9-10: "… anyway" exactly where the app's one review asks for it
      // (`context.multisig.anyway` — what this page cannot check, a run
      // that fails, said as the app's word) or this page's own reading
      // finds a danger; in the danger style, never the plain accent.
      var appAnyway = !!(ctx.multisig && ctx.multisig.anyway === true);
      var danger = finished.risk === 'danger' || finished.risk === 'hard-danger';
      if (appAnyway && !danger) {
        raise(finished, 'danger');
        finished.warnings.push({ tone: 'danger', key: 'warn.msAppAnyway' });
      }
      finished.anyway = appAnyway || danger;
      finished.confirmKey = cancel ? 'button.msReject'
        : finished.anyway ? 'button.msApproveAnyway' : 'button.msApprove';
    }
    return finished;
  }

  // --- a member's owner proof (vela_multisigCreate, §16) -------------------
  //
  // Creating a multisig (or sharing its record again) asks one member's
  // passkey for the genesis service's sponsor challenge: the proof the
  // service needs before it records the multisig where its members find it.
  // The app hands over the GENESIS; lib/multisig.js `ownerProof` reads the
  // address, the members, the rule and the challenge from it by the core's
  // rule, and refuses anything it cannot vouch for. The card says what the
  // person proves — which multisig, whose members, which rule, their own
  // place — every fact read here; the networks and the name are the app's.

  /** The networks, by this page's own names where it has them, else their ids. */
  function networkNames(chainIds) {
    return chainIds.map(function (id) {
      return reg.chainName(id) || ns.i18n.t('value.chainId', { id: id });
    }).join(', ');
  }

  function resolveOwnerProof(intent, ctx) {
    var proof = ns.multisig.ownerProof(intent && intent.params);
    var reading = proof.reading || null;
    // Ours: the member whose keys sign is "you", and no site is asking.
    ctx.siteRelayed = false;
    ctx.selfAccount = reading ? reading.member : null;
    ctx.selfTag = null;
    ctx.membersNow = null;
    if (reading) ctx.chainId = reading.chainIds[0];
    ctx.coin = coinOf(ctx);
    var view = baseView(intent, ctx);
    view.method = intent && intent.method;
    view.intentKey = 'intent.msOwnerProof';
    view.badge = 'tag.multisig';
    view.hero = null;
    // The networks are a row of their own below (the app's word): the one
    // network row a signature carries would name only the first.
    view.chain = null;
    view.chainId = null;

    if (proof.refusal) {
      refuseWith(view, proof.refusal);
      if (proof.refusal === 'refuse.msOwnerAddress') view.risk = 'hard-danger';
      view.account = typeof ctx.account === 'string' && ADDRESS.test(ctx.account) ? ctx.account : null;
      view.tech = techFor('vela_multisigCreate', null, null, {});
      return finishSigning(view, ctx);
    }
    view.account = reading.member;
    // The member who proves is in the challenge's request (its keys answer):
    // the context's account — whose keys the browser is asked for — must be it.
    if (typeof ctx.account === 'string' && ctx.account.toLowerCase() !== reading.member.toLowerCase()) {
      refuseWith(view, 'refuse.msMemberMismatch');
    }
    if (suppliedChallenge(intent, ctx)) {
      refuseWith(view, 'refuse.suppliedChallenge');
    }

    var name = reading.name !== null ? visible(reading.name, view) : null;
    var safeIdentity = identify(reading.multisig, ctx, {});
    safeIdentity.contractName = name;
    safeIdentity.tag = name ? 'tag.nameByApp' : null;
    view.fields.push({ label: 'field.multisig', identity: safeIdentity });
    view.fields.push({ label: 'field.msNetworks', value: networkNames(reading.chainIds), tagKey: 'tag.byApp' });
    reading.owners.forEach(function (owner, i) {
      view.fields.push({
        label: 'field.msMemberAt', labelParams: { n: i + 1 },
        identity: memberIdentity(owner, ctx),
      });
    });
    view.fields.push({
      label: 'field.threshold',
      value: text('value.msNeeds', { m: reading.threshold, n: reading.owners.length }),
      emphasis: true,
    });
    view.sentence = text('sentence.msOwnerProof', {
      place: reading.place, count: reading.owners.length,
    });
    // Not a transaction: read from what is signed — the genesis it derives.
    provenance(view, 'readMessage');
    view.tech = techFor('keccak256("safe-genesis-registry/sponsor/v1" ‖ multisig)', null, null, {
      params: [
        { name: 'multisig', value: reading.multisig },
        { name: 'saltNonce', value: String(intent.params[0].genesis.saltNonce) },
        { name: 'initializer', value: String(intent.params[0].genesis.initializer) },
        { name: 'challenge', value: reading.challenge },
      ],
      addresses: [
        { roleKey: 'ui.contract', name: 'SafeProxyFactory v1.4.1', address: intent.params[0].genesis.factory },
        { roleKey: 'ui.contract', name: 'SafeL2 v1.4.1', address: intent.params[0].genesis.singleton },
      ],
    });

    var finished = finishSigning(view, ctx);
    if (!finished.refuse) finished.confirmKey = 'button.msOwnerProof';
    return finished;
  }

  // --- an executor's operation (round 7, r7-1) ------------------------------
  //
  // The wallet's own operation that executes a multisig transaction: read
  // leg by leg in lib/multisig.js (`execution`) — the SafeTx inside
  // `execTransaction` by the approval's rules, the deploys it needs verified
  // from their setups — and drawn as what it is: "Execute a multisig
  // transaction", the transaction's own reading (its amount, its recipient,
  // its member change…), the approvals it carries, its hashes, and what it
  // sets up on the way. The app's words — the multisig's name, the number
  // the transaction runs at, how many approvals it needs when this operation
  // does not set the multisig up — are drawn as the app's.

  /** The app's account of the execution (`context.multisig`, vela-core `approval::execution_context`). */
  function executionClaim(ctx) {
    var claim = ctx.multisig && typeof ctx.multisig === 'object' && !Array.isArray(ctx.multisig) ? ctx.multisig : {};
    var threshold = typeof claim.threshold === 'number' && Number.isSafeInteger(claim.threshold) && claim.threshold > 0
      ? BigInt(claim.threshold) : null;
    return {
      address: typeof claim.address === 'string' && ADDRESS.test(claim.address) ? claim.address : null,
      name: typeof claim.name === 'string' && claim.name.trim() ? claim.name.trim().slice(0, 64) : null,
      nonce: typeof claim.nonce === 'string' ? ns.multisig.quantity(claim.nonce) : null,
      safeTxHash: typeof claim.safeTxHash === 'string' && /^0x[0-9a-fA-F]{64}$/.test(claim.safeTxHash)
        ? claim.safeTxHash.toLowerCase() : null,
      threshold: threshold,
      // D1: the app's one review asks for "… anyway".
      anyway: claim.anyway === true,
    };
  }

  function resolveExecution(reading, ctx, view) {
    view.badge = 'tag.multisig';
    provenance(view, 'read');
    if (reading.refusal) {
      view.intentKey = 'intent.msExecute';
      view.hero = null;
      refuseWith(view, reading.refusal);
      return view;
    }
    var claim = executionClaim(ctx);
    var multisig = reading.multisig;
    var tx = Object.assign({}, reading.tx, { nonce: claim.nonce });
    // The transaction's own reading is the multisig's: "this multisig" is
    // the one the operation executes on, never the executor.
    var inner = Object.assign({}, ctx, { selfAccount: multisig, selfTag: 'tag.thisMultisig' });
    // r10: a member change reads before → after. When this operation sets the
    // multisig up, its setup (verified above, in lib/multisig.js) says who the
    // members are — a fact; otherwise they are the app's word.
    inner.membersNow = reading.deploy
      ? {
        owners: reading.deploy.owners.map(function (o) { return o.toLowerCase(); }),
        threshold: reading.deploy.threshold, byApp: false,
      }
      : membersClaim(ctx);
    if (claim.address && claim.address.toLowerCase() !== multisig.toLowerCase()) {
      refuseWith(view, 'refuse.msExecMismatch');
    }

    var action = ns.multisig.read(multisig, tx);
    var call = { to: tx.to, value: hexValue(tx.value), data: tx.data };
    if (action.refusal) {
      refuseWith(view, action.refusal, action.params);
    } else if (action.action.kind === 'cancel') {
      view.sentence = text('sentence.msExecCancel');
    } else if (action.action.kind === 'batch') {
      // r10: every leg of every level, as a member's approval lists them
      // (r8-5) — a leg that is itself a batch was drawn with nothing under
      // it here, its own legs never shown, and no depth limit applied.
      view.legs = flattenLegs(action.action.calls.map(function (leg, i) {
        var legView = baseView({ origin: '' }, inner);
        multisigAction(leg.action, { to: leg.to, value: hexValue(leg.value), data: leg.data }, inner, legView, true);
        return { index: i + 1, view: legView };
      }), view);
      view.sentence = text('sentence.msBatch', { count: action.action.calls.length });
      view.legs.forEach(function (leg) { raise(view, leg.view.risk); });
      view.refuse = view.refuse || view.legs.some(function (leg) { return leg.view.refuse; });
      liftLegWarnings(view);
    } else {
      var drawn = baseView({ origin: '' }, inner);
      multisigAction(action.action, call, inner, drawn, false);
      // What the multisig does, on this card: its hero, rows, warnings and
      // risk — said as the multisig's ("The multisig sends …").
      view.hero = drawn.hero;
      view.fields = view.fields.concat(drawn.fields);
      // r10: a lone call that is itself a batch (a CALL to a MultiSend) has
      // its legs listed — they were read, then left off this card.
      view.legs = drawn.legs || [];
      view.warnings = view.warnings.concat(drawn.warnings);
      view.tech = drawn.tech;
      view.level = drawn.level;
      raise(view, drawn.risk);
      if (drawn.refuse) view.refuse = true;
      view.sentence = drawn.sentence && drawn.sentence.key === 'sentence.transfer'
        ? text('sentence.msSends', drawn.sentence.params) : drawn.sentence;
    }
    view.intentKey = 'intent.msExecute';
    view.lead = claim.nonce === null
      ? text('sentence.msExecuteNoNonce')
      : text('sentence.msExecute', { nonce: claim.nonce.toString() });

    // The multisig, at which number, with which hashes. The number is the
    // app's: `execTransaction` carries none (the multisig uses its own).
    var safeIdentity = identify(multisig, ctx, {});
    safeIdentity.contractName = claim.name ? visible(claim.name, view) : null;
    safeIdentity.tag = claim.name ? 'tag.nameByApp' : null;
    view.fields.unshift({ label: 'field.multisig', identity: safeIdentity });
    if (claim.nonce !== null) {
      var hashes = ns.multisig.hashes(ctx.chainId, multisig, tx);
      var matches = claim.safeTxHash === null || claim.safeTxHash === hashes.safeTxHash;
      if (!matches) {
        // The app describes one transaction and the operation runs another.
        view.refuse = true;
        view.risk = 'hard-danger';
        view.warnings.unshift({
          tone: 'danger', key: 'refuse.msHashMismatch',
          params: { claimed: claim.safeTxHash, computed: hashes.safeTxHash },
        });
      }
      view.multisig = {
        address: multisig, name: safeIdentity.contractName, nonce: claim.nonce.toString(),
        hashes: hashes, hashMatches: matches, noteKey: 'ui.msHashesExecNote',
      };
      view.fields.push({ label: 'field.nonce', value: claim.nonce.toString(), mono: true, tagKey: 'tag.byApp' });
    } else {
      view.warnings.push({ tone: 'caution', key: 'warn.msExecNoNonce' });
    }

    // The approvals it carries, counted from its signature bytes; how many
    // it needs is a fact when this operation sets the multisig up.
    var count = reading.approvals.length;
    var actual = reading.deploy ? reading.deploy.threshold : null;
    if (actual !== null && claim.threshold !== null && actual !== claim.threshold) {
      refuseWith(view, 'refuse.msExecThreshold', { claimed: claim.threshold.toString(), actual: actual.toString() });
    }
    var needed = actual !== null ? actual : claim.threshold;
    view.fields.push({
      label: 'field.approvals',
      value: needed === null ? String(count) : text('value.approvalsOf', { count: count, threshold: needed.toString() }),
      emphasis: true,
      tagKey: actual === null && needed !== null ? 'tag.byApp' : undefined,
    });
    reading.approvals.forEach(function (approval) {
      view.fields.push(approval.owner
        ? { label: 'field.approvedBy', identity: memberIdentity(approval.owner, ctx) }
        : { label: 'field.approvedBy', valueKey: 'value.msWalletSignature' });
    });

    // What it sets up on the way.
    view.notes = [];
    if (reading.deploy) {
      view.notes.push(text('sentence.msExecSetsUp', {
        chain: view.chain || String(ctx.chainId), count: reading.deploy.owners.length,
        threshold: reading.deploy.threshold.toString(),
      }));
      reading.deploy.owners.forEach(function (owner) {
        view.fields.push({ label: 'field.member', identity: memberIdentity(owner, ctx) });
      });
    }
    reading.activates.forEach(function (account) {
      var who = memberIdentity(account, ctx);
      view.notes.push(text('sentence.msExecActivates', { account: who.short }));
      view.fields.push({ label: 'field.activates', identity: who });
    });

    if (!view.tech) {
      view.tech = techFor('execTransaction · SafeTx', null, tx.data, {
        addresses: [{ roleKey: 'field.multisig', name: shortAddress(multisig), address: multisig }],
      });
    }

    // D1 (the approval's r9-10, for the executor): "… anyway" exactly where
    // the app's one review asks for it (`context.multisig.anyway` — a danger
    // this page cannot check, or neither of the app's safety checks read),
    // in the danger style, never the plain accent "Execute".
    if (!view.refuse && claim.anyway) {
      if (rank(view.risk) < rank('danger')) {
        raise(view, 'danger');
        view.warnings.push({ tone: 'danger', key: 'warn.msAppAnywayExec' });
      }
      view.anyway = true;
    }
    return view;
  }

  /**
   * What every signature card ends with, whatever it signs: the answer must
   * reach the wallet (R7), the keys must be readable, and the key route and
   * the button are this page's own.
   */
  function finishSigning(view, ctx) {
    // Spec 102 R7: a signature goes to the Vela wallet on this device and
    // nowhere else. Any site can open this page with an operation of its
    // choosing and its own answer address; the card would describe that
    // operation truthfully, and a person who tapped would hand a usable
    // signature to whoever named the address. So nothing is offered — the
    // button is off, no passkey prompt — and the card says where the answer would
    // have gone. Fails closed: a context that does not say the answer
    // reaches the wallet is refused, previews included (they pass the
    // address the apps send).
    if (!ns.resolve.answersToWallet(ctx)) {
      view.refuse = true;
      view.risk = 'danger';
      var destination = answerDestination(ctx);
      view.warnings.unshift(destination
        ? { tone: 'danger', key: 'refuse.answerElsewhere', params: { to: destination } }
        : { tone: 'danger', key: 'refuse.answerNotToWallet' });
    }

    // The account's keys, as the browser will be asked for them. One that
    // cannot be read cannot be asked for: refused here, with the reason,
    // rather than failing inside the tap.
    var allow = ctx.allowCredentials;
    if (allow !== undefined && allow !== null && !(Array.isArray(allow) && allow.every(credentialIdOk))) {
      view.refuse = true;
      view.risk = 'danger';
      view.warnings.push({ tone: 'danger', key: 'refuse.badKeyId' });
    }

    // The key that signs, and where it lives (spec 102 R5). sign.js asks for
    // this one credential; the card says where the person will confirm.
    view.key = keyRoute(ctx.keyRoute, ctx.allowCredentials);
    view.keyLabel = 'field.confirmWith';
    view.confirmKey = confirmKey(view);
    return view;
  }

  function resolve(requested, ctx) {
    // Ours, never the requester's: whatever `selfAccount` it sent is replaced.
    ctx = Object.assign({}, ctx || {});
    // What is being asked, for the answer rule (`answersToWallet`): never the
    // requester's word for it, the intent's own method.
    ctx.method = requested && typeof requested.method === 'string' ? requested.method : '';
    if (ns.multisig && ctx.method === ns.multisig.METHOD) return resolveMultisig(requested, ctx);
    if (ns.multisig && ctx.method === ns.multisig.OWNER_PROOF) return resolveOwnerProof(requested, ctx);
    ctx.siteRelayed = relayedSite(requested, ctx);
    ctx.selfAccount = selfAccountOf(requested, ctx);
    // Ours too (r10): "this multisig", and the multisig's coins in the words,
    // only where this page reads a multisig's transaction; the members the
    // app says it has only where this page puts them.
    ctx.selfTag = null;
    ctx.membersNow = null;
    ctx.coin = coinOf(ctx);
    var view = baseView(requested, ctx);
    var ceremony = ns.ceremony && ns.ceremony.kindOf(requested && requested.method);
    if (ceremony) return resolveCeremony(ceremony, requested, ctx, view);
    var intent = operationSubject(requested, ctx, view);
    view.method = intent.method;
    // The wallet's own operation that executes a multisig transaction (r7-1):
    // read whole, or refused — never a batch of blind calls.
    var shownCalls = intent.method === 'eth_sendTransaction' ? [intent.params[0] || {}]
      : intent.method === 'wallet_sendCalls' ? ((intent.params[0] || {}).calls || []) : [];
    var execution = ctx.operation && ctx.operation.userOp && !requested.origin && ns.multisig
      ? ns.multisig.execution(shownCalls, ctx.operation.userOp.sender) : null;

    if (execution) {
      resolveExecution(execution, ctx, view);
    } else if (intent.method === 'eth_sendTransaction') {
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
      }, rank(view.risk) > rank('normal') ? view.risk : 'normal');
      // OR, never overwrite: `operationSubject` may already have refused (the
      // operation does not contain the site's call). Every Vela operation has
      // at least two legs — the call and the fee — so an overwrite here would
      // offer the button for exactly the tampered operation it exists to stop.
      view.refuse = view.refuse || view.legs.some(function (leg) { return leg.view.refuse; });
      liftLegWarnings(view);
      if (ctx.simulation && ctx.simulation.rows) view.balance = claimedBalance(ctx.simulation, view);
      provenance(view, 'read');
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

    // Checked last, over whatever the method branches decided, and only ever
    // adding a refusal: every call of the operation (or of the request when no
    // operation came), not just the ones the site asked for — an app that
    // slipped in a leg of its own is exactly what this page is here to catch.
    var account = (ctx.operation && ctx.operation.userOp && ctx.operation.userOp.sender) ||
      ctx.account || null;
    var calls = shownCalls;
    for (var c = 0; c < calls.length; c++) {
      // An execution's `execTransaction` was read by the multisig's own rules
      // (MultiSendCallOnly is the one delegatecall they allow, as the core's
      // review does): only its target is checked here — the account calling
      // itself — not what runs inside it.
      var executes = execution && !execution.refusal && calls[c].to &&
        String(calls[c].to).toLowerCase() === execution.multisig.toLowerCase();
      var fn = takeover(calls[c], account, executes ? MAX_NESTING : 0);
      if (fn) { refuseTakeover(fn, view); break; }
    }
    if (intent.method && intent.method.indexOf('signTypedData') >= 0 && isSafeTx(intent)) {
      view.refuse = true;
      view.risk = 'danger';
      view.warnings.push({ tone: 'danger', key: 'refuse.safeTx' });
    }
    // §31: nothing a site asks may act for another Safe — every call of the
    // operation, the multisig execution Vela itself assembled aside.
    for (var p = 0; p < calls.length; p++) {
      var ownExecution = execution && !execution.refusal && calls[p].to &&
        String(calls[p].to).toLowerCase() === execution.multisig.toLowerCase();
      if (ownExecution) continue;
      var parentFn = parentSafeCall(calls[p], account, 0);
      if (parentFn) {
        view.refuse = true;
        view.risk = 'danger';
        view.warnings.push({ tone: 'danger', key: 'refuse.parentSafe', params: { fn: parentFn } });
        break;
      }
    }
    if (intent.method && intent.method.indexOf('signTypedData') >= 0 && isParentSafeMessage(intent, account)) {
      view.refuse = true;
      view.risk = 'danger';
      view.warnings.push({ tone: 'danger', key: 'refuse.parentSafeMessage' });
    }

    if (!view.dapp.originVerified && view.dapp.origin) {
      // About the requester line, so drawn under it (`near`), not as a banner.
      // A site a wallet relays is the WALLET's word, not the site's own.
      view.warnings.push({ tone: 'caution', key: ctx.siteRelayed ? 'warn.relayedOrigin' : 'warn.claimedOrigin', near: 'requester' });
    }

    return finishSigning(view, ctx);
  }

  ns.resolve = resolve;
  ns.resolve.typedDocument = typedDocument;
  ns.resolve.walletRequester = walletRequester;
  ns.resolve.keyRoute = keyRoute;
  ns.resolve.answersToWallet = answersToWallet;
  ns.resolve.EXTENSIONS = EXTENSIONS.slice();
  ns.resolve.isSafariExtension = isSafariExtension;
  // The waiting card draws the same mark as the request card.
  ns.resolve.toneFor = toneFor;
  ns.format = { units: formatUnits, date: formatDate, short: shortAddress, group: group, amountPhrase: amountPhrase };
})(window.VelaCS);
