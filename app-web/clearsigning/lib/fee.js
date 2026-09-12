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

  function fiatOf(context, symbol, value, decimals) {
    var rate = context.rates && context.rates[symbol];
    if (typeof rate !== 'number' || !isFinite(rate)) return null;
    var amount = (Number(value) / Math.pow(10, decimals)) * rate;
    return {
      text: (context.currency || '$') +
        (amount >= 1000 ? group(Math.round(amount).toString()) : amount.toFixed(2)),
      rate: rate,
    };
  }

  /** The 4337 gas ceiling. Zero on Vela, so it is only reported when it is not. */
  function gasCeiling(context, operation) {
    var gas = BigInt(operation.verificationGasLimit || 0) +
      BigInt(operation.callGasLimit || 0) +
      BigInt(operation.preVerificationGas || 0);
    var maxWei = gas * BigInt(operation.maxFeePerGas || 0);
    if (maxWei === 0n) return null;
    var symbol = context.nativeSymbol || 'ETH';
    var fiat = fiatOf(context, symbol, maxWei, 18);
    return {
      symbol: symbol,
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
      var symbol = context.nativeSymbol || 'ETH';
      var nativeFiat = fiatOf(context, symbol, value, 18);
      return {
        index: index,
        symbol: symbol,
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
      var known = ns.registry.token(leg.to);
      var decimals = known ? known.decimals : 18;
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
    if (!gas && !leg) return { gas: null, leg: null, unknown: true, currency: context.currency || '$' };
    return {
      gas: gas,
      leg: leg,
      currency: context.currency || '$',
      // A paymaster changes who pays; say so rather than showing a number that
      // will never leave this account.
      sponsored: !!(operation.paymasterAndData && operation.paymasterAndData !== '0x'),
    };
  }

  ns.fee = { derive: derive, inBandLeg: inBandLeg, gasCeiling: gasCeiling, formatUnits: formatUnits };
})(window.VelaCS);
