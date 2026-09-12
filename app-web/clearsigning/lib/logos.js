// Token and network logos, from the same endpoint the wallet uses.
//
// These are DECORATION and nothing else. A logo cannot change what gets signed,
// so fetching one from a server is a different kind of act than trusting a name
// or an amount — but it is still worth being precise about the limits:
//
//   · every logo is loaded into an <img>, never fetched and parsed, so a
//     compromised endpoint can serve a wrong picture and nothing more;
//   · a failed load is silent and falls back to a drawn chip — the sheet must
//     never wait on, or break because of, a picture;
//   · asking for a token's logo tells that server which token you are about to
//     sign for. The wallet already makes that trade; this page matches it, and
//     `context.dataUrl` lets a deployment point somewhere else.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var DEFAULT_BASE = 'https://ethereum-data.getvela.app';

  function base(context) {
    var url = (context && context.dataUrl) || DEFAULT_BASE;
    return String(url).replace(/\/+$/, '');
  }

  /** EIP-55 checksum casing — the endpoint stores assets under it. */
  function checksum(address) {
    var body = String(address).replace(/^0x/i, '').toLowerCase();
    if (!/^[0-9a-f]{40}$/.test(body)) return null;
    var hash = ns.keccak.hex(new TextEncoder().encode(body)).slice(2);
    var out = '';
    for (var i = 0; i < body.length; i++) {
      out += parseInt(hash[i], 16) >= 8 ? body[i].toUpperCase() : body[i];
    }
    return '0x' + out;
  }

  /** Candidate URLs for an ERC-20 logo, most likely first. */
  function token(context, chainId, address) {
    var cs = checksum(address);
    if (!cs || !chainId) return [];
    var prefix = base(context) + '/assets/eip155-' + chainId;
    var urls = [prefix + '/' + cs + '/logo.png'];
    var lower = String(address).toLowerCase();
    if (lower !== cs) urls.push(prefix + '/' + lower + '/logo.png');
    return urls;
  }

  function chain(context, chainId) {
    return chainId ? [base(context) + '/chainlogos/eip155-' + chainId + '.png'] : [];
  }

  ns.logos = {
    DEFAULT_BASE: DEFAULT_BASE,
    base: base,
    checksum: checksum,
    token: token,
    chain: chain,
  };
})(window.VelaCS);
