/**
 * P-256 public key recovery from WebAuthn assertion signatures.
 *
 * An ECDSA signature determines its signing key up to a small candidate set
 * (≤4, almost always 2), and the candidate sets of two signatures intersect
 * in exactly one key — the same math as Ethereum's ecrecover, minus the
 * recovery-id hint. This gives the wallet a cryptographic escape hatch: if
 * the public key index is unreachable or lost AND the account isn't in local
 * storage, two passkey signatures rebuild the public key — and from it the
 * Safe address — entirely on-device. The index becomes a cache, not a single
 * point of failure.
 *
 * Security notes:
 * - Every input here is PUBLIC data (signatures, authenticator data, client
 *   data). No private material is handled, so variable-time BigInt math is
 *   acceptable. Do NOT reuse these routines for signing.
 * - The recovered key is verified against BOTH signatures before being
 *   returned, so a corrupt input can only yield null, never a wrong key.
 */

import { derSignatureToRaw } from './attestation-parser';
import { fromHex } from './hex';
import { sha256 } from './sha256';

// ---------------------------------------------------------------------------
// Curve parameters (secp256r1 / NIST P-256)
// ---------------------------------------------------------------------------

const P = BigInt('0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff');
const A = P - 3n;
const B = BigInt('0x5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b');
const N = BigInt('0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551');
const GX = BigInt('0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296');
const GY = BigInt('0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5');

// ---------------------------------------------------------------------------
// Field / point arithmetic (affine; null = point at infinity)
// ---------------------------------------------------------------------------

type Point = readonly [bigint, bigint] | null;

const mod = (a: bigint, m: bigint): bigint => ((a % m) + m) % m;

function modPow(base: bigint, exp: bigint, m: bigint): bigint {
  let result = 1n;
  base = mod(base, m);
  while (exp > 0n) {
    if (exp & 1n) result = (result * base) % m;
    base = (base * base) % m;
    exp >>= 1n;
  }
  return result;
}

/** Modular inverse via Fermat's little theorem (both moduli here are prime). */
const modInv = (a: bigint, m: bigint): bigint => modPow(mod(a, m), m - 2n, m);

/** Square root mod P — valid because P ≡ 3 (mod 4). */
const sqrtP = (a: bigint): bigint => modPow(a, (P + 1n) / 4n, P);

function pointAdd(p1: Point, p2: Point): Point {
  if (!p1) return p2;
  if (!p2) return p1;
  const [x1, y1] = p1;
  const [x2, y2] = p2;
  if (x1 === x2 && mod(y1 + y2, P) === 0n) return null;
  const lambda = x1 === x2 && y1 === y2
    ? mod((3n * x1 * x1 + A) * modInv(2n * y1, P), P)
    : mod((y2 - y1) * modInv(x2 - x1, P), P);
  const x3 = mod(lambda * lambda - x1 - x2, P);
  return [x3, mod(lambda * (x1 - x3) - y1, P)];
}

function pointMul(k: bigint, pt: Point): Point {
  let result: Point = null;
  let addend = pt;
  k = mod(k, N);
  while (k > 0n) {
    if (k & 1n) result = pointAdd(result, addend);
    addend = pointAdd(addend, addend);
    k >>= 1n;
  }
  return result;
}

/** Reconstruct the curve point with the given x and y parity, if one exists. */
function liftX(x: bigint, yOdd: 0 | 1): Point {
  if (x >= P) return null;
  const ySquared = mod(x * x * x + A * x + B, P);
  const y = sqrtP(ySquared);
  if ((y * y) % P !== ySquared) return null; // x is not on the curve
  return (y & 1n) === BigInt(yOdd) ? [x, y] : [x, P - y];
}

function verifySignature(q: Point, r: bigint, s: bigint, e: bigint): boolean {
  if (!q) return false;
  const sInv = modInv(s, N);
  const sum = pointAdd(
    pointMul(mod(e * sInv, N), [GX, GY]),
    pointMul(mod(r * sInv, N), q),
  );
  return sum !== null && mod(sum[0], N) === r;
}

// ---------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------

/** All candidate public keys that could have produced signature (r, s) over hash e. */
function recoverCandidates(r: bigint, s: bigint, e: bigint): [bigint, bigint][] {
  if (r <= 0n || r >= N || s <= 0n || s >= N) return [];
  const candidates: [bigint, bigint][] = [];
  // x ≡ r (mod n) allows x = r and (astronomically rarely, since p − n ≈ 2^224
  // is tiny next to p) x = r + n.
  for (const x of [r, r + N]) {
    for (const yOdd of [0, 1] as const) {
      const rPoint = liftX(x, yOdd);
      if (!rPoint) continue;
      const rInv = modInv(r, N);
      const q = pointAdd(
        pointMul(mod(s * rInv, N), rPoint),
        pointMul(mod(-e * rInv, N), [GX, GY]),
      );
      if (q && verifySignature(q, r, s, e)) candidates.push([q[0], q[1]]);
    }
  }
  return candidates;
}

/** The subset of WebAuthn assertion fields recovery needs (hex-encoded). */
export interface RecoverableAssertion {
  signatureHex: string;
  authenticatorDataHex: string;
  clientDataJSONHex: string;
}

const bytesToBig = (bytes: Uint8Array): bigint =>
  bytes.reduce((acc, byte) => (acc << 8n) | BigInt(byte), 0n);

/** e = sha256(authenticatorData || sha256(clientDataJSON)) — what WebAuthn signs. */
function assertionSigningHash(assertion: RecoverableAssertion): bigint {
  const authData = fromHex(assertion.authenticatorDataHex);
  const clientDataHash = sha256(fromHex(assertion.clientDataJSONHex));
  const message = new Uint8Array(authData.length + clientDataHash.length);
  message.set(authData);
  message.set(clientDataHash, authData.length);
  return bytesToBig(sha256(message));
}

function assertionToRs(assertion: RecoverableAssertion): { r: bigint; s: bigint } | null {
  const raw = derSignatureToRaw(fromHex(assertion.signatureHex));
  if (!raw || raw.length !== 64) return null;
  return { r: bytesToBig(raw.subarray(0, 32)), s: bytesToBig(raw.subarray(32, 64)) };
}

/**
 * Recover the uncompressed public key (`04 || x || y` hex) shared by two
 * WebAuthn assertions from the same credential, or null if the inputs don't
 * pin down exactly one key (malformed input, different credentials, or the
 * same signature passed twice — one signature alone is deliberately never
 * enough, its candidate set is ambiguous).
 */
export function recoverPublicKeyFromAssertions(
  first: RecoverableAssertion,
  second: RecoverableAssertion,
): string | null {
  const sig1 = assertionToRs(first);
  const sig2 = assertionToRs(second);
  if (!sig1 || !sig2) return null;
  if (sig1.r === sig2.r && sig1.s === sig2.s) return null; // same signature twice — ambiguous

  const candidates1 = recoverCandidates(sig1.r, sig1.s, assertionSigningHash(first));
  const candidates2 = recoverCandidates(sig2.r, sig2.s, assertionSigningHash(second));

  const shared = candidates1.filter(([x, y]) =>
    candidates2.some(([x2, y2]) => x === x2 && y === y2),
  );
  if (shared.length !== 1) return null;

  const [x, y] = shared[0];
  return '04' + x.toString(16).padStart(64, '0') + y.toString(16).padStart(64, '0');
}
