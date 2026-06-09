/**
 * ERC-7730 Clear Signing service.
 *
 * Fetches descriptors from the ethereum-data server, matches function selectors
 * or EIP-712 type hashes, and resolves fields into human-readable display data.
 *
 * Lookup flow for eth_sendTransaction:
 *   1. Fetch calldata/eip155-{chainId}/{to}.json → match by selector
 *   2. Fallback to ercs/calldata-erc20-tokens.json etc.
 *   3. If no match → blind sign (return null)
 *
 * Lookup flow for eth_signTypedData:
 *   1. Compute encodeTypeHash → fetch eip712/eip155-{chainId}/{contract}.json
 *   2. Fallback to ercs/eip712-erc2612-permit.json
 *   3. If no match → blind sign (return null)
 */
import { getEthereumDataURL } from '@/services/storage';
import { checksumAddress, keccak256 } from '@/services/eth-crypto';
import { toHex } from '@/services/hex';
import { decodeCalldata, matchSelector, type DecodedValue } from '@/services/abi-decode';
import type { TypedData } from '@/services/eip712';

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/** Risk level for visual treatment. */
export type SigningRisk = 'safe' | 'normal' | 'caution' | 'danger';

/** Resolved clear signing result — ready for display. */
export interface ClearSignResult {
  /** User-facing intent, e.g. "Swap", "Send", "Approve" */
  intent: string;
  /** Protocol/contract name, e.g. "Uniswap V3 Router" */
  contractName?: string;
  /** Protocol owner, e.g. "Uniswap Labs" */
  owner?: string;
  /** Resolved display fields */
  fields: ClearSignField[];
  /** Risk level derived from intent + fields */
  risk: SigningRisk;
  /** Contract address being interacted with */
  contractAddress?: string;
  /** Whether the contract has a verified descriptor */
  verified: boolean;
  /** Signing type for button/color decisions */
  type: 'transaction' | 'signature';
}

/** Layout role hint for field rendering. */
export type FieldRole = 'send-amount' | 'receive-amount' | 'recipient' | 'spender' | 'generic';

export interface ClearSignField {
  label: string;
  value: string;
  format: string;
  /** For tokenAmount: token address for logo lookup */
  tokenAddress?: string;
  /** Whether this is a high-risk field (e.g. unlimited approval) */
  warning?: boolean;
  /** Role hint for layout decisions */
  role?: FieldRole;
  /** Whether this field should be in the collapsed details panel */
  detail?: boolean;
}

// ---------------------------------------------------------------------------
// In-memory cache
// ---------------------------------------------------------------------------

const descriptorCache = new Map<string, any | null>();

/** Clear the descriptor cache (for testing). */
export function clearDescriptorCache(): void {
  descriptorCache.clear();
}

async function fetchDescriptor(path: string): Promise<any | null> {
  if (descriptorCache.has(path)) return descriptorCache.get(path) ?? null;
  try {
    const url = `${getEthereumDataURL()}${path}`;
    const res = await fetch(url);
    if (!res.ok) { descriptorCache.set(path, null); return null; }
    const data = await res.json();
    descriptorCache.set(path, data);
    return data;
  } catch {
    descriptorCache.set(path, null);
    return null;
  }
}

// ---------------------------------------------------------------------------
// eth_sendTransaction clear signing
// ---------------------------------------------------------------------------

/**
 * Resolve a transaction into clear signing fields.
 * Returns null if no descriptor matches (falls back to blind signing).
 */
export async function resolveTransaction(
  to: string,
  data: string | undefined,
  value: string | undefined,
  chainId: number,
): Promise<ClearSignResult | null> {
  if (!data || data === '0x' || data === '') {
    // Plain ETH transfer — no calldata
    return null; // Let the modal show its native transfer UI
  }
  if (!to) return null;

  const toAddr = to.toLowerCase();
  const checksumTo = checksumAddress(toAddr);

  // 1. Try contract-specific descriptor
  let descriptor = await fetchDescriptor(`/erc7730/calldata/eip155-${chainId}/${checksumTo}.json`);
  let isContractSpecific = !!descriptor;

  // 2. Fallback to ERC standards
  if (!descriptor) {
    descriptor = await tryErcFallbacks(data);
  }

  if (!descriptor) return null;

  return resolveCalldataDescriptor(descriptor, data, value, toAddr, chainId, isContractSpecific);
}

