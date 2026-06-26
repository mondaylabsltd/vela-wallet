/**
 * ABI decoder unit tests.
 *
 * Tests signature parsing, selector computation, calldata decoding
 * for all common Solidity types.
 */
import {
  parseSignature,
  canonicalize,
  computeSelector,
  decodeCalldata,
  matchSelector,
} from '@/services/abi-decode';

// ---------------------------------------------------------------------------
// Signature parsing
// ---------------------------------------------------------------------------

describe('parseSignature', () => {
  it('parses simple function', () => {
    const r = parseSignature('transfer(address _to, uint256 _value)');
    expect(r.name).toBe('transfer');
    expect(r.params).toHaveLength(2);
    expect(r.params[0]).toEqual({ type: 'address', name: '_to' });
    expect(r.params[1]).toEqual({ type: 'uint256', name: '_value' });
  });

  it('parses function with no params', () => {
    const r = parseSignature('totalSupply()');
    expect(r.name).toBe('totalSupply');
    expect(r.params).toHaveLength(0);
  });

  it('parses function with tuple', () => {
    const r = parseSignature('swap(address executor, (address srcToken, address dstToken) desc)');
    expect(r.name).toBe('swap');
    expect(r.params).toHaveLength(2);
    expect(r.params[1].type).toBe('tuple');
    expect(r.params[1].name).toBe('desc');
    expect(r.params[1].components).toHaveLength(2);
  });

  it('handles bool type', () => {
    const r = parseSignature('setApprovalForAll(address _operator, bool _approved)');
    expect(r.params[1]).toEqual({ type: 'bool', name: '_approved' });
  });

  it('handles dynamic array type', () => {
    const r = parseSignature('batchTransfer(address[] recipients, uint256[] amounts)');
    expect(r.params[0].type).toBe('address[]');
    expect(r.params[1].type).toBe('uint256[]');
  });

  it('handles bytes and string types', () => {
    const r = parseSignature('execute(bytes data, string memo)');
    expect(r.params[0].type).toBe('bytes');
    expect(r.params[1].type).toBe('string');
  });

  it('handles fixed-size bytes', () => {
    const r = parseSignature('foo(bytes32 hash)');
    expect(r.params[0].type).toBe('bytes32');
  });
});

// ---------------------------------------------------------------------------
// Canonicalize
// ---------------------------------------------------------------------------

describe('canonicalize', () => {
  it('strips param names', () => {
    expect(canonicalize('transfer(address _to, uint256 _value)'))
      .toBe('transfer(address,uint256)');
  });

  it('handles tuples', () => {
    const sig = 'swap(address executor, (address srcToken, address dstToken) desc)';
    expect(canonicalize(sig)).toBe('swap(address,(address,address))');
  });

  it('handles no-param functions', () => {
    expect(canonicalize('totalSupply()')).toBe('totalSupply()');
  });
});

// ---------------------------------------------------------------------------
// Selector computation
// ---------------------------------------------------------------------------

describe('computeSelector', () => {
  it('computes ERC-20 transfer selector', () => {
    expect(computeSelector('transfer(address _to, uint256 _value)')).toBe('a9059cbb');
  });

  it('computes ERC-20 approve selector', () => {
    expect(computeSelector('approve(address _spender, uint256 _value)')).toBe('095ea7b3');
  });

  it('computes transferFrom selector', () => {
    expect(computeSelector('transferFrom(address _from, address _to, uint256 _tokenId)')).toBe('23b872dd');
  });

  it('computes setApprovalForAll selector', () => {
    expect(computeSelector('setApprovalForAll(address _operator, bool _approved)')).toBe('a22cb465');
  });
});

// ---------------------------------------------------------------------------
// matchSelector
// ---------------------------------------------------------------------------

