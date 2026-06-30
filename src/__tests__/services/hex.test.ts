/**
 * Tests for hex encoding/decoding utilities.
 */
import { toHex, fromHex, addHexPrefix, stripHexPrefix, concatBytes, toBase64Url, fromBase64Url, toQuantity } from '@/services/hex';

describe('toHex', () => {
  test('empty array → empty string', () => {
    expect(toHex(new Uint8Array(0))).toBe('');
  });

  test('single byte', () => {
    expect(toHex(new Uint8Array([0xff]))).toBe('ff');
    expect(toHex(new Uint8Array([0x00]))).toBe('00');
    expect(toHex(new Uint8Array([0x0a]))).toBe('0a');
  });

  test('multiple bytes', () => {
    expect(toHex(new Uint8Array([0xde, 0xad, 0xbe, 0xef]))).toBe('deadbeef');
  });
});

describe('fromHex', () => {
  test('empty string → empty array', () => {
    expect(fromHex('')).toEqual(new Uint8Array(0));
  });

  test('strips 0x prefix', () => {
    const result = fromHex('0xdeadbeef');
    expect(toHex(result)).toBe('deadbeef');
  });

  test('handles uppercase', () => {
    const result = fromHex('DEADBEEF');
    expect(toHex(result)).toBe('deadbeef');
  });

  test('roundtrips with toHex', () => {
    const original = new Uint8Array([1, 2, 3, 255, 0, 128]);
    const hex = toHex(original);
    const decoded = fromHex(hex);
    expect(decoded).toEqual(original);
  });

  test('throws on odd-length string', () => {
    expect(() => fromHex('abc')).toThrow();
  });
});

describe('addHexPrefix / stripHexPrefix', () => {
  test('addHexPrefix adds 0x', () => {
    expect(addHexPrefix('dead')).toBe('0xdead');
  });

  test('addHexPrefix is idempotent', () => {
    expect(addHexPrefix('0xdead')).toBe('0xdead');
  });

  test('stripHexPrefix removes 0x', () => {
    expect(stripHexPrefix('0xdead')).toBe('dead');
  });

  test('stripHexPrefix is idempotent', () => {
    expect(stripHexPrefix('dead')).toBe('dead');
  });
});

describe('concatBytes', () => {
  test('concatenates arrays', () => {
    const a = new Uint8Array([1, 2]);
    const b = new Uint8Array([3, 4]);
    const c = new Uint8Array([5]);
    expect(concatBytes(a, b, c)).toEqual(new Uint8Array([1, 2, 3, 4, 5]));
  });

  test('handles empty arrays', () => {
    const a = new Uint8Array(0);
    const b = new Uint8Array([1]);
    expect(concatBytes(a, b)).toEqual(new Uint8Array([1]));
  });
});

describe('toQuantity — canonical JSON-RPC quantity (no leading zeros)', () => {
  test('empty / nullish / 0x → 0x0', () => {
    expect(toQuantity(undefined)).toBe('0x0');
    expect(toQuantity(null)).toBe('0x0');
    expect(toQuantity('')).toBe('0x0');
    expect(toQuantity('0x')).toBe('0x0');
    expect(toQuantity('0x0')).toBe('0x0');
    expect(toQuantity('0x00')).toBe('0x0');
  });

  test('strips leading zeros from ethers-style padded hex', () => {
    // BigNumber.toHexString() pads to even length; go-ethereum rejects this form.
    expect(toQuantity('0x0de0b6b3a7640000')).toBe('0xde0b6b3a7640000');
    expect(toQuantity('0x0001')).toBe('0x1');
  });

  test('already-canonical hex is unchanged (lowercased)', () => {
    expect(toQuantity('0xde0b6b3a7640000')).toBe('0xde0b6b3a7640000');
    expect(toQuantity('0xFF')).toBe('0xff');
  });

  test('accepts decimal strings, numbers, and bigints', () => {
    expect(toQuantity('1000000000000000000')).toBe('0xde0b6b3a7640000');
    expect(toQuantity(255)).toBe('0xff');
    expect(toQuantity(1000000000000000000n)).toBe('0xde0b6b3a7640000');
  });

  test('garbage / negative → 0x0 (never throws)', () => {
    expect(toQuantity('not-hex')).toBe('0x0');
    expect(toQuantity(-5)).toBe('0x0');
  });
});

describe('base64url', () => {
  test('roundtrips correctly', () => {
    const original = new Uint8Array([0, 1, 2, 255, 254, 253]);
    const encoded = toBase64Url(original);
    const decoded = fromBase64Url(encoded);
    expect(decoded).toEqual(original);
  });

  test('no padding characters', () => {
    const encoded = toBase64Url(new Uint8Array([1]));
    expect(encoded).not.toContain('=');
  });

  test('uses URL-safe characters', () => {
    // Encode data that would normally produce + and /
    const data = new Uint8Array(256);
    for (let i = 0; i < 256; i++) data[i] = i;
    const encoded = toBase64Url(data);
    expect(encoded).not.toContain('+');
    expect(encoded).not.toContain('/');
  });
});
