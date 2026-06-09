/**
 * Parses WebAuthn attestation objects and ECDSA signatures.
 * Minimal CBOR parsing for the specific structure of attestation data.
 */

interface MutableIndex {
  value: number;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Parse a CBOR-encoded WebAuthn attestation object to extract the P256 public
 * key (x, y coordinates).
 *
 * Attestation object structure: { fmt, attStmt, authData }
 * authData: rpIdHash(32) | flags(1) | signCount(4) | attestedCredData
 * attestedCredData: aaguid(16) | credIdLen(2 BE) | credId(n) | coseKey(CBOR)
 * COSE key: { 1:2, 3:-7, -1:1, -2:x(32), -3:y(32) }
 */
export function extractPublicKey(
  attestationObject: Uint8Array,
): { x: Uint8Array; y: Uint8Array } | null {
  const authData = extractAuthData(attestationObject);
  if (!authData) return null;

  // authData minimum: rpIdHash(32) + flags(1) + signCount(4) = 37
  if (authData.length <= 37) return null;

  const flags = authData[32];
  const hasAttestedCredData = (flags & 0x40) !== 0;
  if (!hasAttestedCredData) return null;

  // Skip rpIdHash(32) + flags(1) + signCount(4) + aaguid(16) = 53
  if (authData.length <= 55) return null;
  const credIdLen = (authData[53] << 8) | authData[54];

  const coseKeyOffset = 55 + credIdLen;
  if (authData.length <= coseKeyOffset) return null;

  const coseKeyData = authData.subarray(coseKeyOffset);
  return extractP256FromCOSE(coseKeyData);
}

/**
 * Convert a DER-encoded ECDSA signature to raw r||s (64 bytes).
 * DER format: 30 <len> 02 <r_len> <r> 02 <s_len> <s>
 */
export function derSignatureToRaw(derSig: Uint8Array): Uint8Array | null {
  if (derSig.length <= 6 || derSig[0] !== 0x30) return null;

  let index = 2; // skip 30 <len>

  if (derSig[index] !== 0x02) return null;
  index += 1;
  const rLen = derSig[index];
  index += 1;

  if (index + rLen >= derSig.length) return null;
  let r = derSig.slice(index, index + rLen);
  index += rLen;

  if (index >= derSig.length || derSig[index] !== 0x02) return null;
  index += 1;
  const sLen = derSig[index];
  index += 1;

  if (index + sLen > derSig.length) return null;
  let s = derSig.slice(index, index + sLen);

  // Strip leading zero padding (DER uses signed integers)
  if (r.length === 33 && r[0] === 0x00) r = r.slice(1);
  if (s.length === 33 && s[0] === 0x00) s = s.slice(1);

  // Pad to 32 bytes if shorter
  if (r.length < 32) {
    const padded = new Uint8Array(32);
    padded.set(r, 32 - r.length);
    r = padded;
  }
  if (s.length < 32) {
    const padded = new Uint8Array(32);
    padded.set(s, 32 - s.length);
    s = padded;
  }

  // Normalize s to low-s form (s <= n/2) for RIP-7212 P256 precompile compatibility.
  // P256 curve order n = FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
  // @ts-ignore - Uint8Array slice buffer type mismatch
  s = normalizeP256S(s);

  const raw = new Uint8Array(64);
  raw.set(r, 0);
  raw.set(s, 32);
  return raw;
}

// P256 curve order n
const P256_N = BigInt('0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551');
const P256_HALF_N = P256_N >> 1n;

/**
 * Normalize P256 ECDSA s-value to low-s form (s <= n/2).
 * The RIP-7212 P256 precompile rejects signatures with s > n/2.
 */
function normalizeP256S(sBytes: Uint8Array): Uint8Array {
  let sVal = 0n;
  for (let i = 0; i < sBytes.length; i++) {
    sVal = (sVal << 8n) | BigInt(sBytes[i]);
  }
  if (sVal <= P256_HALF_N) return sBytes;
  sVal = P256_N - sVal;
  const out = new Uint8Array(32);
  let v = sVal;
  for (let i = 31; i >= 0; i--) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return out;
}

// ---------------------------------------------------------------------------
// Private: Minimal CBOR Parsing
// ---------------------------------------------------------------------------

/**
 * Extract the "authData" byte-string value from a CBOR-encoded attestation
 * object map.
 */
function extractAuthData(cbor: Uint8Array): Uint8Array | null {
  const i: MutableIndex = { value: 0 };

  // Expect a CBOR map (major type 5)
  if (i.value >= cbor.length) return null;
  const major = cbor[i.value] >> 5;
  const additional = cbor[i.value] & 0x1f;
  if (major !== 5) return null;
  i.value += 1;

  let mapCount: number;
  if (additional < 24) {
    mapCount = additional;
  } else if (additional === 24) {
    if (i.value >= cbor.length) return null;
    mapCount = cbor[i.value];
    i.value += 1;
  } else {
    return null;
  }

  // Iterate map entries looking for "authData"
  for (let entry = 0; entry < mapCount; entry++) {
    if (i.value >= cbor.length) return null;

    // Read key (text string)
    const keyMajor = cbor[i.value] >> 5;
    const keyAdd = cbor[i.value] & 0x1f;
    i.value += 1;

    if (keyMajor === 3) {
      // text string
      let keyLen: number;
      if (keyAdd < 24) {
        keyLen = keyAdd;
      } else if (keyAdd === 24) {
        if (i.value >= cbor.length) return null;
        keyLen = cbor[i.value];
        i.value += 1;
      } else {
        return null;
      }

      if (i.value + keyLen > cbor.length) return null;
      const keyStr = new TextDecoder().decode(
        cbor.subarray(i.value, i.value + keyLen),
      );
      i.value += keyLen;

      if (keyStr === "authData") {
        // Read value (byte string)
        return readCBORByteString(cbor, i);
      } else {
        // Skip value
        const next = skipCBORValue(cbor, i.value);
        if (next === null) return null;
        i.value = next;
      }
    } else {
      // Skip non-string key and its value
      const afterKey = skipCBORValue(cbor, i.value - 1);
      if (afterKey === null) return null;
      const afterVal = skipCBORValue(cbor, afterKey);
      if (afterVal === null) return null;
      i.value = afterVal;
    }
  }

  return null;
}

/**
 * Extract x, y coordinates from a COSE key (CBOR map).
 */
function extractP256FromCOSE(
  data: Uint8Array,
): { x: Uint8Array; y: Uint8Array } | null {
  const i: MutableIndex = { value: 0 };

  if (i.value >= data.length) return null;
  const major = data[i.value] >> 5;
  const additional = data[i.value] & 0x1f;
  if (major !== 5) return null;
  i.value += 1;

  let mapCount: number;
  if (additional < 24) {
    mapCount = additional;
  } else if (additional === 24) {
    if (i.value >= data.length) return null;
    mapCount = data[i.value];
    i.value += 1;
  } else {
    return null;
  }

  let x: Uint8Array | null = null;
  let y: Uint8Array | null = null;

  for (let entry = 0; entry < mapCount; entry++) {
    if (i.value >= data.length) return null;

    // Read key (could be positive or negative integer)
    const keyVal = readCBORInt(data, i);

    // Read value
    if (i.value >= data.length) return null;

    if (keyVal === -2) {
      // x coordinate
      x = readCBORByteString(data, i);
    } else if (keyVal === -3) {
      // y coordinate
      y = readCBORByteString(data, i);
    } else {
      const next = skipCBORValue(data, i.value);
      if (next === null) return null;
      i.value = next;
    }
  }

  if (!x || !y || x.length !== 32 || y.length !== 32) return null;
  return { x, y };
}

/**
 * Read a CBOR integer (major type 0 = unsigned, major type 1 = negative).
 */
function readCBORInt(bytes: Uint8Array, i: MutableIndex): number {
  if (i.value >= bytes.length) return 0;
  const major = bytes[i.value] >> 5;
  const additional = bytes[i.value] & 0x1f;
  i.value += 1;

  let rawVal: number;
  if (additional < 24) {
    rawVal = additional;
  } else if (additional === 24) {
    if (i.value >= bytes.length) return 0;
    rawVal = bytes[i.value];
    i.value += 1;
  } else {
    return 0;
  }

  // Major type 1 = negative int (-1 - val)
  if (major === 1) {
    return -1 - rawVal;
  }
  return rawVal;
}

/**
 * Read a CBOR byte string (major type 2).
 */
function readCBORByteString(
  bytes: Uint8Array,
  i: MutableIndex,
): Uint8Array | null {
  if (i.value >= bytes.length) return null;
  const major = bytes[i.value] >> 5;
  const additional = bytes[i.value] & 0x1f;
  i.value += 1;

  if (major !== 2) return null;

  let len: number;
  if (additional < 24) {
    len = additional;
  } else if (additional === 24) {
    if (i.value >= bytes.length) return null;
    len = bytes[i.value];
    i.value += 1;
  } else if (additional === 25) {
    if (i.value + 1 >= bytes.length) return null;
    len = (bytes[i.value] << 8) | bytes[i.value + 1];
    i.value += 2;
  } else {
    return null;
  }

  if (i.value + len > bytes.length) return null;
  const data = bytes.slice(i.value, i.value + len);
  i.value += len;
  return data;
}

/**
 * Skip over a CBOR value and return the new index, or null on error.
 */
function skipCBORValue(bytes: Uint8Array, index: number): number | null {
  let i = index;
  if (i >= bytes.length) return null;

  const major = bytes[i] >> 5;
  const additional = bytes[i] & 0x1f;
  i += 1;

  let val: number;
  if (additional < 24) {
    val = additional;
  } else if (additional === 24) {
    if (i >= bytes.length) return null;
    val = bytes[i];
    i += 1;
  } else if (additional === 25) {
    if (i + 1 >= bytes.length) return null;
    val = (bytes[i] << 8) | bytes[i + 1];
    i += 2;
  } else if (additional === 26) {
    if (i + 3 >= bytes.length) return null;
    val =
      (bytes[i] << 24) |
      (bytes[i + 1] << 16) |
      (bytes[i + 2] << 8) |
      bytes[i + 3];
    i += 4;
  } else {
    return null;
  }

  switch (major) {
    case 0:
    case 1:
      return i; // integer
    case 2:
    case 3:
      return i + val; // byte/text string
    case 4: // array
      for (let n = 0; n < val; n++) {
        const next = skipCBORValue(bytes, i);
        if (next === null) return null;
        i = next;
      }
      return i;
    case 5: // map
      for (let n = 0; n < val; n++) {
        const nextKey = skipCBORValue(bytes, i);
        if (nextKey === null) return null;
        const nextVal = skipCBORValue(bytes, nextKey);
        if (nextVal === null) return null;
        i = nextVal;
      }
      return i;
    case 7:
      return i; // simple/float
    default:
      return null;
  }
}
