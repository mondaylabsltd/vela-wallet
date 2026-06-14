/**
 * Minimal ABI encoding/decoding for EVM contract calls.
 * Handles Multicall3, ERC-20, Uniswap V3 QuoterV2, Solidly Router, and Chainlink feeds.
 * No external dependencies -- pure hex manipulation.
 */

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Left-pad a bigint/number to 32 bytes (64 hex chars). */
function h(v: bigint | number, _bytes = 32): string {
  return BigInt(v).toString(16).padStart(64, '0');
}

/** Left-pad an address to 32 bytes. */
function addr(a: string): string {
  return (a.startsWith('0x') ? a.slice(2) : a).toLowerCase().padStart(64, '0');
}

/** Right-pad hex data to a 32-byte boundary. */
function padR(hex: string): string {
  const c = hex.startsWith('0x') ? hex.slice(2) : hex;
  return c.padEnd(Math.ceil(c.length / 64) * 64, '0');
}

/** Read a 256-bit word from a hex string at a given hex-char position. */
function n64(hex: string, pos: number): number {
  const s = hex.slice(pos, pos + 64);
  if (!s) return 0;
  return Number(BigInt('0x' + s.padStart(64, '0')));
}

// ---------------------------------------------------------------------------
// Function selectors (first 4 bytes of keccak256 of canonical signature)
// ---------------------------------------------------------------------------

export const SEL = {
  aggregate3:       '82ad56cb', // aggregate3((address,bool,bytes)[])
  getEthBalance:    '4d2301cc', // getEthBalance(address)
  balanceOf:        '70a08231', // balanceOf(address)
  decimals:         '313ce567', // decimals()
  symbol:           '95d89b41', // symbol()
  name:             '06fdde03', // name()
  quoteV3:          'c6a5026a', // quoteExactInputSingle((address,address,uint256,uint24,uint160))
  getAmountsOut:    '5509a1ac', // getAmountsOut(uint256,(address,address,bool,address)[]) — Aerodrome/Velodrome V2 Router
  latestRoundData:  'feaf968c', // latestRoundData()
} as const;

// ---------------------------------------------------------------------------
// Multicall3 -- canonical deployment on all EVM chains
// ---------------------------------------------------------------------------

export const MULTICALL3 = '0xcA11bde05977b3631167028862bE2a173976CA11';

export interface Call3 {
  target: string;
  allowFailure: boolean;
  callData: string;
}

export interface McResult {
  success: boolean;
  data: string;
}

/**
 * ABI-encode an `aggregate3(Call3[])` call.
 *
 * Call3 = (address target, bool allowFailure, bytes callData).
 * Since the tuple contains `bytes` (dynamic), each element needs an offset pointer.
 */
export function encAggregate3(calls: Call3[]): string {
  // Function selector + offset to the single dynamic param (always 0x20)
  let out = SEL.aggregate3 + h(0x20);
  // Array length
  out += h(calls.length);

  // Pre-encode each element so we can compute byte-level offsets
  const elems = calls.map(c => {
    const cd = c.callData.startsWith('0x') ? c.callData.slice(2) : c.callData;
    const cdLen = cd.length / 2;
    return (
      addr(c.target) +              // address target    (32 bytes)
      h(c.allowFailure ? 1 : 0) +   // bool allowFailure (32 bytes)
      h(0x60) +                      // offset to bytes   (3*32 = 96)
      h(cdLen) +                     // bytes length
      padR(cd)                       // bytes data (right-padded)
    );
  });

  // Offset pointers (relative to start of elements area, which is right after the length word)
  let off = calls.length * 32; // skip past all offset slots
  for (const e of elems) {
    out += h(off);
    off += e.length / 2; // each hex pair = 1 byte
  }

  // Append element encodings
  out += elems.join('');

  return '0x' + out;
}

/**
 * ABI-decode the return value of `aggregate3 -> Result[]`.
 *
 * Result = (bool success, bytes returnData).
 */
