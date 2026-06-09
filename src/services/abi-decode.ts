/**
 * Generic ABI calldata decoder.
 *
 * Parses Solidity function signatures and decodes calldata into named parameters.
 * No external dependencies — builds on existing eth-crypto.ts for keccak256.
 */
import { functionSelector } from '@/services/eth-crypto';
import { toHex } from '@/services/hex';

// ---------------------------------------------------------------------------
// Signature parsing
// ---------------------------------------------------------------------------

export interface AbiParam {
  type: string;     // e.g. "address", "uint256", "(address,uint256)"
  name: string;     // e.g. "_to", "params"
  components?: AbiParam[]; // for tuple types
}

/**
 * Parse a Solidity function signature into name + params.
 * Input: "transfer(address _to, uint256 _value)"
 * Output: { name: "transfer", params: [{type:"address",name:"_to"}, {type:"uint256",name:"_value"}] }
 */
export function parseSignature(sig: string): { name: string; params: AbiParam[] } {
  const parenIdx = sig.indexOf('(');
  if (parenIdx === -1) return { name: sig, params: [] };
  const name = sig.slice(0, parenIdx);
  const body = sig.slice(parenIdx + 1, sig.lastIndexOf(')'));
  return { name, params: parseParamList(body) };
}

function parseParamList(body: string): AbiParam[] {
  if (!body.trim()) return [];
  const params: AbiParam[] = [];
  let depth = 0;
  let current = '';

  for (const ch of body) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ',' && depth === 0) {
      params.push(parseOneParam(current.trim()));
      current = '';
    } else {
      current += ch;
    }
  }
  if (current.trim()) params.push(parseOneParam(current.trim()));
  return params;
}

function parseOneParam(raw: string): AbiParam {
  // Handle tuple: "(address recipient, uint256 amount) params"
  if (raw.startsWith('(')) {
    const closeIdx = findMatchingParen(raw, 0);
    const tupleBody = raw.slice(1, closeIdx);
    const rest = raw.slice(closeIdx + 1).trim();
    // rest could be "[] name" or " name" or "name"
    let arrayStr = '';
    let name = '';
    if (rest.startsWith('[')) {
      const bIdx = rest.indexOf(']');
      arrayStr = rest.slice(0, bIdx + 1);
      name = rest.slice(bIdx + 1).trim();
    } else {
      name = rest.replace(/^\s+/, '');
    }
    const components = parseParamList(tupleBody);
    return { type: 'tuple' + arrayStr, name, components };
  }

  // Regular: "uint256 _value" or "address" or "bytes[]"
  const parts = raw.split(/\s+/);
  if (parts.length === 1) return { type: parts[0], name: '' };
  return { type: parts[0], name: parts.slice(1).join(' ') };
}

function findMatchingParen(s: string, start: number): number {
  let depth = 0;
  for (let i = start; i < s.length; i++) {
    if (s[i] === '(') depth++;
    if (s[i] === ')') { depth--; if (depth === 0) return i; }
  }
  return s.length - 1;
}

// ---------------------------------------------------------------------------
// Canonical form (for selector computation)
// ---------------------------------------------------------------------------

/**
 * Convert a Solidity-style signature to canonical form (no param names, no whitespace).
 * "transfer(address _to, uint256 _value)" → "transfer(address,uint256)"
 * "exactInput((bytes path, address recipient, uint256 amountIn, uint256 amountOutMinimum) params)"
 *   → "exactInput((bytes,address,uint256,uint256))"
 */
export function canonicalize(sig: string): string {
  const { name, params } = parseSignature(sig);
  return name + '(' + params.map(p => canonicalType(p)).join(',') + ')';
}

function canonicalType(p: AbiParam): string {
  if (p.type.startsWith('tuple')) {
    const suffix = p.type.slice(5); // "[]" or ""
    const inner = (p.components ?? []).map(c => canonicalType(c)).join(',');
    return '(' + inner + ')' + suffix;
  }
  return p.type;
}

/**
 * Compute the 4-byte function selector hex (no 0x prefix).
 */
export function computeSelector(sig: string): string {
  return toHex(functionSelector(canonicalize(sig)));
}

// ---------------------------------------------------------------------------
// ABI type classification
// ---------------------------------------------------------------------------

function isDynamic(type: string, components?: AbiParam[]): boolean {
  if (type === 'bytes' || type === 'string') return true;
  if (type.endsWith('[]')) return true;
  if (type.startsWith('tuple')) {
    if (type.endsWith('[]')) return true;
    return (components ?? []).some(c => isDynamic(c.type, c.components));
  }
  return false;
}

function staticSize(type: string): number {
  // All static ABI types are exactly 32 bytes (one word)
  return 32;
}

// ---------------------------------------------------------------------------
// Calldata decoding
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-redundant-type-constituents
export type DecodedValue = string | bigint | boolean | Array<string | bigint | boolean | Record<string, any>> | Record<string, any>;

/**
 * Decode calldata against a parsed function signature.
 * Returns a map of param name → decoded value.
 */
export function decodeCalldata(
  calldata: string,
  sig: string,
): Record<string, DecodedValue> | null {
  try {
    const hex = calldata.startsWith('0x') ? calldata.slice(2) : calldata;
    if (hex.length < 8) return null;

    const actualSel = hex.slice(0, 8).toLowerCase();
    const expectedSel = computeSelector(sig).toLowerCase();
    if (actualSel !== expectedSel) return null;

    const data = hex.slice(8); // skip selector
    const { params } = parseSignature(sig);
    return decodeTupleParams(data, params, 0).value as Record<string, DecodedValue>;
  } catch (e) {
    console.warn('[abi-decode] Failed:', e);
    return null;
  }
}

