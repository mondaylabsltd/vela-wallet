/**
 * Tests for the approval guard — the "unlimited can never leave the wallet" core.
 * Security-critical pure code: aim for exhaustive coverage of detect / rewrite /
 * enforceNoUnlimited across every approval shape.
 */
import {
  detectApproval,
  rewriteApprovalParams,
  enforceNoUnlimited,
  isUnboundedAmount,
  parseTokenAmount,
  formatTokenAmount,
  UnlimitedApprovalError,
  UNLIMITED_CAP_256,
  UNLIMITED_CAP_160,
} from '@/services/approval-guard';

// --- calldata fixtures ---------------------------------------------------
const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
const SPENDER = '0x111111125421cA6dc452d289314280a0f8842A65';
const word = (hex: string) => stripPad(hex);
function stripPad(hex: string): string {
  return hex.replace(/^0x/, '').padStart(64, '0');
}
function addrWord(addr: string): string {
  return addr.replace(/^0x/, '').toLowerCase().padStart(64, '0');
}
function amtWord(v: bigint): string {
  return v.toString(16).padStart(64, '0');
}
const MAX_U256 = (1n << 256n) - 1n;
const MAX_U160 = (1n << 160n) - 1n;

const approveCalldata = (spender: string, amount: bigint) =>
  '0x095ea7b3' + addrWord(spender) + amtWord(amount);
const increaseCalldata = (spender: string, amount: bigint) =>
  '0x39509351' + addrWord(spender) + amtWord(amount);
const decreaseCalldata = (spender: string, amount: bigint) =>
  '0xa457c2d7' + addrWord(spender) + amtWord(amount);
const setApprovalForAllCalldata = (op: string, approved: boolean) =>
  '0xa22cb465' + addrWord(op) + amtWord(approved ? 1n : 0n);

const txReq = (to: string, data: string) => ({
  method: 'eth_sendTransaction',
  params: [{ to, data, value: '0x0' }],
});

const erc2612 = (value: string, deadline = '1750000000') => ({
  method: 'eth_signTypedData_v4',
  params: ['0x0', JSON.stringify({
    types: { Permit: [
      { name: 'owner', type: 'address' }, { name: 'spender', type: 'address' },
      { name: 'value', type: 'uint256' }, { name: 'nonce', type: 'uint256' },
      { name: 'deadline', type: 'uint256' },
    ] },
    primaryType: 'Permit',
    domain: { name: 'USD Coin', chainId: 1, verifyingContract: USDC },
    message: { owner: '0xaf5e', spender: SPENDER, value, nonce: '0', deadline },
  })],
});

const daiPermit = (allowed: boolean) => ({
  method: 'eth_signTypedData_v4',
  params: ['0x0', JSON.stringify({
    types: { Permit: [
      { name: 'holder', type: 'address' }, { name: 'spender', type: 'address' },
      { name: 'nonce', type: 'uint256' }, { name: 'expiry', type: 'uint256' },
      { name: 'allowed', type: 'bool' },
    ] },
    primaryType: 'Permit',
    domain: { name: 'Dai', verifyingContract: '0x6b175474e89094c44da98b954eedeac495271d0f' },
    message: { holder: '0xaf5e', spender: SPENDER, nonce: '0', expiry: '1750000000', allowed },
  })],
});

const permit2Single = (amount: string) => ({
  method: 'eth_signTypedData_v4',
  params: ['0x0', JSON.stringify({
    types: { PermitSingle: [], PermitDetails: [] },
    primaryType: 'PermitSingle',
    domain: { name: 'Permit2', chainId: 1, verifyingContract: '0x000000000022D473030F116dDEE9F6B43aC78BA3' },
    message: {
      details: { token: USDC, amount, expiration: '1750000000', nonce: '0' },
      spender: SPENDER, sigDeadline: '1750000000',
    },
  })],
});