const ERC_CALLDATA_FALLBACKS = [
  '/erc7730/ercs/calldata-erc20-tokens.json',
  '/erc7730/ercs/calldata-erc721-nfts.json',
  '/erc7730/ercs/calldata-erc4626-vaults.json',
];

async function tryErcFallbacks(calldata: string): Promise<any | null> {
  for (const path of ERC_CALLDATA_FALLBACKS) {
    const desc = await fetchDescriptor(path);
    if (!desc?.display?.formats) continue;
    const sigs = Object.keys(desc.display.formats);
    if (matchSelector(calldata, sigs)) return desc;
  }
  return null;
}

function resolveCalldataDescriptor(
  descriptor: any,
  calldata: string,
  value: string | undefined,
  toAddr: string,
  chainId: number,
  isContractSpecific: boolean,
): ClearSignResult | null {
  const formats = descriptor.display?.formats;
  if (!formats) return null;

  const sigs = Object.keys(formats);
  const matchedSig = matchSelector(calldata, sigs);
  if (!matchedSig) return null;

  const format = formats[matchedSig];
  const decoded = decodeCalldata(calldata, matchedSig);
  if (!decoded) return null;

  // Build context for path resolution
  const txContext = { '@': { to: toAddr, value: value ?? '0x0', from: '' } };
  const fullContext = { ...flattenDecoded(decoded), ...txContext };

  const fields = resolveFields(
    format.fields ?? [],
    fullContext,
    descriptor.metadata,
    descriptor.display?.definitions,
    chainId,
  );

  // Safety check: if descriptor declared visible fields but we resolved
  // less than half, the clear sign UI would be misleading — fall back to
  // blind sign so the user doesn't think they've seen everything.
  const declaredVisible = (format.fields ?? []).filter(
    (f: any) => f.visible !== 'never' && f.label,
  ).length;
  if (declaredVisible > 0 && fields.length < Math.ceil(declaredVisible / 2)) {
    console.warn(`[clear-sign] Only resolved ${fields.length}/${declaredVisible} fields for ${matchedSig} — falling back to blind sign`);
    return null;
  }

  const intent = format.intent ?? matchedSig.split('(')[0];
  const enrichedFields = inferFieldRoles(fields, intent);

  return {
    intent,
    contractName: isContractSpecific
      ? (descriptor.metadata?.contractName ?? descriptor.context?.$id)
      : undefined,
    owner: isContractSpecific ? descriptor.metadata?.owner : undefined,
    fields: enrichedFields,
    risk: assessRisk(intent, enrichedFields, 'transaction'),
    contractAddress: toAddr,
    verified: isContractSpecific,
    type: 'transaction',
  };
}

// ---------------------------------------------------------------------------
// eth_signTypedData clear signing
// ---------------------------------------------------------------------------

/**
 * Resolve EIP-712 typed data into clear signing fields.
 */
export async function resolveTypedData(
  typedData: TypedData,
  chainId: number,
): Promise<ClearSignResult | null> {
  const contract = typedData.domain?.verifyingContract?.toLowerCase();
  if (!contract) return null;

  const checksumContract = checksumAddress(contract);

  // Compute the encodeTypeHash for the primary type
  const encodeType = buildEncodeType(typedData.primaryType, typedData.types);
  const typeHash = toHex(keccak256(new TextEncoder().encode(encodeType)));

  // 1. Try contract-specific descriptor
  let descriptor = await fetchDescriptor(`/erc7730/eip712/eip155-${chainId}/${checksumContract}.json`);
  let resolved: ClearSignResult | null = null;

  if (descriptor) {
    // EIP-712 descriptors are keyed by typeHash
    const entry = descriptor[typeHash] ?? descriptor['0x' + typeHash];
    if (entry) {
      resolved = resolveEip712Entry(entry, typedData, chainId);
    }
  }

  // 2. Fallback to ERC-2612 permit
  if (!resolved) {
    const permitDesc = await fetchDescriptor('/erc7730/ercs/eip712-erc2612-permit.json');
    if (permitDesc) {
      // Permit files have formats keyed by type signature
      resolved = resolveEip712Formats(permitDesc, typedData, chainId);
    }
  }

  return resolved;
}

