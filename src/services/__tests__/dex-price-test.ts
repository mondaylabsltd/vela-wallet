/**
 * Automated DEX + Chainlink price query test.
 *
 * Tests that native token prices can be fetched on all default chains
 * via both DEX swap quotes and on-chain Chainlink feeds.
 *
 * Run: npx tsx src/services/__tests__/dex-price-test.ts
 */

import {
  MULTICALL3,
  encAggregate3, decAggregate3,
  encQuoteV3, encGetAmountsOut,
  encLatestRound,
  decU256, decAmountsOut, decChainlinkUsd,
  type Call3, type McResult,
} from '../abi';

// ---------------------------------------------------------------------------
// Config: what to test on each chain
// ---------------------------------------------------------------------------

interface ChainTest {
  chainId: number;
  name: string;
  rpc: string;
  nativeSymbol: string;
  nativeDecimals: number;
  wrappedNative: string;
  /** Chainlink native/USD feed on this chain */
  chainlinkFeed: string;
  /** DEX protocol */
  dex: {
    protocol: 'uniswap-v3' | 'solidly';
    /** QuoterV2 for uniswap-v3, Router for solidly */
    contract: string;
    /** Fee tiers to try (uniswap-v3 only) */
    feeTiers?: number[];
  };
  /** Stablecoin to quote against */
  quoteToken: { symbol: string; address: string; decimals: number };
}