export function decAggregate3(hex: string): McResult[] {
  const d = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (d.length < 128) return [];

  const arrOff = n64(d, 0) * 2;          // byte offset -> hex-char offset
  if (arrOff + 64 > d.length) return [];

  const len = n64(d, arrOff);             // array length
  if (len <= 0 || len > 10_000) return []; // sanity cap
  const oStart = arrOff + 64;             // offsets begin after length word

  const out: McResult[] = [];
  for (let i = 0; i < len; i++) {
    const offPos = oStart + i * 64;
    if (offPos + 64 > d.length) break;

    const eOff = n64(d, offPos) * 2;
    const ePos = oStart + eOff;
    if (ePos + 128 > d.length) {
      out.push({ success: false, data: '0x' });
      continue;
    }

    const success = n64(d, ePos) !== 0;
    const bOff = n64(d, ePos + 64) * 2;   // offset to bytes within element
    const bPos = ePos + bOff;
    if (bPos + 64 > d.length) {
      out.push({ success, data: '0x' });
      continue;
    }

    const bLen = n64(d, bPos);             // bytes length
    const dataEnd = bPos + 64 + bLen * 2;
    const data = bLen > 0 && dataEnd <= d.length
      ? '0x' + d.slice(bPos + 64, dataEnd)
      : '0x';

    out.push({ success, data });
  }
  return out;
}

// ---------------------------------------------------------------------------
// ERC-20 call encoding
// ---------------------------------------------------------------------------

/** Encode `balanceOf(address)`. */
export function encBalanceOf(a: string): string {
  return '0x' + SEL.balanceOf + addr(a);
}

/** Encode `decimals()`. */
export function encDecimals(): string {
  return '0x' + SEL.decimals;
}

/** Encode Multicall3 `getEthBalance(address)`. */
export function encGetEthBalance(a: string): string {
  return '0x' + SEL.getEthBalance + addr(a);
}

// ---------------------------------------------------------------------------
// Uniswap V3 QuoterV2
// ---------------------------------------------------------------------------

/**
 * Encode `quoteExactInputSingle((address,address,uint256,uint24,uint160))`.
 * The struct is a static tuple, so encoding is just the concatenation of each field.
 */
export function encQuoteV3(
  tokenIn: string,
  tokenOut: string,
  amountIn: bigint,
  fee: number,
): string {
  return '0x' + SEL.quoteV3 +
    addr(tokenIn) +
    addr(tokenOut) +
    h(amountIn) +
    h(fee) +
    h(0n); // sqrtPriceLimitX96 = 0
}

// ---------------------------------------------------------------------------
// Aerodrome / Velodrome V2 Router
// ---------------------------------------------------------------------------

/**
 * Encode `getAmountsOut(uint256 amountIn, Route[] routes)` for a single-hop swap.
 * Route = (address from, address to, bool stable, address factory).
 * factory = 0x0 → use router's default factory.
 */
export function encGetAmountsOut(
  amountIn: bigint,
  tokenIn: string,
  tokenOut: string,
  stable: boolean,
): string {
  return '0x' + SEL.getAmountsOut +
    h(amountIn) +        // uint256 amountIn
    h(0x40) +            // offset to Route[] (2 head words × 32)
    h(1) +               // routes.length = 1
    addr(tokenIn) +      // route[0].from
    addr(tokenOut) +     // route[0].to
    h(stable ? 1 : 0) +  // route[0].stable
    h(0);                // route[0].factory = default (0x0)
}

/**
 * Decode the return of `getAmountsOut → uint256[]`.
 * For a single-hop route: amounts = [amountIn, amountOut].
 * Returns amountOut (the last element).
 */
export function decAmountsOut(hex: string): bigint {
  const d = hex.startsWith('0x') ? hex.slice(2) : hex;
  // Layout: offset(64) + length(64) + amounts[0](64) + amounts[1](64) = 256 hex chars
  if (d.length < 256) return 0n;
  return BigInt('0x' + d.slice(192, 256));
}

// ---------------------------------------------------------------------------
// Chainlink AggregatorV3
// ---------------------------------------------------------------------------

/** Encode `latestRoundData()` (no params). */
export function encLatestRound(): string {
  return '0x' + SEL.latestRoundData;
}

// ---------------------------------------------------------------------------
// Decoders
// ---------------------------------------------------------------------------

