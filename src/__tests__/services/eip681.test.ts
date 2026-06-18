/**
 * EIP-681 build/parse unit tests.
 */
import { buildEIP681, parseEIP681, toBaseUnits, fromBaseUnits } from '@/services/eip681';

const ME = '0x742d35Cc6634C0532925a3b844Bc454e4438f44e';
const USDC = '0x3c499c542cEF5E3811e1192ce70d8cc03d5c3359'; // USDC on Polygon

describe('base-unit helpers', () => {
  it('toBaseUnits handles decimals and truncation', () => {
    expect(toBaseUnits('1.5', 6)).toBe(1500000n);
    expect(toBaseUnits('0.1', 18)).toBe(100000000000000000n);
    expect(toBaseUnits('1.23456789', 6)).toBe(1234567n); // truncates excess
    expect(toBaseUnits('', 6)).toBe(0n);
    expect(toBaseUnits('10', 0)).toBe(10n);
  });

  it('fromBaseUnits round-trips', () => {
    expect(fromBaseUnits(1500000n, 6)).toBe('1.5');
    expect(fromBaseUnits(100000000000000000n, 18)).toBe('0.1');
    expect(fromBaseUnits(10n, 0)).toBe('10');
    expect(fromBaseUnits(0n, 6)).toBe('0');
  });
});

describe('buildEIP681', () => {
  it('network only (native, no amount)', () => {
    expect(buildEIP681({ recipient: ME, chainId: 137 })).toBe(`ethereum:${ME}@137`);
  });

  it('native + amount', () => {
    expect(buildEIP681({ recipient: ME, chainId: 1, decimals: 18, amount: '0.1' }))
      .toBe(`ethereum:${ME}@1?value=100000000000000000`);
  });

  it('token, no amount', () => {
    expect(buildEIP681({ recipient: ME, chainId: 137, tokenAddress: USDC }))
      .toBe(`ethereum:${USDC}@137/transfer?address=${ME}`);
  });

  it('token + amount', () => {
    expect(buildEIP681({ recipient: ME, chainId: 137, tokenAddress: USDC, decimals: 6, amount: '1.5' }))
      .toBe(`ethereum:${USDC}@137/transfer?address=${ME}&uint256=1500000`);
  });

  it('omits amount when zero/empty', () => {
    expect(buildEIP681({ recipient: ME, chainId: 1, decimals: 18, amount: '0' }))
      .toBe(`ethereum:${ME}@1`);
  });
});

describe('parseEIP681', () => {
  it('returns null for non-EIP-681 input', () => {
    expect(parseEIP681(ME)).toBeNull();
    expect(parseEIP681('walletpair:?ch=abc')).toBeNull();
    expect(parseEIP681('https://example.com')).toBeNull();
    expect(parseEIP681('')).toBeNull();
  });

  it('parses network only', () => {
    expect(parseEIP681(`ethereum:${ME}@137`)).toEqual({
      chainId: 137, recipient: ME, isNative: true, amountBaseUnits: undefined,
    });
  });

  it('parses native + amount', () => {
    const r = parseEIP681(`ethereum:${ME}@1?value=100000000000000000`);
    expect(r).toMatchObject({ chainId: 1, recipient: ME, isNative: true, amountBaseUnits: 100000000000000000n });
  });

  it('parses scientific-notation value', () => {
    const r = parseEIP681(`ethereum:${ME}@1?value=2.014e18`);
    expect(r?.amountBaseUnits).toBe(2014000000000000000n);
  });

  it('parses ERC-20 transfer + amount', () => {
    const r = parseEIP681(`ethereum:${USDC}@137/transfer?address=${ME}&uint256=1500000`);
    expect(r).toMatchObject({
      chainId: 137, recipient: ME, tokenAddress: USDC, isNative: false, amountBaseUnits: 1500000n,
    });
  });

  it('accepts the legacy pay- prefix', () => {
    expect(parseEIP681(`ethereum:pay-${ME}@1`)).toMatchObject({ chainId: 1, recipient: ME, isNative: true });
  });

  it('parses a chainless request', () => {
    expect(parseEIP681(`ethereum:${ME}`)).toMatchObject({ chainId: undefined, recipient: ME, isNative: true });
  });

  it('rejects a transfer without a valid address param', () => {
    expect(parseEIP681(`ethereum:${USDC}@137/transfer?uint256=1500000`)).toBeNull();
  });

  it('round-trips build → parse', () => {
    const uri = buildEIP681({ recipient: ME, chainId: 137, tokenAddress: USDC, decimals: 6, amount: '1.5' });
    const r = parseEIP681(uri)!;
    expect(r.recipient).toBe(ME);
    expect(r.tokenAddress).toBe(USDC);
    expect(fromBaseUnits(r.amountBaseUnits!, 6)).toBe('1.5');
  });
});