const CHAINS: ChainTest[] = [
  {
    chainId: 1, name: 'Ethereum', rpc: 'https://ethereum-rpc.publicnode.com',
    nativeSymbol: 'ETH', nativeDecimals: 18,
    wrappedNative: '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
    chainlinkFeed: '0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419',
    dex: { protocol: 'uniswap-v3', contract: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e', feeTiers: [500, 3000] },
    quoteToken: { symbol: 'USDC', address: '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', decimals: 6 },
  },
  {
    chainId: 56, name: 'BSC', rpc: 'https://bsc-dataseed1.bnbchain.org',
    nativeSymbol: 'BNB', nativeDecimals: 18,
    wrappedNative: '0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c',
    chainlinkFeed: '0x0567F2323251f0Aab15c8dFb1967E4e8A7D42aeE',
    dex: { protocol: 'uniswap-v3', contract: '0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997', feeTiers: [500, 2500, 3000] },
    quoteToken: { symbol: 'USDT', address: '0x55d398326f99059fF775485246999027B3197955', decimals: 18 },
  },
  {
    chainId: 137, name: 'Polygon', rpc: 'https://polygon-bor-rpc.publicnode.com',
    nativeSymbol: 'POL', nativeDecimals: 18,
    wrappedNative: '0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270',
    chainlinkFeed: '0xAB594600376Ec9fD91F8e8dC495468db32390157',
    dex: { protocol: 'uniswap-v3', contract: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e', feeTiers: [500, 3000] },
    quoteToken: { symbol: 'USDC', address: '0x3c499c542cef5e3811e1192ce70d8cc03d5c3359', decimals: 6 },
  },
  {
    chainId: 42161, name: 'Arbitrum', rpc: 'https://arb1.arbitrum.io/rpc',
    nativeSymbol: 'ETH', nativeDecimals: 18,
    wrappedNative: '0x82aF49447D8a07e3bd95BD0d56f35241523fBab1',
    chainlinkFeed: '0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612',
    dex: { protocol: 'uniswap-v3', contract: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e', feeTiers: [500, 3000] },
    quoteToken: { symbol: 'USDC', address: '0xaf88d065e77c8cc2239327c5edb3a432268e5831', decimals: 6 },
  },
  {
    chainId: 10, name: 'Optimism', rpc: 'https://optimism-rpc.publicnode.com',
    nativeSymbol: 'ETH', nativeDecimals: 18,
    wrappedNative: '0x4200000000000000000000000000000000000006',
    chainlinkFeed: '0x13e3Ee699D1909E989722E753853AE30b17e08c5',
    dex: { protocol: 'uniswap-v3', contract: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e', feeTiers: [500, 3000] },
    quoteToken: { symbol: 'USDC', address: '0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85', decimals: 6 },
  },
  {
    chainId: 8453, name: 'Base', rpc: 'https://mainnet.base.org',
    nativeSymbol: 'ETH', nativeDecimals: 18,
    wrappedNative: '0x4200000000000000000000000000000000000006',
    chainlinkFeed: '0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70',
    dex: { protocol: 'solidly', contract: '0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43' },
    quoteToken: { symbol: 'USDC', address: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913', decimals: 6 },
  },
  {
    chainId: 43114, name: 'Avalanche', rpc: 'https://api.avax.network/ext/bc/C/rpc',
    nativeSymbol: 'AVAX', nativeDecimals: 18,
    wrappedNative: '0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7',
    chainlinkFeed: '0x0A77230d17318075983913bC2145DB16C7366156',
    dex: { protocol: 'uniswap-v3', contract: '0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F', feeTiers: [500, 3000] },
    quoteToken: { symbol: 'USDC', address: '0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E', decimals: 6 },
  },
  {
    chainId: 100, name: 'Gnosis', rpc: 'https://rpc.gnosischain.com',
    nativeSymbol: 'XDAI', nativeDecimals: 18,
    wrappedNative: '0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d',
    chainlinkFeed: '0x678df3415fc31947dA4324eC63212874be5a82f8',
    dex: { protocol: 'uniswap-v3', contract: '0xb1E835Dc2785b52265711e17fCCb0fd018226a6e', feeTiers: [500, 3000] },
    quoteToken: { symbol: 'USDC', address: '0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83', decimals: 6 },
  },
];

// ---------------------------------------------------------------------------
// RPC helper
// ---------------------------------------------------------------------------

async function ethCall(rpc: string, to: string, data: string): Promise<string> {
  const res = await fetch(rpc, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_call', params: [{ to, data }, 'latest'] }),
  });
  const json: any = await res.json();
  if (json.error) throw new Error(json.error.message);
  if (!json.result || json.result === '0x') throw new Error('Empty result');
  return json.result;
}

// ---------------------------------------------------------------------------
// Test runner
// ---------------------------------------------------------------------------

async function testChain(chain: ChainTest): Promise<{ dex: string; chainlink: string }> {
  const calls: Call3[] = [];
  const amountIn = 10n ** BigInt(chain.nativeDecimals); // 1 native token

  // --- DEX price calls ---
  const dexCallIdxs: number[] = [];
  if (chain.dex.protocol === 'uniswap-v3') {
    for (const fee of chain.dex.feeTiers ?? [500, 3000]) {
      dexCallIdxs.push(calls.length);
      calls.push({ target: chain.dex.contract, allowFailure: true, callData: encQuoteV3(chain.wrappedNative, chain.quoteToken.address, amountIn, fee) });
    }
  } else if (chain.dex.protocol === 'solidly') {
    for (const stable of [false, true]) {
      dexCallIdxs.push(calls.length);
      calls.push({ target: chain.dex.contract, allowFailure: true, callData: encGetAmountsOut(amountIn, chain.wrappedNative, chain.quoteToken.address, stable) });
    }
  }

  // --- Chainlink call ---
  const clIdx = calls.length;
  calls.push({ target: chain.chainlinkFeed, allowFailure: true, callData: encLatestRound() });

  // --- Execute ---
  const encoded = encAggregate3(calls);
  const raw = await ethCall(chain.rpc, MULTICALL3, encoded);
  const results = decAggregate3(raw);

  // --- Decode DEX ---
  let dexResult = 'FAIL';
  const decoder = chain.dex.protocol === 'solidly' ? decAmountsOut : decU256;
  for (let i = 0; i < dexCallIdxs.length; i++) {
    const r = results[dexCallIdxs[i]];
    const label = chain.dex.protocol === 'uniswap-v3'
      ? `fee=${chain.dex.feeTiers?.[i] ?? '?'}`
      : `stable=${i === 1}`;
    if (r?.success && r.data.length >= 66) {
      const amountOut = decoder(r.data);
      if (amountOut > 0n) {
        const price = Number(amountOut) / 10 ** chain.quoteToken.decimals;
        dexResult = `$${price.toFixed(4)} (${label})`;
        break;
      } else {
        // success but 0 amount
      }
    }
  }

  // --- Decode Chainlink ---
  let clResult = 'FAIL';
  const clr = results[clIdx];
  if (clr?.success && clr.data.length >= 66) {
    const usd = decChainlinkUsd(clr.data);
    if (Number.isFinite(usd) && usd > 0) {
      clResult = `$${usd.toFixed(4)}`;
    }
  }

  return { dex: dexResult, chainlink: clResult };
}

async function main() {
  console.log('=== DEX & Chainlink Price Test ===\n');

  const results: { name: string; symbol: string; dex: string; chainlink: string; ok: boolean }[] = [];

  await Promise.all(
    CHAINS.map(async (chain) => {
      try {
        const r = await testChain(chain);
        const ok = !r.dex.startsWith('FAIL') || !r.chainlink.startsWith('FAIL');
        results.push({ name: chain.name, symbol: chain.nativeSymbol, ...r, ok });
      } catch (err) {
        results.push({
          name: chain.name,
          symbol: chain.nativeSymbol,
          dex: 'ERROR',
          chainlink: 'ERROR',
          ok: false,
        });
        console.error(`  ${chain.name}: ${err instanceof Error ? err.message : String(err)}`);
      }
    }),
  );

  // Sort by chainId for stable output
  results.sort((a, b) => {
    const ai = CHAINS.findIndex(c => c.name === a.name);
    const bi = CHAINS.findIndex(c => c.name === b.name);
    return ai - bi;
  });

  // Print results table
  console.log('Chain'.padEnd(12) + 'Symbol'.padEnd(8) + 'DEX'.padEnd(28) + 'Chainlink'.padEnd(16) + 'Status');
  console.log('-'.repeat(72));
  let allOk = true;
  for (const r of results) {
    const status = r.ok ? 'OK' : 'FAIL';
    if (!r.ok) allOk = false;
    console.log(
      r.name.padEnd(12) +
      r.symbol.padEnd(8) +
      r.dex.padEnd(28) +
      r.chainlink.padEnd(16) +
      status,
    );
  }

  console.log('-'.repeat(72));
  console.log(allOk ? '\nAll chains OK!' : '\nSome chains FAILED!');
  process.exit(allOk ? 0 : 1);
}

main();
