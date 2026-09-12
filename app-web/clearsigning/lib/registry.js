// Everything the renderer knows about the world, in one readable file:
// tokens, contacts, your own accounts, contracts, and the ERC-7730-shaped
// descriptors. In the shipped app most of this arrives from the wallet and the
// ethereum-data descriptor server; here it is local so the gallery is offline.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var TOKENS = {
    '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48': { symbol: 'USDC', decimals: 6, name: 'USD Coin', tone: '#2775ca' },
    '0xdac17f958d2ee523a2206206994597c13d831ec7': { symbol: 'USDT', decimals: 6, name: 'Tether USD', tone: '#26a17b' },
    '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2': { symbol: 'WETH', decimals: 18, name: 'Wrapped Ether', tone: '#8a93a5' },
    '0x6b175474e89094c44da98b954eedeac495271d0f': { symbol: 'DAI', decimals: 18, name: 'Dai Stablecoin', tone: '#f5ac37' },
    '0x83f20f44975d03b1b09e64809b757c47f942beea': { symbol: 'sDAI', decimals: 18, name: 'Savings DAI', tone: '#3fae74' },
    '0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d': { symbol: 'BAYC', decimals: 0, name: 'Bored Ape Yacht Club', tone: '#c8963e', nft: true },
    '0x4d224452801aced8b2f0aebe155379bb5d594381': { symbol: 'APE', decimals: 18, name: 'ApeCoin', tone: '#0054f9' },
  };

  var CONTRACTS = {
    '0x1111111254eeb25477b68fb85ed929f73a960582': { name: '1inch Router', owner: '1inch Network', verified: true, descriptor: true },
    '0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45': { name: 'Uniswap V3 Router', owner: 'Uniswap Labs', verified: true, descriptor: true },
    '0x7a250d5630b4cf539739df2c5dacb4c659f2488d': { name: 'Uniswap V2 Router', owner: 'Uniswap Labs', verified: true, descriptor: true },
    '0x000000000022d473030f116ddee9f6b43ac78ba3': { name: 'Permit2', owner: 'Uniswap Labs', verified: true, descriptor: true },
    '0x83f20f44975d03b1b09e64809b757c47f942beea': { name: 'Savings DAI Vault', owner: 'Sky (MakerDAO)', verified: true, descriptor: true },
    '0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d': { name: 'Bored Ape Yacht Club', owner: 'Yuga Labs', verified: true, descriptor: true },
    '0x5f2b8c3a9d1e4f6a7b8c9d0e1f2a3b4c5d6e7f80': { name: 'StakePool', verified: true, descriptor: false }, // 源码已验证，无描述符
    '0x9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a09': { verified: false, descriptor: false },                   // 什么都没有
    '0x4e59b44847b379578588920ca78fbf26c0b4956c': { name: 'CREATE2 Deployer', verified: true, descriptor: false },
  };

  var CHAINS = {
    1: 'Ethereum', 10: 'OP Mainnet', 56: 'BNB Smart Chain', 100: 'Gnosis',
    137: 'Polygon', 8453: 'Base', 42161: 'Arbitrum One', 43114: 'Avalanche',
    59144: 'Linea', 534352: 'Scroll', 4217: 'Tempo',
  };

  var CONTACTS = {
    '0x88cca0f8b4e1f0dc0e7c4f9a2b3d5e6f7a8b6894': { name: 'Account 1', kind: 'own' },
    '0xaf5e8917831ef08a64e18b2cde9f8f5d32c7b3e1': { name: 'Alice Chen', kind: 'contact' },
    '0x031d7d5723f8a1c0e4b96f2d8a7c5e93b1042fd6': { name: 'Account 2', kind: 'own' },
  };

  // ERC-7730-shaped descriptors, keyed by the function signature. The selector
  // is DERIVED with keccak at load — never taken on faith from the descriptor.
  var DESCRIPTORS = [
    {
      signature: 'transfer(address,uint256)',
      kind: 'transfer',
      fields: [
        { label: 'field.recipient', path: [0], format: 'addressName', role: 'recipient' },
        { label: 'field.amount', path: [1], format: 'tokenAmount', token: 'self', role: 'send-amount' },
      ],
    },
    {
      signature: 'approve(address,uint256)',
      kind: 'approve',
      approval: 'set',
      fields: [
        { label: 'field.allowance', path: [1], format: 'tokenAmount', token: 'self', role: 'allowance' },
        { label: 'field.spender', path: [0], format: 'addressName', role: 'spender' },
      ],
    },
    {
      signature: 'increaseAllowance(address,uint256)',
      kind: 'increase',
      approval: 'increase',
      fields: [
        { label: 'field.allowanceDelta', path: [1], format: 'tokenAmount', token: 'self', role: 'allowance-delta' },
        { label: 'field.spender', path: [0], format: 'addressName', role: 'spender' },
      ],
    },
    {
      signature: 'transferFrom(address,address,uint256)',
      kind: 'transferFrom',
      fields: [
        { label: 'field.from', path: [0], format: 'addressName', role: 'from' },
        { label: 'field.to', path: [1], format: 'addressName', role: 'recipient' },
        { label: 'field.amount', path: [2], format: 'tokenAmount', token: 'self', role: 'send-amount' },
      ],
    },
    {
      signature: 'safeTransferFrom(address,address,uint256)',
      kind: 'nftTransfer',
      fields: [
        { label: 'field.from', path: [0], format: 'addressName', role: 'from' },
        { label: 'field.to', path: [1], format: 'addressName', role: 'recipient' },
        { label: 'field.tokenId', path: [2], format: 'nftId', token: 'self' },
      ],
    },
    {
      signature: 'setApprovalForAll(address,bool)',
      kind: 'approveAll',
      approval: 'all',
      fields: [
        { label: 'field.spender', path: [0], format: 'addressName', role: 'spender' },
        { label: 'field.scope', path: [1], format: 'approvalScope' },
      ],
    },
    {
      signature: 'deposit(uint256,address)',
      kind: 'vaultDeposit',
      fields: [
        { label: 'field.deposit', path: [0], format: 'tokenAmount', token: 'vaultAsset', role: 'send-amount' },
        { label: 'field.sharesTo', path: [1], format: 'addressName', role: 'recipient' },
      ],
    },
    {
      signature: 'redeem(uint256,address,address)',
      kind: 'vaultWithdraw',
      fields: [
        { label: 'field.redeem', path: [0], format: 'tokenAmount', token: 'self', role: 'send-amount' },
        { label: 'field.assetTo', path: [1], format: 'addressName', role: 'recipient' },
        { label: 'field.sharesFrom', path: [2], format: 'addressName', detail: true },
      ],
    },
    {
      signature: 'swap(address,(address,address,address,address,uint256,uint256,uint256),bytes,bytes)',
      kind: 'swap',
      fields: [
        { label: 'field.pay', path: [1, 4], format: 'tokenAmount', tokenPath: [1, 0], role: 'send-amount' },
        { label: 'field.minReceive', path: [1, 5], format: 'tokenAmount', tokenPath: [1, 1], role: 'receive-amount' },
        { label: 'field.recipient', path: [1, 3], format: 'addressName', role: 'recipient' },
      ],
    },
    {
      signature: 'swapExactETHForTokens(uint256,address[],address,uint256)',
      kind: 'swap',
      nativeIn: true,
      fields: [
        { label: 'field.minReceive', path: [0], format: 'tokenAmount', tokenPath: [1, 'last'], role: 'receive-amount' },
        { label: 'field.recipient', path: [2], format: 'addressName', role: 'recipient' },
        { label: 'field.deadline', path: [3], format: 'date' },
      ],
    },
    {
      signature: 'multiSend(bytes)',
      kind: 'multiSend',
      nested: 'multiSend',
      fields: [],
    },
    {
      signature: 'execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)',
      kind: 'safeExec',
      nested: 'safeExec',
      fields: [
        { label: 'field.target', path: [0], format: 'addressName' },
        { label: 'field.nativeAttached', path: [1], format: 'nativeAmount' },
      ],
    },
    {
      signature: 'stake(uint256,uint256,address)',
      // Deliberately NO intent: this one stands for "源码已验证但没有描述符",
      // so the ladder must fall to level 2 and speak in facts, not meaning.
      abiOnly: true,
      fields: [],
    },
  ];

  var bySelector = {};
  var selectorProof = [];
  DESCRIPTORS.forEach(function (d) {
    var selector = ns.keccak.selector(d.signature);
    d.selector = selector;
    bySelector[selector] = d;
    selectorProof.push({ selector: selector, signature: d.signature });
  });

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
    tokens: TOKENS,
    contracts: CONTRACTS,
    contacts: CONTACTS,
    bySelector: bySelector,
    fourbyte: FOURBYTE,
    selectorProof: selectorProof,
    token: function (address) {
      return TOKENS[lower(address)] || null;
    },
    contract: function (address) {
      return CONTRACTS[lower(address)] || null;
    },
    contact: function (address) {
      return CONTACTS[lower(address)] || null;
    },
    descriptorFor: function (selector) {
      return bySelector[selector] || null;
    },
  };
})(window.VelaCS);