describe('approval-guard', () => {
  describe('isUnboundedAmount', () => {
    test('uint256 sentinels are unbounded', () => {
      expect(isUnboundedAmount(MAX_U256, 256)).toBe(true);
      expect(isUnboundedAmount(1n << 255n, 256)).toBe(true);
      expect(isUnboundedAmount(UNLIMITED_CAP_256, 256)).toBe(true);
    });
    test('uint160 sentinel is unbounded', () => {
      expect(isUnboundedAmount(MAX_U160, 160)).toBe(true);
      expect(isUnboundedAmount(UNLIMITED_CAP_160, 160)).toBe(true);
    });
    test('large-but-legit amounts are NOT unbounded', () => {
      // 1 quadrillion tokens * 1e18 ≈ 2^110 — well under the cap
      expect(isUnboundedAmount(10n ** 33n, 256)).toBe(false);
      expect(isUnboundedAmount(10n ** 33n, 160)).toBe(false);
      expect(isUnboundedAmount(500_000_000n, 256)).toBe(false);
    });
  });

  describe('detectApproval — calldata', () => {
    test('ERC-20 approve (unlimited)', () => {
      const d = detectApproval(...args(txReq(USDC, approveCalldata(SPENDER, MAX_U256))));
      expect(d).toMatchObject({ kind: 'erc20-approve', tokenAddress: USDC.toLowerCase(), spender: SPENDER.toLowerCase(), isUnbounded: true, amountBits: 256, isBooleanGrant: false, editable: true });
      expect(d!.amountRaw).toBe(MAX_U256);
    });
    test('ERC-20 approve (limited 500 USDC)', () => {
      const d = detectApproval(...args(txReq(USDC, approveCalldata(SPENDER, 500_000000n))));
      expect(d!.isUnbounded).toBe(false);
      expect(d!.amountRaw).toBe(500_000000n);
    });
    test('approve to 0 is reducing (revoke)', () => {
      const d = detectApproval(...args(txReq(USDC, approveCalldata(SPENDER, 0n))));
      expect(d!.isReducing).toBe(true);
    });
    test('increaseAllowance', () => {
      const d = detectApproval(...args(txReq(USDC, increaseCalldata(SPENDER, MAX_U256))));
      expect(d).toMatchObject({ kind: 'increaseAllowance', isUnbounded: true });
    });
    test('decreaseAllowance is safe/reducing, never unbounded', () => {
      const d = detectApproval(...args(txReq(USDC, decreaseCalldata(SPENDER, MAX_U256))));
      expect(d).toMatchObject({ kind: 'decreaseAllowance', isReducing: true, isUnbounded: false });
    });
    test('setApprovalForAll grant', () => {
      const d = detectApproval(...args(txReq(USDC, setApprovalForAllCalldata(SPENDER, true))));
      expect(d).toMatchObject({ kind: 'setApprovalForAll', isBooleanGrant: true, isUnbounded: true, isReducing: false });
    });
    test('setApprovalForAll revoke', () => {
      const d = detectApproval(...args(txReq(USDC, setApprovalForAllCalldata(SPENDER, false))));
      expect(d).toMatchObject({ isBooleanGrant: true, isUnbounded: false, isReducing: true });
    });
    test('Permit2 on-chain approve (unlimited uint160)', () => {
      // approve(address token, address spender, uint160 amount, uint48 expiration)
      const PERMIT2 = '0x000000000022D473030F116dDEE9F6B43aC78BA3';
      const cd = '0x87517c45' + addrWord(USDC) + addrWord(SPENDER) + amtWord(MAX_U160) + amtWord(1750000000n);
      const d = detectApproval(...args(txReq(PERMIT2, cd)));
      // token is the FIRST arg, not the tx `to` (the Permit2 contract).
      expect(d).toMatchObject({ kind: 'permit2-single', tokenAddress: USDC.toLowerCase(), spender: SPENDER.toLowerCase(), amountBits: 160, isUnbounded: true, editable: true });
    });
    test('non-approval calldata → null', () => {
      // transfer(address,uint256)
      const transfer = '0xa9059cbb' + addrWord(SPENDER) + amtWord(1000n);
      expect(detectApproval(...args(txReq(USDC, transfer)))).toBeNull();
    });
    test('plain ETH send → null', () => {
      expect(detectApproval('eth_sendTransaction', [{ to: SPENDER, data: '0x', value: '0x1' }])).toBeNull();
    });
  });

  describe('detectApproval — typed data', () => {
    test('ERC-2612 permit unlimited', () => {
      const d = detectApproval(...args(erc2612(MAX_U256.toString())));
      expect(d).toMatchObject({ kind: 'erc2612-permit', tokenAddress: USDC.toLowerCase(), isUnbounded: true, amountBits: 256 });
      expect(d!.deadline).toBe(1750000000n);
    });
    test('ERC-2612 permit finite (1000 USDC)', () => {
      const d = detectApproval(...args(erc2612('1000000000')));
      expect(d!.isUnbounded).toBe(false);
      expect(d!.amountRaw).toBe(1000000000n);
    });
    test('DAI permit allowed=true is boolean unbounded grant', () => {
      const d = detectApproval(...args(daiPermit(true)));
      expect(d).toMatchObject({ kind: 'dai-permit', isBooleanGrant: true, isUnbounded: true });
      expect(d!.blockReason).toBeTruthy();
    });
    test('DAI permit allowed=false is revoke', () => {
      const d = detectApproval(...args(daiPermit(false)));
      expect(d).toMatchObject({ kind: 'dai-permit', isReducing: true, isUnbounded: false });
    });
    test('Permit2 single unlimited (uint160 max)', () => {
      const d = detectApproval(...args(permit2Single(MAX_U160.toString())));
      expect(d).toMatchObject({ kind: 'permit2-single', amountBits: 160, isUnbounded: true });
    });
    test('Permit2 single finite', () => {
      const d = detectApproval(...args(permit2Single('1000000000')));
      expect(d!.isUnbounded).toBe(false);
    });
  });

  describe('rewriteApprovalParams — calldata', () => {
    test('caps an unlimited approve to a finite amount; spender preserved; no max word remains', () => {
      const req = txReq(USDC, approveCalldata(SPENDER, MAX_U256));
      const d = detectApproval(...args(req))!;
      const out = rewriteApprovalParams(req.method, req.params, d, { type: 'amount', amountRaw: 500_000000n });
      const newData = out[0].data;
      // re-detect the rewritten calldata
      const d2 = detectApproval('eth_sendTransaction', [{ to: USDC, data: newData, value: '0x0' }])!;
      expect(d2.amountRaw).toBe(500_000000n);
      expect(d2.spender).toBe(SPENDER.toLowerCase());
      expect(d2.isUnbounded).toBe(false);
      expect(newData.toLowerCase()).not.toContain('f'.repeat(64)); // no 2^256-1 word
    });
    test('revoke sets amount to 0', () => {
      const req = txReq(USDC, approveCalldata(SPENDER, MAX_U256));
      const d = detectApproval(...args(req))!;
      const out = rewriteApprovalParams(req.method, req.params, d, { type: 'revoke' });
      const d2 = detectApproval('eth_sendTransaction', [{ to: USDC, data: out[0].data, value: '0x0' }])!;
      expect(d2.amountRaw).toBe(0n);
    });
    test('rewriting to an unbounded amount THROWS', () => {
      const req = txReq(USDC, approveCalldata(SPENDER, 100n));
      const d = detectApproval(...args(req))!;
      expect(() => rewriteApprovalParams(req.method, req.params, d, { type: 'amount', amountRaw: MAX_U256 }))
        .toThrow(/Unlimited/i);
      expect(() => rewriteApprovalParams(req.method, req.params, d, { type: 'amount', amountRaw: UNLIMITED_CAP_256 }))
        .toThrow(/Unlimited/i);
    });
    test('does not mutate the input params', () => {
      const req = txReq(USDC, approveCalldata(SPENDER, MAX_U256));
      const before = JSON.stringify(req.params);
      const d = detectApproval(...args(req))!;
      rewriteApprovalParams(req.method, req.params, d, { type: 'amount', amountRaw: 1n });
      expect(JSON.stringify(req.params)).toBe(before);
    });
    test('setApprovalForAll revoke flips bool to false', () => {
      const req = txReq(USDC, setApprovalForAllCalldata(SPENDER, true));
      const d = detectApproval(...args(req))!;
      const out = rewriteApprovalParams(req.method, req.params, d, { type: 'revoke' });
      const d2 = detectApproval('eth_sendTransaction', [{ to: USDC, data: out[0].data, value: '0x0' }])!;
      expect(d2.isUnbounded).toBe(false);
      expect(d2.isReducing).toBe(true);
    });
  });

  describe('rewriteApprovalParams — typed data', () => {
    test('caps an unlimited ERC-2612 permit; preserves owner/spender/nonce/deadline', () => {
      const req = erc2612(MAX_U256.toString());
      const d = detectApproval(...args(req))!;
      const out = rewriteApprovalParams(req.method, req.params, d, { type: 'amount', amountRaw: 1000000000n });
      const td = JSON.parse(out[1]);
      expect(td.message.value).toBe('1000000000');
      expect(td.message.spender).toBe(SPENDER);
      expect(td.message.nonce).toBe('0');
      expect(td.message.deadline).toBe('1750000000');
    });
    test('caps an unlimited Permit2 single', () => {
      const req = permit2Single(MAX_U160.toString());
      const d = detectApproval(...args(req))!;
      const out = rewriteApprovalParams(req.method, req.params, d, { type: 'amount', amountRaw: 1000000000n });
      const td = JSON.parse(out[1]);
      expect(td.message.details.amount).toBe('1000000000');
    });
    test('DAI permit revoke sets allowed=false', () => {
      const req = daiPermit(true);
      const d = detectApproval(...args(req))!;
      const out = rewriteApprovalParams(req.method, req.params, d, { type: 'revoke' });
      const td = JSON.parse(out[1]);
      expect(td.message.allowed).toBe(false);
    });
    test('rewriting a typed permit to unbounded THROWS', () => {
      const req = erc2612('100');
      const d = detectApproval(...args(req))!;
      expect(() => rewriteApprovalParams(req.method, req.params, d, { type: 'amount', amountRaw: MAX_U256 }))
        .toThrow(/Unlimited/i);
    });
  });

  describe('enforceNoUnlimited — the final guard (descriptor-independent)', () => {
    test('throws on raw unlimited approve even with no UI in the loop', () => {
      const req = txReq(USDC, approveCalldata(SPENDER, MAX_U256));
      expect(() => enforceNoUnlimited(req.method, req.params)).toThrow(UnlimitedApprovalError);
    });
    test('throws on unlimited increaseAllowance', () => {
      const req = txReq(USDC, increaseCalldata(SPENDER, MAX_U256));
      expect(() => enforceNoUnlimited(req.method, req.params)).toThrow(UnlimitedApprovalError);
    });
    // Off-chain permit SIGNATURES are signed verbatim under deliberate UI consent:
    // the dApp redeems its own struct, so a forced cap would only desync the
    // signature and revert the swap. The amount guard governs txs the WALLET
    // submits (calldata), not signatures the dApp redeems (typed data).
    test('does NOT throw on unlimited ERC-2612 permit (off-chain signature)', () => {
      const req = erc2612(MAX_U256.toString());
      expect(() => enforceNoUnlimited(req.method, req.params)).not.toThrow();
    });
    test('does NOT throw on unlimited Permit2 single typed-data (off-chain signature)', () => {
      const req = permit2Single(MAX_U160.toString());
      expect(() => enforceNoUnlimited(req.method, req.params)).not.toThrow();
    });
    test('throws on unlimited Permit2 on-chain approve (calldata)', () => {
      const PERMIT2 = '0x000000000022D473030F116dDEE9F6B43aC78BA3';
      const cd = '0x87517c45' + addrWord(USDC) + addrWord(SPENDER) + amtWord(MAX_U160) + amtWord(1750000000n);
      const req = txReq(PERMIT2, cd);
      expect(() => enforceNoUnlimited(req.method, req.params)).toThrow(UnlimitedApprovalError);
    });
    test('allows a finite approve', () => {
      const req = txReq(USDC, approveCalldata(SPENDER, 500_000000n));
      expect(() => enforceNoUnlimited(req.method, req.params)).not.toThrow();
    });
    test('allows decreaseAllowance(max) — reducing, not granting', () => {
      const req = txReq(USDC, decreaseCalldata(SPENDER, MAX_U256));
      expect(() => enforceNoUnlimited(req.method, req.params)).not.toThrow();
    });
    test('allows setApprovalForAll(true) — boolean grant handled by UI consent, not this guard', () => {
      const req = txReq(USDC, setApprovalForAllCalldata(SPENDER, true));
      expect(() => enforceNoUnlimited(req.method, req.params)).not.toThrow();
    });
    test('allows a plain transfer', () => {
      const transfer = '0xa9059cbb' + addrWord(SPENDER) + amtWord(1000n);
      expect(() => enforceNoUnlimited(...args(txReq(USDC, transfer)))).not.toThrow();
    });
    test('end-to-end: rewrite then guard passes', () => {
      const req = txReq(USDC, approveCalldata(SPENDER, MAX_U256));
      const d = detectApproval(...args(req))!;
      const out = rewriteApprovalParams(req.method, req.params, d, { type: 'amount', amountRaw: 500_000000n });
      expect(() => enforceNoUnlimited(req.method, out)).not.toThrow();
    });
  });

  describe('parseTokenAmount / formatTokenAmount', () => {
    test('parses with decimals and commas', () => {
      expect(parseTokenAmount('1,234.5', 6)).toBe(1234_500000n);
      expect(parseTokenAmount('1000', 6)).toBe(1000_000000n);
      expect(parseTokenAmount('0.000001', 6)).toBe(1n);
    });
    test('rejects over-precision and junk', () => {
      expect(parseTokenAmount('0.0000001', 6)).toBeNull(); // 7 dp on a 6dp token
      expect(parseTokenAmount('abc', 18)).toBeNull();
      expect(parseTokenAmount('1.2.3', 18)).toBeNull();
    });
    test('round-trips (with thousands separators)', () => {
      expect(formatTokenAmount(1234_500000n, 6)).toBe('1,234.5');
      expect(formatTokenAmount(1000_000000n, 6)).toBe('1,000');
      expect(formatTokenAmount(0n, 6)).toBe('0');
      // format → parse round-trips despite the grouping commas
      expect(parseTokenAmount(formatTokenAmount(1234_500000n, 6), 6)).toBe(1234_500000n);
    });
  });
});

// helper to spread a request into (method, params)
function args(req: { method: string; params: any[] }): [string, any[]] {
  return [req.method, req.params];
}
