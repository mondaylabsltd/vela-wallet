// The 33 scenarios of the design spec, expressed the only way that matters:
// as the signing intent a caller would actually hand this page.
//
// Every calldata below is ENCODED from its signature at load time — none of it
// is hand-typed hex, so a fixture cannot silently disagree with its own label.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var call = ns.encode.call;

  var A = {
    alice: '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1',
    own2: '0x031d7D5723F8a1c0e4B96f2d8A7c5e93b1042Fd6',
    me: '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894',
    stranger: '0x9A8b7C6d5E4F3a2B1c0D9e8F7a6B5c4D3e2F1a09',
    usdc: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
    usdt: '0xdAC17F958D2ee523a2206206994597C13D831ec7',
    weth: '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
    dai: '0x6B175474E89094C44Da98b954EedeAC495271d0F',
    sdai: '0x83F20F44975D03b1b09e64809B757c47f942BEeA',
    bayc: '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D',
    oneinch: '0x1111111254EEB25477B68fb85Ed929f73A960582',
    univ2: '0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D',
    permit2: '0x000000000022D473030F116dDEE9F6B43aC78BA3',
    stakePool: '0x5f2B8c3a9D1E4f6A7b8C9d0E1f2A3b4C5d6E7F80',
    darkPool: '0x7F3d2A1b0C9e8D7f6A5b4C3d2E1f0A9b8C7d6E5f',
    safe: '0x2f0B23f53734Fa68559E1B1B3B9bC3fD9B1F0E5a',
  };

  var MAX = (1n << 256n) - 1n;
  var NOW = Date.now();
  var SOON = Math.floor(NOW / 1000) + 3600;
  var PAST = Math.floor(NOW / 1000) - 7200;

  var seen = [A.alice, A.own2, A.oneinch, A.usdc, A.usdt, A.weth, A.dai].map(function (a) {
    return a.toLowerCase();
  });

  // `icon` is display-only and never reaches the digest — see PROTOCOL.md §4.
  var UNISWAP = { name: 'Uniswap', tone: '#ff007a', icon: 'https://assets.getvela.app/dapps/uniswap.png' };
  var ONEINCH = { name: '1inch', tone: '#d82122' };
  var OPENSEA = { name: 'OpenSea', tone: '#2081e2' };
  var SKY = { name: 'Sky', tone: '#3fae74' };

  function base(extra) {
    var ctx = {
      seenAddresses: seen, now: NOW, account: A.me, chainId: 1,
      // Where token and network logos come from. Display only.
      dataUrl: 'https://ethereum-data.getvela.app',
      // Rates travel with the intent; the sheet multiplies locally and
      // prints the rate it used.
      rates: { ETH: 3400, USDC: 1, USDT: 1, DAI: 1, WETH: 3400 },
      currency: '$',
    };
    Object.keys(extra || {}).forEach(function (k) { ctx[k] = extra[k]; });
    return ctx;
  }

  function tx(to, data, value, origin) {
    return {
      method: 'eth_sendTransaction',
      origin: origin,
      params: [{ to: to, data: data, value: value || '0x0' }],
    };
  }

  function typed(origin, payload) {
    return { method: 'eth_signTypedData_v4', origin: origin, params: [A.me, JSON.stringify(payload)] };
  }

  function hexOf(text) {
    return '0x' + Array.from(new TextEncoder().encode(text))
      .map(function (b) { return b.toString(16).padStart(2, '0'); }).join('');
  }

  var SIWE_OK = 'app.uniswap.org wants you to sign in with your Ethereum account:\n' +
    A.me + '\n\n' +
    'Sign in to Uniswap.\n\n' +
    'URI: https://app.uniswap.org\nVersion: 1\nChain ID: 1\nNonce: 8a3b9f2c\n' +
    'Issued At: 2026-09-07T01:00:00Z';

  var SIWE_PHISH = 'app.uniswap.org wants you to sign in with your Ethereum account:\n' +
    A.me + '\n\n' +
    'Sign in to claim your reward.\n\n' +
    'URI: https://uniswap-airdrop.xyz\nVersion: 1\nChain ID: 1\nNonce: 00ff11ee\n' +
    'Issued At: 2026-09-07T01:00:00Z';

  var ONEINCH_SWAP = call(
    'swap(address,(address,address,address,address,uint256,uint256,uint256),bytes,bytes)',
    [
      A.oneinch,
      [A.usdc, A.weth, A.oneinch, A.me, 100000000n, 30400000000000000n, 0n],
      '0x',
      '0x',
    ],
  );

  var SCENARIOS = [
    {
      code: 'CS1', title: { zh: 'ERC-20 转账', en: 'ERC-20 transfer' },
      intent: tx(A.usdc, call('transfer(address,uint256)', [A.alice, 1000000000n]), '0x0', 'https://app.uniswap.org'),
      ctx: base({ dapp: UNISWAP }),
    },
    {
      code: 'CS2', title: { zh: 'ETH 大额转账', en: 'Large ETH transfer' },
      intent: tx(A.stranger, '0x', '0x22b1c8c1227a0000', 'https://app.uniswap.org'),
      ctx: base({ dapp: UNISWAP }),
    },
    {
      code: 'CS3', title: { zh: '转给另一个地址', en: 'Sending to another address' },
      intent: tx(A.own2, '0x', '0x58d15e17628000', 'https://app.uniswap.org'),
      ctx: base({ dapp: UNISWAP }),
    },
    {
      code: 'CS4', title: { zh: 'ERC-20 代理转账 transferFrom', en: 'ERC-20 transferFrom' },
      intent: tx(A.usdt, call('transferFrom(address,address,uint256)', [A.me, A.alice, 250000000n]), '0x0', 'https://app.1inch.io'),
      ctx: base({ dapp: ONEINCH }),
    },
    {
      code: 'CS5', title: { zh: '授权 · 无限额（已拦截）', en: 'Approval · unlimited (refused)' },
      intent: tx(A.usdc, call('approve(address,uint256)', [A.oneinch, MAX]), '0x0', 'https://app.1inch.io'),
      ctx: base({ dapp: ONEINCH }),
    },
    {
      code: 'CS6', title: { zh: '授权 · 已设上限（按余额）', en: 'Approval · capped' },
      intent: tx(A.usdc, call('approve(address,uint256)', [A.oneinch, 500000000n]), '0x0', 'https://app.1inch.io'),
      ctx: base({ dapp: ONEINCH }),
    },
    {
      code: 'CS7', title: { zh: 'increaseAllowance · 增量语义', en: 'increaseAllowance · additive semantics' },
      intent: tx(A.usdc, call('increaseAllowance(address,uint256)', [A.oneinch, 100000000n]), '0x0', 'https://app.1inch.io'),
      ctx: base({ dapp: ONEINCH, currentAllowance: 50000000n }),
    },
    {
      code: 'CS8', title: { zh: '撤销授权', en: 'Revoke approval' },
      intent: tx(A.usdc, call('approve(address,uint256)', [A.oneinch, 0n]), '0x0', 'https://app.1inch.io'),
      ctx: base({ dapp: ONEINCH }),
    },
    {
      code: 'CS9', title: { zh: 'NFT 转移', en: 'NFT transfer' },
      intent: tx(A.bayc, call('safeTransferFrom(address,address,uint256)', [A.me, A.alice, 3412n]), '0x0', 'https://opensea.io'),
      ctx: base({ dapp: OPENSEA }),
    },
    {
      code: 'CS10', title: { zh: 'NFT 全部授权 setApprovalForAll', en: 'setApprovalForAll · whole collection' },
      intent: tx(A.bayc, call('setApprovalForAll(address,bool)', [A.stranger, true]), '0x0', 'https://opensea.io'),
      ctx: base({ dapp: OPENSEA }),
    },
    {
      code: 'CS11', title: { zh: '兑换 · 1inch 描述符', en: 'Swap · 1inch descriptor' },
      intent: tx(A.oneinch, ONEINCH_SWAP, '0x0', 'https://app.1inch.io'),
      ctx: base({ dapp: ONEINCH }),
    },
    {
      code: 'CS12', title: { zh: '原生币兑换 ETH → 代币', en: 'Native swap ETH → token' },
      intent: tx(A.univ2, call('swapExactETHForTokens(uint256,address[],address,uint256)',
        [3380000000n, [A.weth, A.usdc], A.me, BigInt(SOON)]), '0xde0b6b3a7640000', 'https://app.uniswap.org'),
      ctx: base({ dapp: UNISWAP }),
    },
    {
      code: 'CS13', title: { zh: '兑换 · 截止时间已过', en: 'Swap · deadline already passed' },
      intent: tx(A.univ2, call('swapExactETHForTokens(uint256,address[],address,uint256)',
        [3380000000n, [A.weth, A.usdc], A.me, BigInt(PAST)]), '0xde0b6b3a7640000', 'https://app.uniswap.org'),
      ctx: base({ dapp: UNISWAP }),
    },
    {
      code: 'CS14', title: { zh: '金库存入 ERC-4626', en: 'ERC-4626 vault deposit' },
      intent: tx(A.sdai, call('deposit(uint256,address)', [1000000000000000000000n, A.me]), '0x0', 'https://app.sky.money'),
      ctx: base({ dapp: SKY, vaultAsset: A.dai }),
    },
    {
      code: 'CS15', title: { zh: '金库取出', en: 'Vault withdrawal' },
      intent: tx(A.sdai, call('redeem(uint256,address,address)', [500000000000000000000n, A.me, A.me]), '0x0', 'https://app.sky.money'),
      ctx: base({ dapp: SKY }),
    },
    {
      code: 'CS16', title: { zh: 'Permit2 授权签名 · 无限额', en: 'Permit2 approval · unlimited' },
      intent: typed('https://app.uniswap.org', {
        primaryType: 'PermitSingle',
        domain: { name: 'Permit2', chainId: 1, verifyingContract: A.permit2 },
        types: { PermitSingle: [{ name: 'details', type: 'PermitDetails' }, { name: 'spender', type: 'address' }] },
        message: {
          details: { token: A.usdc, amount: (2n ** 160n - 1n).toString(), expiration: SOON, nonce: 0 },
          spender: A.oneinch,
          sigDeadline: SOON,
        },
      }),
      ctx: base({ dapp: UNISWAP }),
    },
    {
      code: 'CS17', title: { zh: 'EIP-712 Permit · 限额 + 期限', en: 'EIP-712 Permit · cap + deadline' },
      intent: typed('https://app.uniswap.org', {
        primaryType: 'Permit',
        domain: { name: 'USD Coin', version: '2', chainId: 1, verifyingContract: A.usdc },
        types: { Permit: [{ name: 'owner', type: 'address' }, { name: 'spender', type: 'address' }] },
        message: { owner: A.me, spender: A.oneinch, value: '500000000', nonce: 3, deadline: SOON },
      }),
      ctx: base({ dapp: UNISWAP }),
    },
    {
      code: 'CS18', title: { zh: 'EIP-712 未知结构化数据', en: 'EIP-712 unknown structured data' },
      intent: typed('https://mint.example.xyz', {
        primaryType: 'BidOrder',
        domain: { name: 'ExampleMarket', chainId: 1, verifyingContract: A.darkPool },
        types: { BidOrder: [{ name: 'maker', type: 'address' }] },
        message: { maker: A.me, collection: A.bayc, strategy: '0x0000000000000000000000000000000000000000', price: '4200000000000000000', endTime: SOON },
      }),
      ctx: base({}),
    },
    {
      code: 'CS19', title: { zh: 'SIWE 登录 · 域名一致（补：设计稿未导出）', en: 'SIWE sign-in · domain matches (added)' },
      intent: { method: 'personal_sign', origin: 'https://app.uniswap.org', params: [hexOf(SIWE_OK), A.me] },
      ctx: base({ dapp: UNISWAP }),
    },
    {
      code: 'CS20', title: { zh: 'SIWE 登录 · 域名不匹配（钓鱼）', en: 'SIWE sign-in · domain mismatch (phishing)' },
      intent: { method: 'personal_sign', origin: 'https://uniswap-airdrop.xyz', params: [hexOf(SIWE_PHISH), A.me] },
      ctx: base({}),
    },
    {
      code: 'CS21', title: { zh: '十六进制消息签名', en: 'Hex message signature' },
      intent: {
        method: 'personal_sign', origin: 'https://dapp.example.com',
        params: ['0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658', A.me],
      },
      ctx: base({}),
    },
    {
      code: 'CS22', title: { zh: 'eth_sign · 原始哈希（强警告）', en: 'eth_sign · raw hash (hard warning)' },
      intent: {
        method: 'eth_sign', origin: 'https://dapp.example.com',
        params: [A.me, '0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658'],
      },
      ctx: base({}),
    },
    {
      code: 'CS23', title: { zh: '盲签交易 · 模拟兜底', en: 'Blind transaction · simulation fallback' },
      intent: tx(A.darkPool, call('executeRoute(bytes32,uint256)', ['0x' + 'ab'.repeat(32), 100000000n]), '0x0', 'https://router.example.com'),
      ctx: base({
        simulation: {
          summary: '-100 USDC · +0.0304 WETH',
          rows: [{ symbol: 'USDC', delta: '-100' }, { symbol: 'WETH', delta: '+0.0304' }],
          note: '模拟通过 — 没有其他资产变动',
        },
      }),
    },
    {
      code: 'CS24', title: { zh: '盗刷 · 模拟揭穿未声明流出', en: 'Drainer · simulation exposes undeclared outflow' },
      intent: tx(A.darkPool, call('claimAirdrop(address,uint256)', [A.me, 1n]), '0x0', 'https://free-airdrop.example'),
      ctx: base({
        simulation: {
          undeclared: true,
          summary: '-4,820.55 USDC · -1.2 WETH',
          rows: [{ symbol: 'USDC', delta: '-4,820.55' }, { symbol: 'WETH', delta: '-1.2' }],
          note: '站点声称这是「领取空投」，模拟显示它把两种资产转走了',
        },
      }),
    },
    {
      code: 'CS25', title: { zh: '部署合约', en: 'Contract deployment' },
      intent: {
        method: 'eth_sendTransaction', origin: 'https://remix.ethereum.org',
        params: [{ to: '', data: '0x60806040523480156100' + 'ab'.repeat(120), value: '0x0' }],
      },
      ctx: base({}),
    },
    {
      code: 'CS26', title: { zh: 'EIP-5792 批量 · 逐笔明细', en: 'EIP-5792 batch · leg by leg' },
      intent: {
        method: 'wallet_sendCalls', origin: 'https://app.1inch.io',
        params: [{
          version: '1.0', chainId: '0x1', from: A.me,
          calls: [
            { to: A.usdc, data: call('approve(address,uint256)', [A.oneinch, 100000000n]), value: '0x0' },
            { to: A.oneinch, data: ONEINCH_SWAP, value: '0x0' },
          ],
        }],
      },
      ctx: base({
        dapp: ONEINCH,
        simulation: {
          rows: [{ symbol: 'USDC', delta: '-100' }, { symbol: 'WETH', delta: '+0.0304' }],
          note: '模拟通过 — 没有其他资产变动',
        },
      }),
    },
    {
      code: 'CS27', title: { zh: 'Safe 执行 · 嵌套 calldata', en: 'Safe execution · nested calldata' },
      intent: tx(A.safe, call(
        'execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)',
        [A.usdc, 0n, call('transfer(address,uint256)', [A.stranger, 25000000000n]), 0, 0n, 0n, 0n,
          '0x0000000000000000000000000000000000000000', '0x0000000000000000000000000000000000000000', '0x'],
      ), '0x0', 'https://app.safe.global'),
      ctx: base({ dapp: { name: 'Safe', tone: '#12ff80' } }),
    },
    {
      code: 'CS28', title: { zh: '转入代币自身合约（烧毁拦截）', en: 'Transfer into the token contract (burn, refused)' },
      intent: tx(A.usdc, call('transfer(address,uint256)', [A.usdc, 500000000n]), '0x0', 'https://dapp.example.com'),
      ctx: base({}),
    },
    {
      code: 'CS29', title: { zh: '技术细节 · 展开全景', en: 'Technical details · fully expanded' },
      intent: tx(A.usdc, call('transfer(address,uint256)', [A.alice, 1000000000n]), '0x0', 'https://app.uniswap.org'),
      ctx: base({
        dapp: UNISWAP,
        simulation: { summary: '-1,000 USDC · 无其他变动' },
      }),
      options: { techOpen: true },
    },
    {
      code: 'CS30', title: { zh: '陌生调用 · 4byte 兜底', en: 'Unfamiliar call · 4byte fallback' },
      intent: tx(A.darkPool, call('mint(address,uint256)', [A.me, 5n]), '0x0', 'https://mint.example.xyz'),
      ctx: base({}),
    },
    {
      code: 'CS31', title: { zh: '陌生调用 · 验证 ABI 全解码', en: 'Unfamiliar call · verified ABI, full decode' },
      intent: tx(A.stakePool, call('stake(uint256,uint256,address)', [1000000000000000000000n, 2592000n, A.me]), '0x0', 'https://stake.example.com'),
      ctx: base({
        simulation: { summary: '-1,000 DAI · 无其他变动' },
      }),
    },
    {
      code: 'CS32', title: { zh: '最深降级 · 无法解码且无法模拟', en: 'Deepest fallback · neither decodable nor simulatable' },
      intent: tx(A.darkPool, call('executeRoute(bytes32,uint256)', ['0x' + 'cd'.repeat(32), 1n]), '0x0', 'https://router.example.com'),
      ctx: base({}),
    },
    {
      code: 'CS33', title: { zh: '网络费 · 只读，不可切换', en: 'Network fee · read-only, not switchable' },
      intent: tx(A.usdc, call('transfer(address,uint256)', [A.alice, 1000000000n]), '0x0', 'https://app.uniswap.org'),
      ctx: base({
        dapp: UNISWAP,
      }),
    },
  ];

  ns.scenarios = SCENARIOS;
  ns.addresses = A;
})(window.VelaCS);
