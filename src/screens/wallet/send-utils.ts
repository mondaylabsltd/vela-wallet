/**
 * Pure, stateless helpers for the Send flow — extracted from SendScreen so the
 * screen file holds orchestration, not utility math. No React, no side effects.
 */
import { chainName, nativeSymbol, networkId } from '@/models/network';
import type { APIToken } from '@/models/types';

export function isValidAddress(addr: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(addr);
}

export function explorerUserOpUrl(chainId: number): string | null {
  // Most ERC-4337 UserOps can be tracked via jiffyscan
  return `https://jiffyscan.xyz`;
}

export function amountToWeiHex(amount: string, decimals: number): string {
  const parts = amount.split('.');
  const intPart = parts[0] || '0';
  let fracPart = parts[1] || '';
  if (fracPart.length > decimals) {
    fracPart = fracPart.slice(0, decimals);
  } else {
    fracPart = fracPart.padEnd(decimals, '0');
  }
  const weiStr = (intPart + fracPart).replace(/^0+/, '') || '0';
  let n = BigInt(weiStr);
  return n.toString(16);
}

/** Convert a human-readable balance string (e.g. "0.0113") to BigInt wei. */
export function balanceToWei(balance: string, decimals: number): bigint {
  return BigInt('0x' + amountToWeiHex(balance, decimals));
}

/** Whether a native transfer and its already-quoted in-band fee fit in the Safe. */
export function canCoverNativeTransfer(amountWei: bigint, balanceWei: bigint, quotedFeeWei: bigint): boolean {
  return amountWei >= 0n && quotedFeeWei >= 0n && amountWei + quotedFeeWei <= balanceWei;
}

/** ERC-20 `transfer(address,uint256)` calldata, for the balance-change pre-check. */
export function encErc20Transfer(to: string, amountHex: string): string {
  const a = to.replace(/^0x/, '').toLowerCase().padStart(64, '0');
  const amt = amountHex.replace(/^0x/, '').padStart(64, '0');
  return '0xa9059cbb' + a + amt;
}

/** A zero-balance native token for a chain the user holds nothing on (locked EIP-681 send). */
export function synthNativeToken(chainId: number): APIToken {
  const sym = nativeSymbol(chainId);
  return { network: networkId(chainId), chainName: chainName(chainId), symbol: sym, balance: '0', decimals: 18, logo: null, name: sym, tokenAddress: null, priceUsd: null, spam: false };
}

/** A zero-balance ERC-20 placeholder built from resolved metadata (locked EIP-681 send). */
export function synthErc20Token(chainId: number, address: string, symbol: string, decimals: number): APIToken {
  return { network: networkId(chainId), chainName: chainName(chainId), symbol, balance: '0', decimals, logo: null, name: symbol, tokenAddress: address, priceUsd: null, spam: false };
}

/**
 * Font size for the amount the user is typing. Big-tech input pattern (Cash App):
 * the number stays on one line and shrinks *smoothly* as digits are added — no
 * visible step jumps, never abbreviated (you must see exactly what you typed).
 */
export function amountFontSize(value: string): number {
  const len = Math.max(value.length, 1);
  const size = Math.round(230 / Math.max(len, 5.75)); // ~5.75 chars at the 40px max
  return Math.max(17, Math.min(40, size));
}

/** Validate and constrain amount input: max `maxDecimals` decimal places, valid number chars only. */
export function sanitizeAmountInput(text: string, maxDecimals: number): string | null {
  // Allow only digits and a single dot
  const cleaned = text.replace(/[^0-9.]/g, '');
  // Reject multiple dots
  if ((cleaned.match(/\./g) || []).length > 1) return null;

  const parts = cleaned.split('.');
  if (parts.length === 2 && parts[1].length > maxDecimals) {
    // Truncate excess decimals
    return parts[0] + '.' + parts[1].slice(0, maxDecimals);
  }
  return cleaned;
}

export function shortAddr(addr: string): string {
  if (addr.length <= 14) return addr;
  return `${addr.slice(0, 8)}...${addr.slice(-6)}`;
}
