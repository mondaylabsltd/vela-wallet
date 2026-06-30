/**
 * Hex encoding/decoding utilities.
 * Equivalent to iOS Data+Hex extensions.
 */

/** Convert a Uint8Array to hex string (lowercase, no 0x prefix). */
export function toHex(data: Uint8Array): string {
  let hex = '';
  for (let i = 0; i < data.length; i++) {
    hex += data[i].toString(16).padStart(2, '0');
  }
  return hex;
}

/** Convert hex string (with or without 0x prefix) to Uint8Array. */
export function fromHex(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (clean.length % 2 !== 0) throw new Error('Invalid hex string length');
  const arr = new Uint8Array(clean.length / 2);
  for (let i = 0; i < arr.length; i++) {
    arr[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return arr;
}

/** Add 0x prefix if not present. */
export function addHexPrefix(hex: string): string {
  return hex.startsWith('0x') ? hex : `0x${hex}`;
}

/** Remove 0x prefix if present. */
export function stripHexPrefix(hex: string): string {
  return hex.startsWith('0x') ? hex.slice(2) : hex;
}

/**
 * Normalise a value to a canonical JSON-RPC QUANTITY: `0x`-prefixed, lowercase,
 * no leading zeros (`0x0` for zero).
 *
 * go-ethereum's `hexutil.Big` REJECTS leading-zero quantities with
 * "cannot unmarshal hex number with leading zero digits". dApps routinely emit
 * them — ethers v5's `BigNumber.toHexString()` zero-pads to an even length
 * (`0x0de0b6b3a7640000`) — so any tx `value`/`gas`/`nonce` forwarded from a dApp
 * MUST pass through here before it reaches `eth_call` / `eth_simulateV1` /
 * `eth_estimateGas`. Parsing via BigInt also canonicalises decimal-string and
 * numeric inputs. Unparseable/empty/negative → `0x0`.
 */
export function toQuantity(value: string | number | bigint | undefined | null): string {
  if (value === undefined || value === null || value === '' || value === '0x') return '0x0';
  try {
    const n = typeof value === 'bigint' ? value : BigInt(value);
    return n > 0n ? '0x' + n.toString(16) : '0x0';
  } catch {
    return '0x0';
  }
}

/** Concatenate multiple Uint8Arrays. */
export function concatBytes(...arrays: Uint8Array[]): Uint8Array {
  const totalLength = arrays.reduce((sum, arr) => sum + arr.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const arr of arrays) {
    result.set(arr, offset);
    offset += arr.length;
  }
  return result;
}

/** Base64url encode (no padding). */
export function toBase64Url(data: Uint8Array): string {
  // Convert Uint8Array to binary string
  let binary = '';
  for (let i = 0; i < data.length; i++) {
    binary += String.fromCharCode(data[i]);
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** Base64url decode (no padding). */
export function fromBase64Url(b64url: string): Uint8Array {
  let b64 = b64url.replace(/-/g, '+').replace(/_/g, '/');
  while (b64.length % 4) b64 += '=';
  const binary = atob(b64);
  const arr = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    arr[i] = binary.charCodeAt(i);
  }
  return arr;
}
