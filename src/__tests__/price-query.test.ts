/**
 * Automated test: verify DEX and Chainlink price queries work on all chains.
 * Sends real eth_call to public RPCs — no mocking.
 *
 * Run: npx jest src/__tests__/price-query.test.ts --testTimeout=30000
 */

// These tests hit real RPCs which can be slow (especially Gnosis)
jest.setTimeout(30_000);

import {
  MULTICALL3,
  encAggregate3, decAggregate3,
  encQuoteV3, encGetAmountsOut, decAmountsOut,
  encLatestRound, decChainlinkUsd, decU256,
  type Call3,
} from '../services/abi';

// ---------------------------------------------------------------------------
// Test data per chain
// ---------------------------------------------------------------------------

interface ChainTestData {
  chainId: number;
  name: string;
  rpc: string;
  wrappedNative: string;
  nativeDecimals: number;
  nativeSymbol: string;
  // DEX
  dexProtocol: 'uniswap-v3' | 'solidly' | 'none';
  dexQuoter?: string; // QuoterV2 for uniswap-v3, Router for solidly
  quoteToken: string; // USDC or USDT
  quoteDecimals: number;
  feeTiers?: number[];
  // Chainlink
  chainlinkFeed?: string;
}

const CHAINS: ChainTestData[] = [
  {
    chainId: 1, name: 'Ethereum', rpc: 'https://1rpc.io/eth',
    wrappedNative: '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
    nativeDecimals: 18, nativeSymbol: 'ETH',
    dexProtocol: 'uniswap-v3',
    dexQuoter: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e',
    quoteToken: '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', // USDC
    quoteDecimals: 6,
    feeTiers: [500, 3000],
    chainlinkFeed: '0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419',
  },
  {
    chainId: 56, name: 'BNB Chain', rpc: 'https://bsc-dataseed1.bnbchain.org',
    wrappedNative: '0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c',
    nativeDecimals: 18, nativeSymbol: 'BNB',
    dexProtocol: 'uniswap-v3',
    dexQuoter: '0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997', // PancakeSwap V3 QuoterV2
    quoteToken: '0x55d398326f99059fF775485246999027B3197955', // USDT (18 decimals on BSC)
    quoteDecimals: 18,
    feeTiers: [500, 2500, 100],
    chainlinkFeed: '0x0567F2323251f0Aab15c8dFb1967E4e8A7D42aeE',
  },
  {
    chainId: 137, name: 'Polygon', rpc: 'https://1rpc.io/matic',
    wrappedNative: '0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270',
    nativeDecimals: 18, nativeSymbol: 'POL',
    dexProtocol: 'uniswap-v3',
    dexQuoter: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e',
    quoteToken: '0x3c499c542cef5e3811e1192ce70d8cc03d5c3359', // USDC
    quoteDecimals: 6,
    feeTiers: [500, 3000],
    // No working Chainlink feed (MATIC→POL migration)
  },
  {
    chainId: 42161, name: 'Arbitrum', rpc: 'https://1rpc.io/arb',
    wrappedNative: '0x82aF49447D8a07e3bd95BD0d56f35241523fBab1',
    nativeDecimals: 18, nativeSymbol: 'ETH',
    dexProtocol: 'uniswap-v3',
    dexQuoter: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e',
    quoteToken: '0xaf88d065e77c8cc2239327c5edb3a432268e5831', // USDC
    quoteDecimals: 6,
    feeTiers: [500, 3000],
    chainlinkFeed: '0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612',
  },
  {
    chainId: 10, name: 'Optimism', rpc: 'https://1rpc.io/op',
    wrappedNative: '0x4200000000000000000000000000000000000006',
    nativeDecimals: 18, nativeSymbol: 'ETH',
    dexProtocol: 'uniswap-v3',
    dexQuoter: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e',
    quoteToken: '0x0b2c639c533813f4aa9d7837caf62653d097ff85', // USDC
    quoteDecimals: 6,
    feeTiers: [500, 3000],
    chainlinkFeed: '0x13e3Ee699D1909E989722E753853AE30b17e08c5',
  },
  {
    chainId: 8453, name: 'Base', rpc: 'https://1rpc.io/base',
    wrappedNative: '0x4200000000000000000000000000000000000006',
    nativeDecimals: 18, nativeSymbol: 'ETH',
    dexProtocol: 'solidly',
    dexQuoter: '0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43', // Aerodrome Router
    quoteToken: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913', // USDC
    quoteDecimals: 6,
    chainlinkFeed: '0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70',
  },
  {
    chainId: 43114, name: 'Avalanche', rpc: 'https://1rpc.io/avax/c',
    wrappedNative: '0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7',
    nativeDecimals: 18, nativeSymbol: 'AVAX',
    dexProtocol: 'uniswap-v3',
    dexQuoter: '0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F', // Uniswap V3 on Avalanche
    quoteToken: '0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E', // USDC
    quoteDecimals: 6,
    feeTiers: [500, 3000],
    chainlinkFeed: '0x0A77230d17318075983913bC2145DB16C7366156',
  },
  {
    chainId: 100, name: 'Gnosis', rpc: 'https://rpc.gnosischain.com',
    wrappedNative: '0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d',
    nativeDecimals: 18, nativeSymbol: 'xDAI',
    dexProtocol: 'none', // No reliable DEX quoter
    quoteToken: '0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83', // USDC
    quoteDecimals: 6,
    chainlinkFeed: '0x678df3415fc31947dA4324eC63212874be5a82f8',
  },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function ethCall(rpc: string, to: string, data: string): Promise<string> {
  const res = await fetch(rpc, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0', id: 1,
      method: 'eth_call',
      params: [{ to, data }, 'latest'],
    }),
  });
  const json = await res.json();
  if (json.error) throw new Error(json.error.message);
  return json.result;
}

