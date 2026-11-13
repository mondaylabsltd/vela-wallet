// Everything the renderer knows about the world, in one readable file:
// tokens, networks, contracts and the ERC-7730-shaped descriptors — each per
// chain, and nothing but real ones. The gallery's made-up examples (a
// "source verified" contract at an address nothing lives at) live in demo/fixtures.js,
// which only the gallery loads, so the published page never names them
// (cs-20). The page reaches no network (076): all of it is local.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  // Tokens, PER CHAIN (spec 102): the same address can be a different
  // contract on another network, so a token is known only on the chain the
  // signature is for. Every row of vela-core's token registry
  // (lib/catalog.js, generated), plus three mainnet entries only this page
  // carries — a vault, a collection and a coin its examples use.
  var OWN_TOKENS = {
    1: {
      '0x83f20f44975d03b1b09e64809b757c47f942beea': { symbol: 'sDAI', decimals: 18, name: 'Savings DAI', tone: '#3fae74' },
      '0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d': { symbol: 'BAYC', decimals: 0, name: 'Bored Ape Yacht Club', tone: '#c8963e', nft: true },
      '0x4d224452801aced8b2f0aebe155379bb5d594381': { symbol: 'APE', decimals: 18, name: 'ApeCoin', tone: '#0054f9' },
    },
  };

  // A token chip's colour, by symbol — decoration, like the logo it stands in
  // for. Anything not listed is drawn in the neutral grey.
  var TONES = {
    USDC: '#2775ca', 'USDC.e': '#2775ca', USDbC: '#2775ca', axlUSDC: '#2775ca',
    USDT: '#26a17b', USDT0: '#26a17b', 'USDT.e': '#26a17b', 'USD₮0': '#26a17b',
    DAI: '#f5ac37', 'DAI.e': '#f5ac37', EURC: '#2775ca', PYUSD: '#0074de', PYUSD0: '#0074de',
  };

  var TOKENS = {};
  ((ns.catalog && ns.catalog.tokens) || []).forEach(function (row) {
    var chain = TOKENS[row[0]] || (TOKENS[row[0]] = {});
    chain[row[1]] = { symbol: row[2], decimals: row[3], tone: TONES[row[2]] || '#8a93a5' };
  });
  Object.keys(OWN_TOKENS).forEach(function (chainId) {
    var chain = TOKENS[chainId] || (TOKENS[chainId] = {});
    Object.keys(OWN_TOKENS[chainId]).forEach(function (address) {
      chain[address] = OWN_TOKENS[chainId][address];
    });
  });

  // Contracts this page names, PER CHAIN (cs-13), like its tokens: an address
  // is a different contract — or nothing at all — on another network. The
  // Uniswap router's address on Gnosis drew "Uniswap V3 Router" over a call
  // to whatever sits there. `EVERY_CHAIN` holds only contracts whose address
  // fixes their code wherever there is code at it: deployed through a
  // deterministic deployer from fixed bytes (Permit2, Safe's MultiSend), or
  // the keyless deployer itself.
  var EVERY_CHAIN = '*';
  var ONEINCH_V5 = '0x1111111254eeb25477b68fb85ed929f73a960582';
  var UNISWAP_V3_ROUTER2 = '0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45';
  var UNISWAP_V2_ROUTER = '0x7a250d5630b4cf539739df2c5dacb4c659f2488d';
  var SDAI = '0x83f20f44975d03b1b09e64809b757c47f942beea';
  var DAI = '0x6b175474e89094c44da98b954eedeac495271d0f';
  var PERMIT2 = '0x000000000022d473030f116ddee9f6b43ac78ba3';
  var MULTI_SEND = [
    '0x38869bf66a61cf6bdb996a6ae40d5853fd43b526', // MultiSend 1.4.1
    '0x9641d764fc13c8b624c04430c7356c1c7c8102e2', // MultiSendCallOnly 1.4.1
    '0xa238cbeb142c10ef7ad8442c6d1f9e89e07e7761', // MultiSend 1.3.0
    '0x40a2accbd92bca938b02010e17a5b8929b49130d', // MultiSendCallOnly 1.3.0
  ];
  var ONEINCH_CHAINS = [1, 10, 56, 100, 137, 8453, 42161, 43114];
  var UNISWAP_V3_CHAINS = [1, 10, 137, 42161];

  var CONTRACTS = {};
  function contractOn(chainId, address, entry) {
    (CONTRACTS[chainId] || (CONTRACTS[chainId] = {}))[address] = entry;
  }
  contractOn(EVERY_CHAIN, PERMIT2, { name: 'Permit2', owner: 'Uniswap Labs', verified: true, descriptor: true });
  contractOn(EVERY_CHAIN, '0x4e59b44847b379578588920ca78fbf26c0b4956c', { name: 'CREATE2 Deployer', verified: true, descriptor: false });
  MULTI_SEND.forEach(function (address) {
    contractOn(EVERY_CHAIN, address, { name: 'Safe MultiSend', owner: 'Safe', verified: true, descriptor: true });
  });
  ONEINCH_CHAINS.forEach(function (chainId) {
    contractOn(chainId, ONEINCH_V5, { name: '1inch Router', owner: '1inch Network', verified: true, descriptor: true });
  });
  UNISWAP_V3_CHAINS.forEach(function (chainId) {
    contractOn(chainId, UNISWAP_V3_ROUTER2, { name: 'Uniswap V3 Router', owner: 'Uniswap Labs', verified: true, descriptor: false });
  });
  contractOn(1, UNISWAP_V2_ROUTER, { name: 'Uniswap V2 Router', owner: 'Uniswap Labs', verified: true, descriptor: true });
  contractOn(1, SDAI, { name: 'Savings DAI Vault', owner: 'Sky (MakerDAO)', verified: true, descriptor: true });
  contractOn(1, '0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d', { name: 'Bored Ape Yacht Club', owner: 'Yuga Labs', verified: true, descriptor: true });

  /** `{chainId: {address: binding}}` for every chain in `chains`. */
  function on(chains, address, binding) {
    var out = {};
    chains.forEach(function (chainId) {
      out[chainId] = {};
      out[chainId][address] = binding || {};
    });
    return out;
  }

  // Networks: the ones Vela ships, named as the apps name them
  // (lib/catalog.js), and two more this page has always known.
  var CHAINS = { 59144: 'Linea', 534352: 'Scroll' };
  var NATIVE = { 59144: 'ETH', 534352: 'ETH' };
  ((ns.catalog && ns.catalog.chains) || []).forEach(function (row) {
    CHAINS[row[0]] = row[1];
    NATIVE[row[0]] = row[2];
  });
  // Each chain's own coin — except Tempo, which has none (fees are paid in a
  // stablecoin; the apps' "USD" is a display unit, not a coin a call can
  // carry), so it is absent rather than guessed.
  delete NATIVE[4217];

  // ERC-7730-shaped descriptors, keyed by the function signature. The selector
  // is DERIVED with keccak at load — never taken on faith from the descriptor.
  //
  // A function's shape says nothing about the contract that has it (cs-4):
  // any contract can expose `swap(address,(…),bytes,bytes)` and do with it
  // whatever it likes, so a reading keyed by shape alone drew "Swap · You pay
  // 250 USDC and receive…" and a "Confirm swap" button over a call to an
  // attacker's contract. Two kinds, then:
  //
  //  · `standard` — a token standard's own functions (ERC-20, ERC-721). The
  //    standard IS the meaning, so they are read on any contract; whether
  //    the contract is a token this page knows on this chain decides only
  //    the provenance line (read from the bytes of a known token, or "looks
  //    like" for anything else — resolve.js);
  //  · `contracts` — a protocol's descriptor, bound to the contracts (and
  //    chains) it was written for, with what each one needs that the bytes
  //    do not carry (a vault's asset). Anywhere else the same shape is only
  //    a resemblance, drawn as a contract call with `shape` said in words.
  var DESCRIPTORS = [
    {
      signature: 'transfer(address,uint256)',
      kind: 'transfer',
      standard: 'erc20',
      fields: [
        { label: 'field.recipient', path: [0], format: 'addressName', role: 'recipient' },
        { label: 'field.amount', path: [1], format: 'tokenAmount', token: 'self', role: 'send-amount' },
      ],
    },
    {
      signature: 'approve(address,uint256)',
      kind: 'approve',
      standard: 'erc20',
      approval: 'set',
      fields: [
        { label: 'field.allowance', path: [1], format: 'tokenAmount', token: 'self', role: 'allowance' },
        { label: 'field.spender', path: [0], format: 'addressName', role: 'spender' },
      ],
    },
    {
      signature: 'increaseAllowance(address,uint256)',
      kind: 'increase',
      standard: 'erc20',
      approval: 'increase',
      fields: [
        { label: 'field.allowanceDelta', path: [1], format: 'tokenAmount', token: 'self', role: 'allowance-delta' },
        { label: 'field.spender', path: [0], format: 'addressName', role: 'spender' },
      ],
    },
    {
      signature: 'transferFrom(address,address,uint256)',
      kind: 'transferFrom',
      standard: 'erc20',
      fields: [
        { label: 'field.from', path: [0], format: 'addressName', role: 'from' },
        { label: 'field.to', path: [1], format: 'addressName', role: 'recipient' },
        { label: 'field.amount', path: [2], format: 'tokenAmount', token: 'self', role: 'send-amount' },
      ],
    },
    {
      signature: 'safeTransferFrom(address,address,uint256)',
      kind: 'nftTransfer',
      standard: 'erc721',
      fields: [
        { label: 'field.from', path: [0], format: 'addressName', role: 'from' },
        { label: 'field.to', path: [1], format: 'addressName', role: 'recipient' },
        { label: 'field.tokenId', path: [2], format: 'nftId', token: 'self' },
      ],
    },
    {
      signature: 'setApprovalForAll(address,bool)',
      kind: 'approveAll',
      standard: 'erc721',
      approval: 'all',
      fields: [
        { label: 'field.spender', path: [0], format: 'addressName', role: 'spender' },
        { label: 'field.scope', path: [1], format: 'approvalScope' },
      ],
    },
    {
      // ERC-4626, as Savings DAI implements it. The asset is the vault's, in
      // the page's own words for the vault it knows — never the app's
      // `context.vaultAsset` (cs-4: a claimed asset drew "You deposit 1,000
      // USDC" into a contract that takes no USDC).
      signature: 'deposit(uint256,address)',
      kind: 'vaultDeposit',
      shape: 'shape.vaultDeposit',
      contracts: on([1], SDAI, { asset: DAI }),
      fields: [
        { label: 'field.deposit', path: [0], format: 'tokenAmount', token: 'vaultAsset', role: 'send-amount' },
        // `returns`: where what the call gives back goes — the account, as a
        // rule, and anywhere else is said (cs-2).
        { label: 'field.sharesTo', path: [1], format: 'addressName', role: 'recipient', returns: true },
      ],
    },
    {
      signature: 'redeem(uint256,address,address)',
      kind: 'vaultWithdraw',
      shape: 'shape.vaultWithdraw',
      contracts: on([1], SDAI),
      fields: [
        { label: 'field.redeem', path: [0], format: 'tokenAmount', token: 'self', role: 'send-amount' },
        { label: 'field.assetTo', path: [1], format: 'addressName', role: 'recipient', returns: true },
        { label: 'field.sharesFrom', path: [2], format: 'addressName', detail: true },
      ],
    },
    {
      // 1inch AggregationRouterV5.
      signature: 'swap(address,(address,address,address,address,uint256,uint256,uint256),bytes,bytes)',
      kind: 'swap',
      shape: 'shape.oneInchSwap',
      contracts: on(ONEINCH_CHAINS, ONEINCH_V5),
      fields: [
        { label: 'field.pay', path: [1, 4], format: 'tokenAmount', tokenPath: [1, 0], role: 'send-amount' },
        { label: 'field.minReceive', path: [1, 5], format: 'tokenAmount', tokenPath: [1, 1], role: 'receive-amount' },
        { label: 'field.recipient', path: [1, 3], format: 'addressName', role: 'recipient', returns: true },
      ],
    },
    {
      signature: 'swapExactETHForTokens(uint256,address[],address,uint256)',
      kind: 'swap',
      shape: 'shape.uniswapV2Swap',
      contracts: on([1], UNISWAP_V2_ROUTER),
      nativeIn: true,
      fields: [
        { label: 'field.minReceive', path: [0], format: 'tokenAmount', tokenPath: [1, 'last'], role: 'receive-amount' },
        { label: 'field.recipient', path: [2], format: 'addressName', role: 'recipient', returns: true },
        { label: 'field.deadline', path: [3], format: 'date' },
      ],
    },
    {
      // Safe's MultiSend. Its legs are read from the bytes wherever it is
      // called, but only MultiSend is known to run them as legs.
      signature: 'multiSend(bytes)',
      kind: 'multiSend',
      shape: 'shape.safeBatch',
      contracts: (function () {
        var out = {};
        out[EVERY_CHAIN] = {};
        MULTI_SEND.forEach(function (address) { out[EVERY_CHAIN][address] = {}; });
        return out;
      })(),
      nested: 'multiSend',
      fields: [],
    },
    {
      // A Safe is any address (a proxy), so this one is bound to none: it is
      // always a resemblance, its inner call read from the bytes.
      signature: 'execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)',
      kind: 'safeExec',
      shape: 'shape.safeExec',
      contracts: {},
      nested: 'safeExec',
      fields: [
        { label: 'field.target', path: [0], format: 'addressName' },
        { label: 'field.nativeAttached', path: [1], format: 'nativeAmount' },
      ],
    },
  ];

  var bySelector = {};
  var selectorProof = [];
  function addDescriptor(d) {
    var selector = ns.keccak.selector(d.signature);
    d.selector = selector;
    bySelector[selector] = d;
    selectorProof.push({ selector: selector, signature: d.signature });
  }
  DESCRIPTORS.forEach(addDescriptor);

  // A 4-byte database entry with no descriptor behind it: we know the NAME of
  // the function and nothing about what it means. Ladder level 3.
  var FOURBYTE = {};
  ['mint(address,uint256)', 'claim()', 'harvest(address)'].forEach(function (sig) {
    FOURBYTE[ns.keccak.selector(sig)] = sig;
  });

  function lower(address) {
    return (address || '').toLowerCase();
  }

  ns.registry = {
    chains: CHAINS,
    chainName: function (chainId) { return CHAINS[chainId] || null; },
    nativeSymbol: function (chainId) { return NATIVE[chainId] || null; },
    tokens: TOKENS,
    contracts: CONTRACTS,
    bySelector: bySelector,
    fourbyte: FOURBYTE,
    selectorProof: selectorProof,
    /** The gallery's own fixtures (demo/fixtures.js) come in through these. */
    addContract: contractOn,
    addDescriptor: addDescriptor,
    /** A token this page knows ON THAT CHAIN, or null. */
    token: function (address, chainId) {
      var chain = TOKENS[Number(chainId)];
      return (chain && chain[lower(address)]) || null;
    },
    /** A contract this page knows ON THAT CHAIN (or on every chain), or null. */
    contract: function (address, chainId) {
      var chain = CONTRACTS[Number(chainId)];
      return (chain && chain[lower(address)]) || CONTRACTS[EVERY_CHAIN][lower(address)] || null;
    },
    descriptorFor: function (selector) {
      return bySelector[selector] || null;
    },
    /**
     * What a protocol descriptor knows about the contract it is called on
     * (`{}` at least), or null when it was not written for that contract on
     * that chain. A `standard` descriptor is no protocol's: always `{}`.
     */
    binding: function (descriptor, address, chainId) {
      if (!descriptor) return null;
      if (descriptor.standard) return {};
      var bound = descriptor.contracts || {};
      var chain = bound[Number(chainId)];
      var every = bound[EVERY_CHAIN];
      return (chain && chain[lower(address)]) || (every && every[lower(address)]) || null;
    },
  };
})(window.VelaCS);
