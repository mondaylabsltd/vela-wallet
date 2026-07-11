/**
 * Tests for EIP-712 Typed Data Hashing.
 * Validates against the example from EIP-712 specification.
 */
import { hashTypedData, type TypedData } from '@/services/eip712';
import { toHex } from '@/services/hex';

describe('EIP-712', () => {
  // The canonical EIP-712 example from the spec
  const mailTypedData: TypedData = {
    types: {
      EIP712Domain: [
        { name: 'name', type: 'string' },
        { name: 'version', type: 'string' },
        { name: 'chainId', type: 'uint256' },
        { name: 'verifyingContract', type: 'address' },
      ],
      Person: [
        { name: 'name', type: 'string' },
        { name: 'wallet', type: 'address' },
      ],
      Mail: [
        { name: 'from', type: 'Person' },
        { name: 'to', type: 'Person' },
        { name: 'contents', type: 'string' },
      ],
    },
    primaryType: 'Mail',
    domain: {
      name: 'Ether Mail',
      version: '1',
      chainId: 1,
      verifyingContract: '0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC',
    },
    message: {
      from: { name: 'Cow', wallet: '0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826' },
      to: { name: 'Bob', wallet: '0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB' },
      contents: 'Hello, Bob!',
    },
  };

  test('produces correct hash for EIP-712 Mail example', () => {
    const hash = hashTypedData(mailTypedData);
    // Known correct hash from EIP-712 spec / ethers.js / viem
    expect(toHex(hash)).toBe(
      'be609aee343fb3c4b28e1df9e632fca64fcfaede20f02e86244efddf30957bd2'
    );
  });

  // Issue #83: dApps (viem/ethers/MetaMask) routinely omit EIP712Domain from
  // `types` because it's derivable from the domain fields present. The hasher
  // used to throw "Unknown type: EIP712Domain".
  test('derives EIP712Domain when the dApp omits it — matches the spec hash', () => {
    const { EIP712Domain, ...typesWithoutDomain } = mailTypedData.types;
    expect(EIP712Domain).toBeDefined(); // (sanity: we actually removed something)
    const withoutDomain: TypedData = { ...mailTypedData, types: typesWithoutDomain };
    // The derived domain type must reproduce the exact spec hash, proving the
    // derivation matches what the dApp's own verifier computes.
    expect(toHex(hashTypedData(withoutDomain))).toBe(
      'be609aee343fb3c4b28e1df9e632fca64fcfaede20f02e86244efddf30957bd2'
    );
  });

  test('omitting EIP712Domain equals supplying it explicitly (any domain shape)', () => {
    const permitLike: TypedData = {
      types: {
        Mail: [{ name: 'contents', type: 'string' }],
      },
      primaryType: 'Mail',
      domain: { name: 'biubiu', version: '1', chainId: 8453 }, // no verifyingContract/salt
      message: { contents: 'gm' },
    };
    const explicit: TypedData = {
      ...permitLike,
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
          { name: 'version', type: 'string' },
          { name: 'chainId', type: 'uint256' },
        ],
        Mail: permitLike.types.Mail,
      },
    };
    expect(() => hashTypedData(permitLike)).not.toThrow();
    expect(toHex(hashTypedData(permitLike))).toBe(toHex(hashTypedData(explicit)));
  });

  test('supplying EIP712Domain explicitly is unchanged (no derivation)', () => {
    // The canonical example already supplies it; assert the derivation path is a
    // no-op by comparing against a deep copy that also supplies it.
    const copy: TypedData = JSON.parse(JSON.stringify(mailTypedData));
    expect(toHex(hashTypedData(copy))).toBe(toHex(hashTypedData(mailTypedData)));
  });

  test('handles typed data with only domain', () => {
    const simple: TypedData = {
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
        ],
        Simple: [
          { name: 'value', type: 'uint256' },
        ],
      },
      primaryType: 'Simple',
      domain: { name: 'Test' },
      message: { value: 42 },
    };
    const hash = hashTypedData(simple);
    expect(hash).toBeInstanceOf(Uint8Array);
    expect(hash.length).toBe(32);
  });

  test('handles bool, bytes32, and address types', () => {
    const data: TypedData = {
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
        ],
        Test: [
          { name: 'active', type: 'bool' },
          { name: 'hash', type: 'bytes32' },
          { name: 'addr', type: 'address' },
        ],
      },
      primaryType: 'Test',
      domain: { name: 'Test' },
      message: {
        active: true,
        hash: '0x0000000000000000000000000000000000000000000000000000000000000001',
        addr: '0x0000000000000000000000000000000000000001',
      },
    };
    const hash = hashTypedData(data);
    expect(hash.length).toBe(32);
  });

  test('handles bytes and string dynamic types', () => {
    const data: TypedData = {
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
        ],
        Doc: [
          { name: 'title', type: 'string' },
          { name: 'content', type: 'bytes' },
        ],
      },
      primaryType: 'Doc',
      domain: { name: 'Test' },
      message: {
        title: 'Hello',
        content: '0xdeadbeef',
      },
    };
    const hash = hashTypedData(data);
    expect(hash.length).toBe(32);
  });

  test('handles array types', () => {
    const data: TypedData = {
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
        ],
        Batch: [
          { name: 'values', type: 'uint256[]' },
        ],
      },
      primaryType: 'Batch',
      domain: { name: 'Test' },
      message: {
        values: [1, 2, 3],
      },
    };
    const hash = hashTypedData(data);
    expect(hash.length).toBe(32);
  });

  test('deterministic: same input always produces same hash', () => {
    const h1 = hashTypedData(mailTypedData);
    const h2 = hashTypedData(mailTypedData);
    expect(toHex(h1)).toBe(toHex(h2));
  });

  test('different messages produce different hashes', () => {
    const modified = {
      ...mailTypedData,
      message: {
        ...mailTypedData.message,
        contents: 'Different message',
      },
    };
    const h1 = hashTypedData(mailTypedData);
    const h2 = hashTypedData(modified);
    expect(toHex(h1)).not.toBe(toHex(h2));
  });

  test('different domains produce different hashes', () => {
    const modified = {
      ...mailTypedData,
      domain: { ...mailTypedData.domain, chainId: 5 },
    };
    const h1 = hashTypedData(mailTypedData);
    const h2 = hashTypedData(modified);
    expect(toHex(h1)).not.toBe(toHex(h2));
  });

  test('handles nested struct types with sorted dependencies', () => {
    const data: TypedData = {
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
        ],
        Order: [
          { name: 'maker', type: 'Identity' },
          { name: 'amount', type: 'uint256' },
        ],
        Identity: [
          { name: 'name', type: 'string' },
          { name: 'wallet', type: 'address' },
        ],
      },
      primaryType: 'Order',
      domain: { name: 'Exchange' },
      message: {
        maker: { name: 'Alice', wallet: '0x0000000000000000000000000000000000000001' },
        amount: 100,
      },
    };
    const hash = hashTypedData(data);
    expect(hash.length).toBe(32);
  });

  test('handles int256 negative values', () => {
    const data: TypedData = {
      types: {
        EIP712Domain: [{ name: 'name', type: 'string' }],
        Signed: [{ name: 'val', type: 'int256' }],
      },
      primaryType: 'Signed',
      domain: { name: 'Test' },
      message: { val: -1 },
    };
    const hash = hashTypedData(data);
    expect(hash.length).toBe(32);
  });

  test('throws on unknown type', () => {
    const data: TypedData = {
      types: {
        EIP712Domain: [{ name: 'name', type: 'string' }],
        Bad: [{ name: 'val', type: 'tuple' }],
      },
      primaryType: 'Bad',
      domain: { name: 'Test' },
      message: { val: 'something' },
    };
    expect(() => hashTypedData(data)).toThrow('Unsupported EIP-712 type');
  });

  test('circular type references do not cause infinite loop', () => {
    // TypeA references TypeB and TypeB references TypeA.
    // The type resolver (findTypeDependencies) must not infinitely recurse.
    // Data encoding may fail on null sub-structs, but the key assertion is
    // that this terminates promptly rather than hanging.
    const circular: TypedData = {
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
        ],
        TypeA: [
          { name: 'value', type: 'uint256' },
          { name: 'b', type: 'TypeB' },
        ],
        TypeB: [
          { name: 'value', type: 'uint256' },
          { name: 'a', type: 'TypeA' },
        ],
      },
      primaryType: 'TypeA',
      domain: { name: 'Test' },
      message: {
        value: 1,
        b: {
          value: 2,
          a: { value: 3, b: { value: 4, a: { value: 5, b: { value: 6, a: { value: 7, b: { value: 8, a: {} } } } } } },
        },
      },
    };

    // Should terminate (not hang). It may throw during data encoding because
    // the innermost nested struct has missing fields, but it must not loop forever.
    try {
      const hash = hashTypedData(circular);
      expect(hash).toBeInstanceOf(Uint8Array);
      expect(hash.length).toBe(32);
    } catch (e: any) {
      // Acceptable — encoding deeply-nested circular data may fail,
      // but the important thing is it didn't infinite-loop.
      expect(e.message).not.toMatch(/Maximum call stack size exceeded/);
    }
  });

  test('self-referencing type does not cause infinite loop', () => {
    // Node references itself.
    const selfRef: TypedData = {
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
        ],
        Node: [
          { name: 'value', type: 'uint256' },
          { name: 'child', type: 'Node' },
        ],
      },
      primaryType: 'Node',
      domain: { name: 'Test' },
      message: {
        value: 1,
        child: { value: 2, child: { value: 3, child: { value: 4, child: {} } } },
      },
    };

    try {
      const hash = hashTypedData(selfRef);
      expect(hash).toBeInstanceOf(Uint8Array);
      expect(hash.length).toBe(32);
    } catch (e: any) {
      expect(e.message).not.toMatch(/Maximum call stack size exceeded/);
    }
  });

  // Permit2 style typed data (common DeFi pattern)
  test('handles Permit2-style typed data', () => {
    const permit2: TypedData = {
      types: {
        EIP712Domain: [
          { name: 'name', type: 'string' },
          { name: 'chainId', type: 'uint256' },
          { name: 'verifyingContract', type: 'address' },
        ],
        PermitSingle: [
          { name: 'details', type: 'PermitDetails' },
          { name: 'spender', type: 'address' },
          { name: 'sigDeadline', type: 'uint256' },
        ],
        PermitDetails: [
          { name: 'token', type: 'address' },
          { name: 'amount', type: 'uint160' },
          { name: 'expiration', type: 'uint48' },
          { name: 'nonce', type: 'uint48' },
        ],
      },
      primaryType: 'PermitSingle',
      domain: {
        name: 'Permit2',
        chainId: 1,
        verifyingContract: '0x000000000022D473030F116dDEE9F6B43aC78BA3',
      },
      message: {
        details: {
          token: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
          amount: '1461501637330902918203684832716283019655932542975',
          expiration: 1717200000,
          nonce: 0,
        },
        spender: '0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD',
        sigDeadline: 1717200000,
      },
    };
    const hash = hashTypedData(permit2);
    expect(hash.length).toBe(32);
  });
});