function resolveEip712Entry(
  entry: any,
  typedData: TypedData,
  chainId: number,
): ClearSignResult | null {
  const formats = entry.display?.formats;
  if (!formats) return null;

  // Match by primary type signature
  const sigs = Object.keys(formats);
  const matchedSig = sigs.find(sig => sig.startsWith(typedData.primaryType + '('));
  if (!matchedSig) return null;

  const format = formats[matchedSig];
  const context = flattenForEip712(typedData.message);

  const fields = resolveFields(
    format.fields ?? [],
    context,
    entry.metadata,
    entry.display?.definitions,
    chainId,
  );

  // Safety: fall back to blind sign if too many fields failed to resolve
  const declaredVisible = (format.fields ?? []).filter(
    (f: any) => f.visible !== 'never' && f.label,
  ).length;
  if (declaredVisible > 0 && fields.length < Math.ceil(declaredVisible / 2)) {
    console.warn(`[clear-sign] EIP-712: only resolved ${fields.length}/${declaredVisible} fields — falling back`);
    return null;
  }

  const intent = format.intent ?? typedData.primaryType;
  const enrichedFields = inferFieldRoles(fields, intent);
  const contract = typedData.domain?.verifyingContract?.toLowerCase();

  return {
    intent,
    contractName: entry.metadata?.contractName ?? entry.context?.eip712?.domain?.name,
    owner: entry.metadata?.owner,
    fields: enrichedFields,
    risk: assessRisk(intent, enrichedFields, 'signature'),
    contractAddress: contract,
    verified: true,
    type: 'signature' as const,
  };
}

function resolveEip712Formats(
  descriptor: any,
  typedData: TypedData,
  chainId: number,
): ClearSignResult | null {
  const formats = descriptor.display?.formats;
  if (!formats) return null;

  const sigs = Object.keys(formats);
  const matchedSig = sigs.find(sig => sig.startsWith(typedData.primaryType + '('));
  if (!matchedSig) return null;

  const format = formats[matchedSig];
  const context = flattenForEip712(typedData.message);

  const fields = resolveFields(
    format.fields ?? [],
    context,
    descriptor.metadata,
    descriptor.display?.definitions,
    chainId,
  );

  const declaredVisible = (format.fields ?? []).filter(
    (f: any) => f.visible !== 'never' && f.label,
  ).length;
  if (declaredVisible > 0 && fields.length < Math.ceil(declaredVisible / 2)) {
    console.warn(`[clear-sign] ERC fallback: only resolved ${fields.length}/${declaredVisible} fields — falling back`);
    return null;
  }

  const intent = format.intent ?? typedData.primaryType;
  const enrichedFields = inferFieldRoles(fields, intent);

  return {
    intent,
    fields: enrichedFields,
    risk: assessRisk(intent, enrichedFields, 'signature'),
    contractAddress: typedData.domain?.verifyingContract?.toLowerCase(),
    verified: false,
    type: 'signature' as const,
  };
}

// ---------------------------------------------------------------------------
// encodeType for EIP-712 typeHash computation
// ---------------------------------------------------------------------------

function buildEncodeType(primaryType: string, types: Record<string, { name: string; type: string }[]>): string {
  const deps = new Set<string>();
  collectDeps(primaryType, types, deps);
  deps.delete(primaryType);
  const sorted = [primaryType, ...Array.from(deps).sort()];
  return sorted.map(t => {
    const fields = types[t];
    if (!fields) return '';
    return `${t}(${fields.map(f => `${f.type} ${f.name}`).join(',')})`;
  }).join('');
}

function collectDeps(
  type: string,
  types: Record<string, { name: string; type: string }[]>,
  deps: Set<string>,
): void {
  if (deps.has(type)) return;
  const fields = types[type];
  if (!fields) return;
  deps.add(type);
  for (const f of fields) {
    const baseType = f.type.replace(/\[\d*\]$/, '');
    if (types[baseType]) collectDeps(baseType, types, deps);
  }
}