describe('matchSelector', () => {
  const sigs = [
    'transfer(address _to, uint256 _value)',
    'approve(address _spender, uint256 _value)',
    'transferFrom(address _from, address _to, uint256 _value)',
  ];

  it('matches transfer', () => {
    const data = '0xa9059cbb' + '0'.repeat(128);
    expect(matchSelector(data, sigs)).toBe('transfer(address _to, uint256 _value)');
  });

  it('matches approve', () => {
    const data = '0x095ea7b3' + '0'.repeat(128);
    expect(matchSelector(data, sigs)).toBe('approve(address _spender, uint256 _value)');
  });

  it('returns null for unknown selector', () => {
    const data = '0xdeadbeef' + '0'.repeat(128);
    expect(matchSelector(data, sigs)).toBeNull();
  });

  it('returns null for short data', () => {
    expect(matchSelector('0xab', sigs)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// decodeCalldata
// ---------------------------------------------------------------------------

describe('decodeCalldata', () => {
  it('decodes ERC-20 transfer', () => {
    const calldata =
      '0xa9059cbb' +
      '000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045' +
      '000000000000000000000000000000000000000000000000000000003b9aca00';

    const result = decodeCalldata(calldata, 'transfer(address _to, uint256 _value)');
    expect(result).not.toBeNull();
    expect(result!._to).toBe('0xd8da6bf26964af9d7eed9e03e53415d37aa96045');
    expect(result!._value).toBe(1000000000n);
  });

  it('decodes ERC-20 approve', () => {
    const calldata =
      '0x095ea7b3' +
      '000000000000000000000000111111125421ca6dc452d289314280a0f8842a65' +
      'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff';

    const result = decodeCalldata(calldata, 'approve(address _spender, uint256 _value)');
    expect(result).not.toBeNull();
    expect(result!._spender).toBe('0x111111125421ca6dc452d289314280a0f8842a65');
    // Max uint256
    expect(result!._value).toBe(2n ** 256n - 1n);
  });

  it('decodes transferFrom with 3 params', () => {
    const calldata =
      '0x23b872dd' +
      '000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' +
      '000000000000000000000000bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' +
      '0000000000000000000000000000000000000000000000000000000000000064'; // 100

    const result = decodeCalldata(calldata, 'transferFrom(address _from, address _to, uint256 _tokenId)');
    expect(result).not.toBeNull();
    expect(result!._from).toBe('0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
    expect(result!._to).toBe('0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb');
    expect(result!._tokenId).toBe(100n);
  });

  it('decodes bool param', () => {
    const calldata =
      '0xa22cb465' +
      '0000000000000000000000001e0049783f008a0085193e00003d00cd54003c71' +
      '0000000000000000000000000000000000000000000000000000000000000001';

    const result = decodeCalldata(calldata, 'setApprovalForAll(address _operator, bool _approved)');
    expect(result).not.toBeNull();
    expect(result!._approved).toBe(true);
  });

  it('returns null for wrong selector', () => {
    const calldata = '0xdeadbeef' + '0'.repeat(128);
    const result = decodeCalldata(calldata, 'transfer(address _to, uint256 _value)');
    expect(result).toBeNull();
  });

  it('returns null for empty calldata', () => {
    expect(decodeCalldata('0x', 'transfer(address,uint256)')).toBeNull();
  });

  it('decodes selector-only calldata with zero-padded params', () => {
    // ABI decoder pads missing data with zeros — this is by design
    const result = decodeCalldata('0xa9059cbb', 'transfer(address _to, uint256 _value)');
    expect(result).not.toBeNull();
    expect(result!._to).toBe('0x0000000000000000000000000000000000000000');
    expect(result!._value).toBe(0n);
  });

  it('decodes zero value', () => {
    const calldata =
      '0xa9059cbb' +
      '000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045' +
      '0000000000000000000000000000000000000000000000000000000000000000';

    const result = decodeCalldata(calldata, 'transfer(address _to, uint256 _value)');
    expect(result).not.toBeNull();
    expect(result!._value).toBe(0n);
  });
});

// ---------------------------------------------------------------------------
// Nested tuples & dynamic offsets (the baseOffset fix)
// ---------------------------------------------------------------------------

describe('nested tuples + dynamic offsets', () => {
  const wNum = (n: bigint | number) => BigInt(n).toString(16).padStart(64, '0');
  const wAddr = (a: string) => a.replace(/^0x/, '').toLowerCase().padStart(64, '0');
  const wBytes = (hexNo0x: string) => wNum(hexNo0x.length / 2) + hexNo0x.padEnd(Math.ceil(hexNo0x.length / 64) * 64, '0');
  const sel = (sig: string) => '0x' + computeSelector(sig);
  const A = '0xd8da6bf26964af9d7eed9e03e53415d37aa96045';

  it('decodes a STATIC tuple in place', () => {
    const sig = 'register((address owner,uint256 id) info)';
    const cd = sel(sig) + wAddr(A) + wNum(7n);
    const d = decodeCalldata(cd, sig)! as any;
    expect(d.info.owner).toBe(A);
    expect(d.info.id).toBe(7n);
  });

  it('decodes a tuple with a DYNAMIC field (Uniswap exactInput shape)', () => {
    // The pre-fix decoder resolved the tuple\'s inner offsets from byte 0 instead
    // of the tuple\'s start, yielding garbage for recipient/amounts/path.
    const sig = 'exactInput((bytes path,address recipient,uint256 amountIn,uint256 amountOutMinimum) params)';
    const path = 'aabbccddeeff';
    const data =
      wNum(0x20n) +        // top-level: offset to the (dynamic) tuple
      wNum(0x80n) +        // tuple.path offset, RELATIVE to the tuple start
      wAddr(A) +           // tuple.recipient
      wNum(1_000_000n) +   // tuple.amountIn
      wNum(999n) +         // tuple.amountOutMinimum
      wBytes(path);        // tuple.path tail
    const d = decodeCalldata(sel(sig) + data, sig)! as any;
    expect(d.params.recipient).toBe(A);
    expect(d.params.amountIn).toBe(1_000_000n);
    expect(d.params.amountOutMinimum).toBe(999n);
    expect(d.params.path).toBe('0x' + path);
  });

  it('decodes a dynamic address[] array', () => {
    const sig = 'route(address[] hops)';
    const B = '0x1111111111111111111111111111111111111111';
    const cd = sel(sig) + wNum(0x20n) + wNum(2n) + wAddr(A) + wAddr(B);
    const d = decodeCalldata(cd, sig)! as any;
    expect(d.hops).toEqual([A, B]);
  });

  it('decodes a string (dynamic) param', () => {
    const sig = 'greet(string name)';
    const cd = sel(sig) + wNum(0x20n) + wBytes(Buffer.from('hello', 'utf8').toString('hex'));
    const d = decodeCalldata(cd, sig)! as any;
    expect(d.name).toBe('hello');
  });
});