/**
 * Match calldata selector against a list of signatures.
 * Returns the matching signature or null.
 */
export function matchSelector(calldata: string, signatures: string[]): string | null {
  const hex = calldata.startsWith('0x') ? calldata.slice(2) : calldata;
  if (hex.length < 8) return null;
  const sel = hex.slice(0, 8).toLowerCase();

  for (const sig of signatures) {
    if (computeSelector(sig).toLowerCase() === sel) return sig;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Internal decoders
// ---------------------------------------------------------------------------

interface DecodeResult {
  value: DecodedValue;
  consumed: number; // hex chars consumed from the head
}

function decodeTupleParams(
  data: string,
  params: AbiParam[],
  baseOffset: number,
): DecodeResult {
  const result: Record<string, DecodedValue> = {};
  let headPos = 0;

  for (const param of params) {
    const dyn = isDynamic(param.type, param.components);
    if (dyn) {
      // Read offset pointer from head
      const offset = Number(readUint(data, headPos)) * 2; // bytes → hex chars
      const { value } = decodeType(data, param, offset);
      const key = param.name || `_${params.indexOf(param)}`;
      result[key] = value;
      headPos += 64;
    } else {
      const { value, consumed } = decodeType(data, param, headPos);
      const key = param.name || `_${params.indexOf(param)}`;
      result[key] = value;
      headPos += consumed;
    }
  }

  return { value: result, consumed: headPos };
}

function decodeType(data: string, param: AbiParam, pos: number): DecodeResult {
  const { type, components } = param;

  // Dynamic array
  if (type.endsWith('[]')) {
    return decodeDynArray(data, param, pos);
  }

  // Fixed array (e.g. uint256[3])
  const fixedMatch = type.match(/^(.+)\[(\d+)\]$/);
  if (fixedMatch) {
    return decodeFixedArray(data, param, pos, parseInt(fixedMatch[2]));
  }

  // Tuple
  if (type === 'tuple' || type.startsWith('tuple')) {
    return decodeTupleParams(data, components ?? [], pos);
  }

  // bytes (dynamic)
  if (type === 'bytes') {
    return decodeBytes(data, pos);
  }

  // string (dynamic)
  if (type === 'string') {
    return decodeString(data, pos);
  }

  // Static types
  return decodeStatic(data, type, pos);
}

function decodeStatic(data: string, type: string, pos: number): DecodeResult {
  const word = data.slice(pos, pos + 64).padEnd(64, '0');

  if (type === 'address') {
    return { value: '0x' + word.slice(24), consumed: 64 };
  }
  if (type === 'bool') {
    return { value: BigInt('0x' + word) !== 0n, consumed: 64 };
  }
  if (type.startsWith('uint')) {
    return { value: BigInt('0x' + word), consumed: 64 };
  }
  if (type.startsWith('int')) {
    const v = BigInt('0x' + word);
    const bits = parseInt(type.slice(3)) || 256;
    const max = 1n << BigInt(bits);
    const half = max >> 1n;
    return { value: v >= half ? v - max : v, consumed: 64 };
  }
  if (type.startsWith('bytes')) {
    // bytesN (fixed)
    const n = parseInt(type.slice(5));
    return { value: '0x' + word.slice(0, n * 2), consumed: 64 };
  }

  // Fallback: return raw hex
  return { value: '0x' + word, consumed: 64 };
}

function decodeBytes(data: string, pos: number): DecodeResult {
  const len = readUint(data, pos);
  const start = pos + 64;
  const hexLen = Number(len) * 2;
  return { value: '0x' + data.slice(start, start + hexLen), consumed: 64 };
}

function decodeString(data: string, pos: number): DecodeResult {
  const len = readUint(data, pos);
  const start = pos + 64;
  const hexLen = Number(len) * 2;
  const hex = data.slice(start, start + hexLen);
  try {
    const bytes = new Uint8Array(hex.match(/.{1,2}/g)!.map(b => parseInt(b, 16)));
    return { value: new TextDecoder().decode(bytes), consumed: 64 };
  } catch {
    return { value: '0x' + hex, consumed: 64 };
  }
}

function decodeDynArray(data: string, param: AbiParam, pos: number): DecodeResult {
  const len = Number(readUint(data, pos));
  const elemsStart = pos + 64;
  const baseType = param.type.slice(0, -2); // remove "[]"
  const elemParam: AbiParam = { type: baseType, name: '', components: param.components };
  const dyn = isDynamic(baseType, param.components);

  const items: DecodedValue[] = [];
  let headPos = elemsStart;

  for (let i = 0; i < len && i < 200; i++) {
    if (dyn) {
      const offset = readUint(data, headPos) * 2n;
      const { value } = decodeType(data, elemParam, elemsStart + Number(offset));
      items.push(value);
      headPos += 64;
    } else {
      const { value, consumed } = decodeType(data, elemParam, headPos);
      items.push(value);
      headPos += consumed;
    }
  }

  return { value: items, consumed: 64 };
}

function decodeFixedArray(data: string, param: AbiParam, pos: number, size: number): DecodeResult {
  const baseType = param.type.replace(/\[\d+\]$/, '');
  const elemParam: AbiParam = { type: baseType, name: '', components: param.components };
  const items: DecodedValue[] = [];
  let offset = pos;

  for (let i = 0; i < size; i++) {
    const { value, consumed } = decodeType(data, elemParam, offset);
    items.push(value);
    offset += consumed;
  }

  return { value: items, consumed: offset - pos };
}

function readUint(data: string, pos: number): bigint {
  const word = data.slice(pos, pos + 64).padEnd(64, '0');
  return BigInt('0x' + word);
}