function mc(target: string, callData: string): Call3 {
  return { target, allowFailure: true, callData };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Native token price queries (real RPC)', () => {
  for (const chain of CHAINS) {
    describe(`${chain.name} (${chain.chainId})`, () => {

      if (chain.dexProtocol !== 'none') {
        test(`DEX price query (${chain.dexProtocol})`, async () => {
          const amountIn = 10n ** BigInt(chain.nativeDecimals);
          const calls: Call3[] = [];

          if (chain.dexProtocol === 'uniswap-v3') {
            for (const fee of chain.feeTiers!) {
              calls.push(mc(chain.dexQuoter!, encQuoteV3(chain.wrappedNative, chain.quoteToken, amountIn, fee)));
            }
          } else if (chain.dexProtocol === 'solidly') {
            calls.push(mc(chain.dexQuoter!, encGetAmountsOut(amountIn, chain.wrappedNative, chain.quoteToken, false)));
            calls.push(mc(chain.dexQuoter!, encGetAmountsOut(amountIn, chain.wrappedNative, chain.quoteToken, true)));
          }

          const encoded = encAggregate3(calls);
          const raw = await ethCall(chain.rpc, MULTICALL3, encoded);
          const results = decAggregate3(raw);

          let price: number | null = null;
          for (let i = 0; i < results.length; i++) {
            const r = results[i];
            if (r.success && r.data.length >= 66) {
              let amountOut: bigint;
              if (chain.dexProtocol === 'solidly') {
                amountOut = decAmountsOut(r.data);
              } else {
                amountOut = decU256(r.data);
              }
              if (amountOut > 0n) {
                price = Number(amountOut) / 10 ** chain.quoteDecimals;
                break;
              }
            }
          }

          console.log(`  ${chain.name} DEX: $${price?.toFixed(2) ?? 'FAIL'} (${results.map((r, i) => `fee=${chain.feeTiers?.[i] ?? (i === 0 ? 'volatile' : 'stable')}:${r.success ? 'OK' : 'FAIL'}`).join(', ')})`);
          expect(price).not.toBeNull();
          expect(price!).toBeGreaterThan(0);
        });
      }

      if (chain.chainlinkFeed) {
        test('Chainlink price query', async () => {
          const calls: Call3[] = [
            mc(chain.chainlinkFeed!, encLatestRound()),
          ];

          const encoded = encAggregate3(calls);
          const raw = await ethCall(chain.rpc, MULTICALL3, encoded);
          const results = decAggregate3(raw);

          expect(results[0].success).toBe(true);
          expect(results[0].data.length).toBeGreaterThanOrEqual(66);

          const price = decChainlinkUsd(results[0].data);
          console.log(`  ${chain.name} Chainlink: $${price.toFixed(2)}`);
          expect(price).toBeGreaterThan(0);
        });
      }
    });
  }
});