// ---------------------------------------------------------------------------
// Field resolution
// ---------------------------------------------------------------------------

function resolveFields(
  fieldDefs: any[],
  context: any,
  metadata: any,
  definitions: any,
  chainId: number,
): ClearSignField[] {
  const fields: ClearSignField[] = [];

  for (const fd of fieldDefs) {
    // Resolve $ref
    let def = fd;
    if (fd.$ref && definitions) {
      const refPath = fd.$ref.replace('$.display.definitions.', '');
      def = { ...definitions[refPath], ...fd, $ref: undefined };
    }

    if (def.visible === 'never') continue;

    const label = def.label ?? '';
    const format = def.format ?? 'raw';
    const rawValue = resolvePath(def.path, context);

    const formatted = formatField(rawValue, format, def.params, context, metadata, chainId);
    if (formatted) {
      fields.push(formatted.warning
        ? { label, ...formatted }
        : { label, ...formatted }
      );
    }
  }

  return fields;
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

function resolvePath(path: string | undefined, context: any): DecodedValue | undefined {
  if (!path) return undefined;

  // Handle "@.field" (transaction-level fields)
  if (path.startsWith('@.')) {
    const key = path.slice(2);
    return context['@']?.[key];
  }

  // Handle "$.metadata.constants.X" or "$.metadata.enums.X"
  if (path.startsWith('$.')) return undefined; // resolved in formatField

  // Handle "field.subfield" for decoded calldata
  const parts = path.split('.');
  let current: any = context;
  for (const part of parts) {
    if (current == null) return undefined;
    // Handle slice notation like "[-20:]" or "[0:20]"
    if (part.includes('[') && part.includes(':')) {
      const base = part.split('[')[0];
      if (base) current = current[base];
      const slice = part.match(/\[(-?\d*):(-?\d*)\]/);
      if (slice && typeof current === 'string') {
        const s = current.startsWith('0x') ? current.slice(2) : current;
        const start = slice[1] ? parseInt(slice[1]) : 0;
        const end = slice[2] ? parseInt(slice[2]) : s.length / 2;
        if (start < 0) {
          current = '0x' + s.slice(start * 2);
        } else {
          current = '0x' + s.slice(start * 2, end * 2);
        }
      }
      continue;
    }
    // Handle array iteration "[]"
    if (part === '[]') {
      if (Array.isArray(current)) return current.map(String).join(', ');
      continue;
    }
    current = current[part];
  }
  return current;
}

function resolveMetadataRef(path: string, metadata: any): any {
  if (!path || !metadata) return undefined;
  // "$.metadata.constants.X" or "$.metadata.enums.X"
  const parts = path.replace('$.metadata.', '').split('.');
  let current = metadata;
  for (const p of parts) {
    if (current == null) return undefined;
    current = current[p];
  }
  return current;
}

// ---------------------------------------------------------------------------
// Field formatting
// ---------------------------------------------------------------------------

interface FormattedField {
  value: string;
  format: string;
  tokenAddress?: string;
  warning?: boolean;
}

function formatField(
  rawValue: DecodedValue | undefined,
  format: string,
  params: any,
  context: any,
  metadata: any,
  chainId: number,
): FormattedField | null {
  if (rawValue === undefined && format !== 'amount') return null;

  switch (format) {
    case 'tokenAmount':
      return formatTokenAmount(rawValue, params, context, metadata);

    case 'addressName':
      return formatAddress(rawValue);

    case 'amount':
      return formatNativeAmount(rawValue);

    case 'raw':
      return formatRaw(rawValue);

    case 'date':
      return formatDate(rawValue, params);

    case 'duration':
      return formatDuration(rawValue);

    case 'enum':
      return formatEnum(rawValue, params, metadata);

    case 'nftName':
      return { value: String(rawValue ?? 'NFT'), format };

    case 'unit':
      return formatUnit(rawValue, params);

    case 'calldata':
      return { value: truncateHex(String(rawValue ?? '')), format: 'raw' };

    default:
      return { value: String(rawValue ?? ''), format };
  }
}

function formatTokenAmount(
  rawValue: DecodedValue | undefined,
  params: any,
  context: any,
  metadata: any,
): FormattedField | null {
  const amount = toBigInt(rawValue);

  // Check threshold for unlimited approvals
  if (params?.threshold) {
    const threshold = BigInt(params.threshold);
    if (amount >= threshold) {
      return {
        value: params.message ?? 'Unlimited',
        format: 'tokenAmount',
        warning: true,
      };
    }
  }

  // Resolve token address from path
  let tokenAddr: string | undefined;
  if (params?.tokenPath) {
    const resolved = resolvePath(params.tokenPath, context);
    if (typeof resolved === 'string') tokenAddr = resolved;
  }
  if (params?.token) {
    const resolved = resolveMetadataRef(params.token, metadata);
    if (typeof resolved === 'string') tokenAddr = resolved;
  }

  // Check native currency
  if (params?.nativeCurrencyAddress) {
    for (const refPath of params.nativeCurrencyAddress) {
      const addr = resolveMetadataRef(refPath, metadata);
      if (addr && tokenAddr?.toLowerCase() === String(addr).toLowerCase()) {
        tokenAddr = undefined; // native token, no address needed
        break;
      }
    }
  }

  // Use known decimals when possible, fallback to smart detection
  const decimals = guessTokenDecimals(tokenAddr);
  const display = formatTokenValue(amount, decimals);
  // Always show a token identifier — known symbol, abbreviated address, or "tokens"
  const symbol = tokenAddr
    ? (guessTokenSymbol(tokenAddr) ?? `${tokenAddr.slice(0, 6)}...`)
    : 'tokens';
  const displayWithSymbol = `${display} ${symbol}`;

  return {
    value: displayWithSymbol,
    format: 'tokenAmount',
    tokenAddress: tokenAddr ? normalizeAddress(String(tokenAddr)) : undefined,
  };
}

function formatAddress(rawValue: DecodedValue | undefined): FormattedField | null {
  if (!rawValue) return null;
  const addr = String(rawValue);
  if (addr.length < 10) return { value: addr, format: 'addressName' };
  return {
    value: `${addr.slice(0, 8)}...${addr.slice(-6)}`,
    format: 'addressName',
  };
}

function formatNativeAmount(rawValue: DecodedValue | undefined): FormattedField | null {
  const amount = toBigInt(rawValue);
  if (amount === 0n) return null;
  return { value: formatWeiAmount(amount), format: 'amount' };
}

function formatRaw(rawValue: DecodedValue | undefined): FormattedField | null {
  if (rawValue === undefined) return null;
  const s = String(rawValue);
  return { value: truncateHex(s), format: 'raw' };
}

function formatDate(rawValue: DecodedValue | undefined, params: any): FormattedField | null {
  const ts = Number(toBigInt(rawValue));
  if (ts === 0) return null;
  try {
    const date = new Date(ts * 1000);
    return { value: date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit' }), format: 'date' };
  } catch {
    return { value: String(ts), format: 'date' };
  }
}

function formatDuration(rawValue: DecodedValue | undefined): FormattedField | null {
  const secs = Number(toBigInt(rawValue));
  if (secs === 0) return null;
  if (secs < 60) return { value: `${secs}s`, format: 'duration' };
  if (secs < 3600) return { value: `${Math.floor(secs / 60)}m`, format: 'duration' };
  if (secs < 86400) return { value: `${Math.floor(secs / 3600)}h`, format: 'duration' };
  return { value: `${Math.floor(secs / 86400)}d`, format: 'duration' };
}

function formatEnum(rawValue: DecodedValue | undefined, params: any, metadata: any): FormattedField | null {
  if (rawValue === undefined) return null;
  const key = String(rawValue);
  if (params?.$ref) {
    const enumDef = resolveMetadataRef(params.$ref, metadata);
    if (enumDef && enumDef[key]) return { value: enumDef[key], format: 'enum' };
  }
  return { value: key, format: 'enum' };
}

function formatUnit(rawValue: DecodedValue | undefined, params: any): FormattedField | null {
  if (rawValue === undefined) return null;
  const num = Number(toBigInt(rawValue));
  const decimals = params?.decimals ?? 0;
  const base = params?.base ?? '';
  const prefix = params?.prefix ?? false;
  const display = decimals > 0 ? (num / Math.pow(10, decimals)).toFixed(decimals) : String(num);
  const formatted = prefix ? `${base}${display}` : `${display}${base}`;
  return { value: formatted.trim(), format: 'unit' };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function toBigInt(v: DecodedValue | undefined): bigint {
  if (v === undefined || v === null) return 0n;
  if (typeof v === 'bigint') return v;
  if (typeof v === 'boolean') return v ? 1n : 0n;
  if (typeof v === 'string') {
    try {
      if (v.startsWith('0x')) return BigInt(v);
      return BigInt(v);
    } catch { return 0n; }
  }
  return 0n;
}

function formatWeiAmount(wei: bigint): string {
  if (wei === 0n) return '0';
  const eth = Number(wei) / 1e18;
  if (eth >= 0.0001) {
    return eth.toFixed(6).replace(/\.?0+$/, '');
  }
  if (wei < 1000000n) return `${wei} wei`;
  return eth.toFixed(8).replace(/\.?0+$/, '');
}

/** Well-known ERC-20 token decimals (mainnet addresses, lowercased). */
const KNOWN_DECIMALS: Record<string, number> = {
  '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48': 6,  // USDC
  '0xdac17f958d2ee523a2206206994597c13d831ec7': 6,  // USDT
  '0x6b175474e89094c44da98b954eedeac495271d0f': 18, // DAI
  '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2': 18, // WETH
  '0x2260fac5e5542a773aa44fbcfedf7c193bc2c599': 8,  // WBTC
  '0x514910771af9ca656af840dff83e8264ecf986ca': 18, // LINK
  '0x1f9840a85d5af5bf1d1762f925bdaddc4201f984': 18, // UNI
  // Polygon
  '0x3c499c542cef5e3811e1192ce70d8cc03d5c3359': 6,  // USDC (Polygon)
  '0x2791bca1f2de4661ed88a30c99a7a9449aa84174': 6,  // USDC.e (Polygon)
  // Arbitrum
  '0xaf88d065e77c8cc2239327c5edb3a432268e5831': 6,  // USDC (Arbitrum)
  '0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9': 6,  // USDT (Arbitrum)
};

function guessTokenDecimals(tokenAddr: string | undefined): number {
  if (!tokenAddr) return 18;
  const known = KNOWN_DECIMALS[tokenAddr.toLowerCase()];
  if (known !== undefined) return known;
  return 18;
}

/** Well-known ERC-20 token symbols (mainnet addresses, lowercased). */
const KNOWN_SYMBOLS: Record<string, string> = {
  '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48': 'USDC',
  '0xdac17f958d2ee523a2206206994597c13d831ec7': 'USDT',
  '0x6b175474e89094c44da98b954eedeac495271d0f': 'DAI',
  '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2': 'WETH',
  '0x2260fac5e5542a773aa44fbcfedf7c193bc2c599': 'WBTC',
  '0x514910771af9ca656af840dff83e8264ecf986ca': 'LINK',
  '0x1f9840a85d5af5bf1d1762f925bdaddc4201f984': 'UNI',
  '0xae7ab96520de3a18e5e111b5eaab095312d7fe84': 'stETH',
  '0xbe9895146f7af43049ca1c1ae358b0541ea49704': 'cbETH',
  '0xae78736cd615f374d3085123a210448e74fc6393': 'rETH',
  '0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0': 'wstETH',
  '0x5a98fcbea516cf06857215779fd812ca3bef1b32': 'LDO',
  '0xd533a949740bb3306d119cc777fa900ba034cd52': 'CRV',
  '0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9': 'AAVE',
  '0xc00e94cb662c3520282e6f5717214004a7f26888': 'COMP',
  '0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2': 'MKR',
  '0x3c499c542cef5e3811e1192ce70d8cc03d5c3359': 'USDC',
  '0x2791bca1f2de4661ed88a30c99a7a9449aa84174': 'USDC.e',
  '0xaf88d065e77c8cc2239327c5edb3a432268e5831': 'USDC',
  '0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9': 'USDT',
};

function guessTokenSymbol(tokenAddr: string | undefined): string | undefined {
  if (!tokenAddr) return undefined;
  return KNOWN_SYMBOLS[tokenAddr.toLowerCase()];
}

function formatTokenValue(raw: bigint, decimals: number): string {
  if (raw === 0n) return '0';
  const divisor = 10 ** decimals;
  const whole = raw / BigInt(divisor);
  const frac = raw % BigInt(divisor);

  if (frac === 0n) {
    return formatWithCommas(whole.toString());
  }

  const fracStr = frac.toString().padStart(decimals, '0');
  // Show up to 4 significant fractional digits, trim trailing zeros
  const trimmed = fracStr.slice(0, Math.min(4, decimals)).replace(/0+$/, '');
  if (!trimmed) return formatWithCommas(whole.toString());
  return `${formatWithCommas(whole.toString())}.${trimmed}`;
}

function formatWithCommas(n: string): string {
  return n.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

function normalizeAddress(addr: string): string {
  if (!addr.startsWith('0x')) return '0x' + addr;
  return addr;
}

function truncateHex(s: string): string {
  if (s.length <= 20) return s;
  return `${s.slice(0, 10)}...${s.slice(-8)}`;
}

function flattenDecoded(decoded: Record<string, DecodedValue>): any {
  // Build a context object where both top-level and nested paths work
  const ctx: any = { ...decoded };
  for (const [key, val] of Object.entries(decoded)) {
    if (val && typeof val === 'object' && !Array.isArray(val)) {
      for (const [k2, v2] of Object.entries(val as Record<string, DecodedValue>)) {
        ctx[`${key}.${k2}`] = v2;
      }
    }
  }
  return ctx;
}

function flattenForEip712(message: Record<string, any>): any {
  const ctx: any = { ...message };
  for (const [key, val] of Object.entries(message)) {
    if (val && typeof val === 'object' && !Array.isArray(val)) {
      for (const [k2, v2] of Object.entries(val)) {
        ctx[`${key}.${k2}`] = v2;
      }
    }
  }
  return ctx;
}

// ---------------------------------------------------------------------------
// Risk assessment
// ---------------------------------------------------------------------------

function assessRisk(
  intent: string,
  fields: ClearSignField[],
  type: 'transaction' | 'signature',
): SigningRisk {
  // Any field with a warning (e.g. unlimited approval) → danger
  if (fields.some(f => f.warning)) return 'danger';

  const i = intent.toLowerCase();
  if (/approve|permit|authorize/.test(i)) return 'caution';
  if (/stake|deposit|claim|supply/.test(i)) return 'safe';
  if (/swap|send|transfer|buy|exchange/.test(i)) return 'normal';
  if (/withdraw|redeem|unstake|exit/.test(i)) return 'normal';
  return 'normal';
}

// ---------------------------------------------------------------------------
// Field role inference
// ---------------------------------------------------------------------------

function inferFieldRoles(fields: ClearSignField[], intent: string): ClearSignField[] {
  const i = intent.toLowerCase();
  return fields.map(f => {
    if (f.role) return f; // already assigned
    const label = f.label.toLowerCase();

    // Amount roles
    if (f.format === 'tokenAmount' || f.format === 'amount') {
      if (/receive|output|min|return|get/.test(label)) return { ...f, role: 'receive-amount' as const };
      if (/send|pay|input|deposit|spend|stake|amount|value/.test(label)) return { ...f, role: 'send-amount' as const };
      // For approve/permit: the amount is what's being approved
      if (/approve|swap/.test(i)) return { ...f, role: 'send-amount' as const };
      return { ...f, role: 'send-amount' as const };
    }

    // Address roles
    if (f.format === 'addressName') {
      if (/spender|operator/.test(label)) return { ...f, role: 'spender' as const };
      if (/to|recipient|receiver|destination/.test(label)) return { ...f, role: 'recipient' as const };
      return { ...f, role: 'generic' as const };
    }

    return { ...f, role: 'generic' as const };
  });
}
