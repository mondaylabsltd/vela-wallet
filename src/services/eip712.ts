/**
 * EIP-712 Typed Data Hashing.
 *
 * Implements the structured data hashing per https://eips.ethereum.org/EIPS/eip-712
 * Used by handleSignTypedData to produce the correct hash for on-chain verification.
 */

import { keccak256 } from './eth-crypto';
import { fromHex, stripHexPrefix, concatBytes } from './hex';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** EIP-712 typed data as received from DApps. */
export interface TypedData {
  types: Record<string, TypedDataField[]>;
  primaryType: string;
  domain: Record<string, any>;
  message: Record<string, any>;
}

export interface TypedDataField {
  name: string;
  type: string;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Compute the EIP-712 hash for typed data.
 * Returns the 32-byte hash: keccak256("\x19\x01" || domainSeparator || structHash)
 */
export function hashTypedData(typedData: TypedData): Uint8Array {
  const types = ensureDomainType(typedData);
  const domainSeparator = hashStruct('EIP712Domain', typedData.domain, types);
  const messageHash = hashStruct(typedData.primaryType, typedData.message, types);

  return keccak256(concatBytes(
    new Uint8Array([0x19, 0x01]),
    domainSeparator,
    messageHash,
  ));
}

/**
 * Canonical EIP712Domain fields, in the order EIP-712 mandates. A field belongs
 * to the domain type iff it is actually present in `domain`.
 */
const DOMAIN_FIELDS: TypedDataField[] = [
  { name: 'name', type: 'string' },
  { name: 'version', type: 'string' },
  { name: 'chainId', type: 'uint256' },
  { name: 'verifyingContract', type: 'address' },
  { name: 'salt', type: 'bytes32' },
];

/**
 * Ensure the types map has an `EIP712Domain` entry.
 *
 * Our hasher looks the domain type up by name, but valid payloads routinely omit
 * `EIP712Domain` from `types` because it's derivable from the domain fields
 * present — that's what viem, ethers and MetaMask do, and what biubiu sends
 * (issue #83: signing threw "Unknown type: EIP712Domain"). When it's absent,
 * synthesize it from the domain fields actually present, in EIP-712 canonical
 * order, so the derived typeHash matches what the dApp's verifier expects. If the
 * dApp already supplied `EIP712Domain`, the types are returned untouched
 * (byte-identical behavior — no change for those payloads).
 */
function ensureDomainType(typedData: TypedData): Record<string, TypedDataField[]> {
  if (typedData.types.EIP712Domain) return typedData.types;
  const domain = typedData.domain ?? {};
  const domainType = DOMAIN_FIELDS.filter((f) => domain[f.name] != null);
  return { ...typedData.types, EIP712Domain: domainType };
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

/**
 * Hash a struct: keccak256(typeHash || encodeData)
 */
function hashStruct(
  primaryType: string,
  data: Record<string, any>,
  types: Record<string, TypedDataField[]>,
): Uint8Array {
  const typeHash = hashType(primaryType, types);
  const encodedData = encodeData(primaryType, data, types);
  return keccak256(concatBytes(typeHash, encodedData));
}

/**
 * Compute the type hash: keccak256(encodeType(primaryType))
 */
function hashType(
  primaryType: string,
  types: Record<string, TypedDataField[]>,
): Uint8Array {
  const encoded = encodeType(primaryType, types);
  return keccak256(new TextEncoder().encode(encoded));
}

/**
 * Encode a type string including referenced types (sorted, deduplicated).
 * e.g., "Mail(Person from,Person to,string contents)Person(string name,address wallet)"
 */
function encodeType(
  primaryType: string,
  types: Record<string, TypedDataField[]>,
): string {
  const deps = findTypeDependencies(primaryType, types);
  // Remove primary type, sort the rest, then prepend primary
  deps.delete(primaryType);
  const sorted = [primaryType, ...Array.from(deps).sort()];

  return sorted.map(type => {
    const fields = types[type];
    if (!fields) throw new Error(`Unknown type: ${type}`);
    const fieldStr = fields.map(f => `${f.type} ${f.name}`).join(',');
    return `${type}(${fieldStr})`;
  }).join('');
}

/** Maximum recursion depth when resolving type dependencies. */
const MAX_TYPE_DEPTH = 256;

/**
 * Find all custom type dependencies for a given type.
 * Uses a visited set (`deps`) to handle circular references and a depth limit
 * as a secondary safety net.
 */
function findTypeDependencies(
  type: string,
  types: Record<string, TypedDataField[]>,
  deps: Set<string> = new Set(),
  depth: number = 0,
): Set<string> {
  if (depth > MAX_TYPE_DEPTH) {
    throw new Error(
      `EIP-712: maximum type depth exceeded (${MAX_TYPE_DEPTH}). Possible circular type reference.`,
    );
  }

  // Strip array suffix if present (e.g., "Person[]" -> "Person")
  const baseType = type.replace(/\[\d*\]$/, '');
  if (deps.has(baseType)) return deps;
  if (!types[baseType]) return deps;

  deps.add(baseType);
  for (const field of types[baseType]) {
    findTypeDependencies(field.type, types, deps, depth + 1);
  }
  return deps;
}

/**
 * ABI-encode the values of a struct according to EIP-712 rules.
 * Does NOT include the type hash (caller prepends it).
 */
function encodeData(
  primaryType: string,
  data: Record<string, any>,
  types: Record<string, TypedDataField[]>,
): Uint8Array {
  const fields = types[primaryType];
  if (!fields) throw new Error(`Unknown type: ${primaryType}`);

  const parts: Uint8Array[] = [];
  for (const field of fields) {
    const value = data[field.name];
    parts.push(encodeValue(field.type, value, types));
  }
  return concatBytes(...parts);
}

/**
 * Encode a single value per EIP-712 encoding rules.
 */
function encodeValue(
  type: string,
  value: any,
  types: Record<string, TypedDataField[]>,
): Uint8Array {
  // Struct type → hashStruct
  if (types[type]) {
    return hashStruct(type, value, types);
  }

  // Dynamic bytes → keccak256(value)
  if (type === 'bytes') {
    const bytes = typeof value === 'string' ? fromHex(stripHexPrefix(value)) : value;
    return keccak256(bytes);
  }

  // String → keccak256(utf8 bytes)
  if (type === 'string') {
    return keccak256(new TextEncoder().encode(value));
  }

  // Array type → keccak256(concat(encoded elements))
  const arrayMatch = type.match(/^(.+?)(\[\d*\])$/);
  if (arrayMatch) {
    const baseType = arrayMatch[1];
    const arr = value as any[];
    const encoded = arr.map(v => encodeValue(baseType, v, types));
    return keccak256(concatBytes(...encoded));
  }

  // address → left-padded to 32 bytes
  if (type === 'address') {
    return encodeAbiWord(addressToBytes(value));
  }

  // bool → uint256(0 or 1)
  if (type === 'bool') {
    const word = new Uint8Array(32);
    word[31] = value ? 1 : 0;
    return word;
  }

  // bytesN (fixed-size) → right-padded to 32 bytes
  const bytesMatch = type.match(/^bytes(\d+)$/);
  if (bytesMatch) {
    const bytes = typeof value === 'string' ? fromHex(stripHexPrefix(value)) : value;
    const word = new Uint8Array(32);
    word.set(bytes.slice(0, 32));
    return word;
  }

  // int/uint (all sizes) → padded to 32 bytes big-endian
  if (type.startsWith('uint') || type.startsWith('int')) {
    return uint256ToBytes(BigInt(value));
  }

  throw new Error(`Unsupported EIP-712 type: ${type}`);
}

// ---------------------------------------------------------------------------
// Encoding helpers
// ---------------------------------------------------------------------------

function addressToBytes(address: string): Uint8Array {
  const clean = stripHexPrefix(address).toLowerCase();
  return fromHex(clean.padStart(40, '0'));
}

function encodeAbiWord(data: Uint8Array): Uint8Array {
  const word = new Uint8Array(32);
  // Left-pad: place data at the end
  word.set(data, 32 - data.length);
  return word;
}

function uint256ToBytes(value: bigint): Uint8Array {
  const word = new Uint8Array(32);
  let v = value < 0n ? (1n << 256n) + value : value; // two's complement for negative
  for (let i = 31; i >= 0; i--) {
    word[i] = Number(v & 0xFFn);
    v >>= 8n;
  }
  return word;
}
