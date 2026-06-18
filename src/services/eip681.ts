/**
 * EIP-681 payment-request URIs.
 * https://eips.ethereum.org/EIPS/eip-681
 *
 * We build the canonical forms below and parse them tolerantly (accepting the
 * legacy `pay-` prefix and scientific-notation amounts like `2.014e18`):
 *
 *   native, no amount   ethereum:<recipient>@<chainId>
 *   native + amount     ethereum:<recipient>@<chainId>?value=<wei>
 *   token, no amount    ethereum:<tokenAddr>@<chainId>/transfer?address=<recipient>
 *   token + amount      ethereum:<tokenAddr>@<chainId>/transfer?address=<recipient>&uint256=<baseUnits>
 *
 * This module is intentionally framework-free (no React / app-state imports) so
 * it can be unit-tested in isolation and reused from any layer.
 */

const ADDR_RE = /^0x[0-9a-fA-F]{40}$/;

export interface EIP681Request {
  /** Chain ID, or undefined when the URI omits `@chainId`. */
  chainId?: number;
  /** The address that should ultimately receive the funds. */
  recipient: string;
  /** ERC-20 contract address, or undefined for a native-coin payment. */
  tokenAddress?: string;
  /** Amount in the token's smallest unit (wei for native), or undefined if unspecified. */
  amountBaseUnits?: bigint;
  /** True for a native-coin payment, false for an ERC-20 `transfer`. */
  isNative: boolean;
}

export interface BuildEIP681Args {
  recipient: string;
  chainId: number;
  /** Omit (or null) for a native-coin request. */
  tokenAddress?: string | null;
  /** Decimals of the token (18 for native). Required when `amount` is set. */
  decimals?: number;
  /** Human-readable decimal amount, e.g. "1.5". Omit/empty for an open amount. */
  amount?: string;
}

export function isHexAddress(s: string): boolean {
  return ADDR_RE.test(s.trim());
}

/** Convert a human decimal string ("1.5") to integer base units given `decimals`. */
export function toBaseUnits(amount: string, decimals: number): bigint {
  const cleaned = (amount || '').trim();
  if (!cleaned) return 0n;
  const [intPart, fracPart = ''] = cleaned.split('.');
  const frac = (fracPart + '0'.repeat(decimals)).slice(0, decimals);
  return BigInt((intPart || '0') + frac);
}

/** Convert integer base units back to a trimmed human decimal string. */
export function fromBaseUnits(value: bigint, decimals: number): string {
  const s = value.toString();
  if (decimals <= 0) return s;
  const padded = s.padStart(decimals + 1, '0');
  const intPart = padded.slice(0, padded.length - decimals);
  const frac = padded.slice(padded.length - decimals).replace(/0+$/, '');
  return frac ? `${intPart}.${frac}` : intPart;
}

/**
 * Parse an EIP-681 numeric value (wei / uint256) into an integer bigint.
 * Accepts plain integers, decimals, and scientific notation (`2.014e18`).
 * Returns null for anything unparseable or negative.
 */
function parseAmount(raw: string): bigint | null {
  const m = raw.trim().match(/^([+-]?)(\d+)(?:\.(\d+))?(?:[eE]([+-]?\d+))?$/);
  if (!m) return null;
  const [, sign, intDigits, fracDigits = '', expStr = '0'] = m;
  if (sign === '-') return null;
  const digits = intDigits + fracDigits;
  const decimalExp = parseInt(expStr, 10) - fracDigits.length;
  let value: bigint;
  try {
    value = BigInt(digits);
  } catch {
    return null;
  }
  if (decimalExp >= 0) return value * 10n ** BigInt(decimalExp);
  // Negative exponent: truncate to an integer (base units are always integral).
  return value / 10n ** BigInt(-decimalExp);
}

/** Build a canonical EIP-681 URI for a receive request. */
export function buildEIP681(args: BuildEIP681Args): string {
  const { recipient, chainId, tokenAddress, decimals = 18, amount } = args;
  const hasAmount = !!amount && parseFloat(amount) > 0;

  if (tokenAddress) {
    let uri = `ethereum:${tokenAddress}@${chainId}/transfer?address=${recipient}`;
    if (hasAmount) uri += `&uint256=${toBaseUnits(amount!, decimals).toString()}`;
    return uri;
  }

  let uri = `ethereum:${recipient}@${chainId}`;
  if (hasAmount) uri += `?value=${toBaseUnits(amount!, decimals).toString()}`;
  return uri;
}

/**
 * Parse an EIP-681 URI. Returns null for anything that isn't a recognizable
 * `ethereum:` request (plain addresses, walletconnect/walletpair URIs, etc.),
 * so callers can fall through to their existing handling.
 */
