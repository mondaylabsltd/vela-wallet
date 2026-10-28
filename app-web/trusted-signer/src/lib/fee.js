// What this signature can cost — read out of the signed bytes, never quoted.
//
// A requester that hands over "~0.0021 ETH ≈ $5.40" as text is choosing what
// the screen says while the operation decides what the account pays. The two
// have no reason to agree, and nobody would notice if they didn't.
//
// Vela pays IN BAND: the fee is an extra leg inside the operation's own
// calldata — a transfer of native coin or of a stablecoin to the relayer — and
// the 4337 gas fields are typically zero. That leg is inside the SafeOp digest
// like every other, so its token, amount and recipient are FACTS. The only
// claim in the whole arrangement is the label "this leg is the fee", and the
// sheet presents it as exactly that: a claim attached to a payment the reader
// can see for themselves.
//
// Fiat is multiplied here from a RATE the request declares. A rate can be
// wrong, but it is one checkable number printed next to the result, rather
// than a total nobody can verify.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var ERC20_TRANSFER = 'transfer(address,uint256)';

  function group(text) {
    return text.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  }

  function formatUnits(value, decimals, maxFrac) {
    var base = 10n ** BigInt(decimals);
    var whole = group((value / base).toString());
    var frac = decimals ? (value % base).toString().padStart(decimals, '0') : '';
    frac = frac.slice(0, maxFrac === undefined ? 6 : maxFrac).replace(/0+$/, '');
    return whole + (frac ? '.' + frac : '');
  }

  // The requester's currency, only as an ISO 4217 code ("USD"); anything
  // else is words on the card nobody checked, and no fiat is shown at all.
  var ISO_CURRENCY = /^[A-Z]{3}$/;

  function currencyOf(context) {
    var currency = context && context.currency;
    return typeof currency === 'string' && ISO_CURRENCY.test(currency) ? currency : null;
  }

  // A figure at the REQUESTER's rate, so render.js marks it as theirs.
  function fiatOf(context, symbol, value, decimals) {
    var currency = currencyOf(context);
    var rates = context.rates;
    var rate = currency && rates && typeof rates === 'object' && Object.prototype.hasOwnProperty.call(rates, symbol)
      ? rates[symbol] : undefined;
    if (typeof rate !== 'number' || !isFinite(rate) || rate <= 0) return null;
    var amount = (Number(value) / Math.pow(10, decimals)) * rate;
    if (!isFinite(amount)) return null;
    return {
      text: '≈ ' + currency + ' ' +
        (amount >= 1000 ? group(Math.round(amount).toString()) : amount.toFixed(2)),
      rate: rate,
    };
  }

  /**
   * The chain's own coin, from this page's catalogue for the chain id that is
   * signed for — not the requester's `nativeSymbol`, which used to name the
   * fee on every chain ("0.0009 USDC" on Ethereum), with "ETH" when it said
   * nothing. The hero's rule (resolve.js `coinOf`, handed over as
   * `context.coin`), so the card never names one coin two ways: only a chain
   * the page does not know takes the app's word, and the fee then says the
   * symbol is the app's (`claimed`); nobody naming it is "?".
   */
  function coinOf(context) {
    if (context.coin && typeof context.coin.symbol === 'string') return context.coin;
    var known = ns.registry && ns.registry.nativeSymbol(context.chainId);
    return { symbol: known || '?', claimed: false };
  }

  /** The 4337 gas ceiling. Zero on Vela, so it is only reported when it is not. */
  function gasCeiling(context, operation) {
    var gas = BigInt(operation.verificationGasLimit || 0) +
      BigInt(operation.callGasLimit || 0) +
      BigInt(operation.preVerificationGas || 0);
    var maxWei = gas * BigInt(operation.maxFeePerGas || 0);
    if (maxWei === 0n) return null;
    var coin = coinOf(context);
    var symbol = coin.symbol;
    var fiat = fiatOf(context, symbol, maxWei, 18);
    return {
      symbol: symbol,
      symbolClaimed: !!coin.claimed,
      gas: group(gas.toString()),
      max: formatUnits(maxWei, 18),
      fiat: fiat && fiat.text,
      rate: fiat && fiat.rate,
    };
  }

  /**
   * The in-band fee leg, READ from the calldata. `feeLegIndex` says which leg to
   * label; everything shown about it is decoded, not quoted. An index that does
   * not exist, or a leg that is not a plain payment, yields null rather than a
   * guess.
   */
  function inBandLeg(context, calls) {
    var index = context.operation && context.operation.feeLegIndex;
    if (index === undefined || index === null || !calls || !calls[index]) return null;
    var leg = calls[index];
    var value = BigInt(leg.value || 0);

    // Native coin: a bare value transfer with no calldata.
    if ((!leg.data || leg.data === '0x') && value > 0n) {
      var coin = coinOf(context);
      var symbol = coin.symbol;
      var nativeFiat = fiatOf(context, symbol, value, 18);
      return {
        index: index,
        symbol: symbol,
        symbolClaimed: !!coin.claimed,
        amount: formatUnits(value, 18),
        to: leg.to,
        token: null,
        fiat: nativeFiat && nativeFiat.text,
        rate: nativeFiat && nativeFiat.rate,
      };
    }

    // A stablecoin: transfer(recipient, amount) on the token contract.
    if (ns.abi.selectorOf(leg.data) === ns.keccak.selector(ERC20_TRANSFER)) {
      var decoded = ns.abi.decode(ERC20_TRANSFER, leg.data);
      if (!decoded) return null;
      // Known on THIS chain, or read as the integer it is (no guessed decimals).
      var known = ns.registry.token(leg.to, context.chainId);
      var decimals = known ? known.decimals : 0;
      var tokenSymbol = known ? known.symbol : '?';
      var tokenFiat = fiatOf(context, tokenSymbol, decoded[1], decimals);
      return {
        index: index,
        symbol: tokenSymbol,
        amount: formatUnits(decoded[1], decimals),
        to: decoded[0],
        token: leg.to,
        unverifiedDecimals: !known,
        fiat: tokenFiat && tokenFiat.text,
        rate: tokenFiat && tokenFiat.rate,
      };
    }
    return null;
  }

  /**
   * Returns null when nothing is charged (an off-chain signature), otherwise
   * { gas, leg, sponsored, currency } — either half may be null.
   */
  function derive(context, calls) {
    var operation = context && context.operation && context.operation.userOp;
    if (!operation) return null;
    var gas = gasCeiling(context, operation);
    var leg = inBandLeg(context, calls);
    if (!gas && !leg) return { gas: null, leg: null, unknown: true, currency: currencyOf(context) };
    return {
      gas: gas,
      leg: leg,
      currency: currencyOf(context),
      // A paymaster changes who pays; say so rather than showing a number that
      // will never leave this account.
      sponsored: !!(operation.paymasterAndData && operation.paymasterAndData !== '0x'),
    };
  }

  ns.fee = { derive: derive, inBandLeg: inBandLeg, gasCeiling: gasCeiling, formatUnits: formatUnits };
})(window.VelaCS);