/** Decode the first 256-bit word as unsigned bigint. */
export function decU256(hex: string): bigint {
  const d = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (d.length < 64) return 0n;
  return BigInt('0x' + d.slice(0, 64));
}

/** Decode the first 256-bit word as signed bigint. */
export function decI256(hex: string): bigint {
  const v = decU256(hex);
  return v >= (1n << 255n) ? v - (1n << 256n) : v;
}

/** Decode as uint8 (for `decimals()`). */
export function decU8(hex: string): number {
  return Number(decU256(hex));
}

/**
 * Decode an ABI-encoded `string` return value — the `[offset][length][data]`
 * layout that most ERC-20 `name()` / `symbol()` calls use. Falls back to a
 * `bytes32` interpretation for legacy tokens (e.g. MKR) that return a single
 * fixed 32-byte word, and decodes the bytes as UTF-8 so multibyte symbols
 * (e.g. "USD₮0") survive intact. Returns '' for short or empty input.
 */
export function decString(hex: string): string {
  const d = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (d.length < 64) return '';
  // bytes32-style: a single 32-byte word, no offset/length header.
  if (d.length < 128) return utf8FromHex(d.slice(0, 64));
  const len = n64(d, 64);
  // A real dynamic string declares its byte length in the 2nd word; an
  // out-of-range value means this isn't offset-encoded → treat head as bytes32.
  if (len <= 0 || len > 4096 || 128 + len * 2 > d.length) return utf8FromHex(d.slice(0, 64));
  return utf8FromHex(d.slice(128, 128 + len * 2));
}

/** Decode a run of hex-encoded bytes as UTF-8, stopping at the first NUL. */
function utf8FromHex(dataHex: string): string {
  const bytes: number[] = [];
  for (let i = 0; i + 1 < dataHex.length; i += 2) {
    const b = parseInt(dataHex.slice(i, i + 2), 16);
    if (b === 0) break; // NUL = end of bytes32 padding / C-string terminator
    bytes.push(b);
  }
  let out = '';
  for (let i = 0; i < bytes.length; ) {
    const b0 = bytes[i];
    if (b0 < 0x80) { out += String.fromCharCode(b0); i += 1; }
    else if (b0 < 0xc0) { i += 1; } // stray continuation byte → skip
    else if (b0 < 0xe0 && i + 1 < bytes.length) {
      out += String.fromCharCode(((b0 & 0x1f) << 6) | (bytes[i + 1] & 0x3f));
      i += 2;
    } else if (b0 < 0xf0 && i + 2 < bytes.length) {
      out += String.fromCharCode(((b0 & 0x0f) << 12) | ((bytes[i + 1] & 0x3f) << 6) | (bytes[i + 2] & 0x3f));
      i += 3;
    } else if (i + 3 < bytes.length) {
      const cp = ((b0 & 0x07) << 18) | ((bytes[i + 1] & 0x3f) << 12) | ((bytes[i + 2] & 0x3f) << 6) | (bytes[i + 3] & 0x3f);
      const c = cp - 0x10000;
      out += String.fromCharCode(0xd800 + (c >> 10), 0xdc00 + (c & 0x3ff));
      i += 4;
    } else { i += 1; }
  }
  return out.trim();
}

/**
 * Decode Chainlink `latestRoundData()` return value.
 * Returns: (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound)
 * All Chainlink USD feeds use 8 decimals.
 */
export function decChainlinkUsd(hex: string): number {
  const d = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (d.length < 128) return 0;
  // answer is the 2nd word (int256, but always positive for USD feeds)
  const answer = BigInt('0x' + d.slice(64, 128));
  return Number(answer) / 1e8;
}

/**
 * Decode the raw `answer` (2nd word, int256) from Chainlink `latestRoundData()`
 * WITHOUT scaling. Pair with the feed's `decimals()` to compute the price
 * (`answer / 10**decimals`) — fiat/USD feeds vary (most are 8, e.g. PHP is 18).
 */
export function decChainlinkAnswer(hex: string): bigint {
  const d = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (d.length < 128) return 0n;
  const raw = BigInt('0x' + d.slice(64, 128));
  return raw >= 1n << 255n ? raw - (1n << 256n) : raw; // int256 sign
}