export function parseEIP681(input: string): EIP681Request | null {
  let s = (input || '').trim();
  if (!/^ethereum:/i.test(s)) return null;
  s = s.slice('ethereum:'.length);
  if (s.startsWith('pay-')) s = s.slice('pay-'.length);

  // Split query string off.
  const qIndex = s.indexOf('?');
  const path = qIndex >= 0 ? s.slice(0, qIndex) : s;
  const query = qIndex >= 0 ? s.slice(qIndex + 1) : '';
  const params = parseQuery(query);

  // path = target[@chainId][/function]
  const slashIndex = path.indexOf('/');
  const targetWithChain = slashIndex >= 0 ? path.slice(0, slashIndex) : path;
  const functionName = slashIndex >= 0 ? path.slice(slashIndex + 1) : '';

  const atIndex = targetWithChain.indexOf('@');
  const target = (atIndex >= 0 ? targetWithChain.slice(0, atIndex) : targetWithChain).trim();
  const chainIdStr = atIndex >= 0 ? targetWithChain.slice(atIndex + 1) : '';

  if (!target) return null;
  const chainId = chainIdStr && /^\d+$/.test(chainIdStr) ? parseInt(chainIdStr, 10) : undefined;

  if (functionName === 'transfer') {
    // ERC-20: target is the token contract, recipient is the `address` param.
    const recipient = (params.address || '').trim();
    if (!isHexAddress(target) || !isHexAddress(recipient)) return null;
    const amountRaw = params.uint256 ?? params.value;
    return {
      chainId,
      recipient,
      tokenAddress: target,
      amountBaseUnits: amountRaw != null ? parseAmount(amountRaw) ?? undefined : undefined,
      isNative: false,
    };
  }

  // Native payment: target is the recipient. Accept only real addresses — ENS
  // resolution is out of scope and the send flow needs a 0x recipient anyway.
  if (!isHexAddress(target)) return null;
  return {
    chainId,
    recipient: target,
    amountBaseUnits: params.value != null ? parseAmount(params.value) ?? undefined : undefined,
    isNative: true,
  };
}

/** Public fallback host for payment links when there's no web origin (native app). */
export const PAY_LINK_FALLBACK = 'https://getvela.app/pay';

/**
 * The base for payment links. On the web we use the *current* origin's /pay
 * route, so a self-hosted wallet (e.g. mydomain.com) produces links that point
 * back to its own deployment. On native (no `window`) we fall back to the
 * public hosted page.
 */
export function payLinkBase(): string {
  try {
    if (typeof window !== 'undefined' && window.location?.origin) {
      return `${window.location.origin}/pay`;
    }
  } catch {
    /* not a browser */
  }
  return PAY_LINK_FALLBACK;
}

export interface PayLinkArgs {
  recipient: string;
  chainId: number;
  tokenAddress?: string | null;
  /** Human-readable decimal amount; omitted/zero → open amount. */
  amount?: string;
  /** Display hints so the landing page needs no RPC. */
  symbol: string;
  decimals: number;
  networkName: string;
  baseUrl?: string;
}

/**
 * Build a public, shareable payment-link URL — a web page (default getvela.app/pay)
 * that bridges to the Vela web wallet, to other EIP-681 wallets, or to manual entry.
 * The EIP-681 fields are carried as URL-encoded query params.
 */
export function buildPayLink(a: PayLinkArgs): string {
  const base = a.baseUrl ?? payLinkBase();
  const parts = [`to=${encodeURIComponent(a.recipient)}`, `chain=${a.chainId}`];
  if (a.tokenAddress) parts.push(`token=${encodeURIComponent(a.tokenAddress)}`);
  if (a.amount && parseFloat(a.amount) > 0) parts.push(`amount=${encodeURIComponent(a.amount)}`);
  parts.push(`sym=${encodeURIComponent(a.symbol)}`);
  parts.push(`dec=${a.decimals}`);
  if (a.networkName) parts.push(`net=${encodeURIComponent(a.networkName)}`);
  return `${base}?${parts.join('&')}`;
}

function parseQuery(query: string): Record<string, string> {
  const out: Record<string, string> = {};
  if (!query) return out;
  for (const pair of query.split('&')) {
    if (!pair) continue;
    const eq = pair.indexOf('=');
    const key = eq >= 0 ? pair.slice(0, eq) : pair;
    const val = eq >= 0 ? pair.slice(eq + 1) : '';
    try {
      out[decodeURIComponent(key)] = decodeURIComponent(val);
    } catch {
      out[key] = val;
    }
  }
  return out;
}
