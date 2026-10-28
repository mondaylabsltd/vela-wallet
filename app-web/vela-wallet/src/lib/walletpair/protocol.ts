// WalletPair core messages: spec/core.md §2.3, §3, §4 and §7.
//
// Shared by the extension (the DApp side) and the reference Wallet used in
// tests, and checked against spec/test-vectors/core.json.

export const MAX_MESSAGE_BYTES = 1_048_576;

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type Params = Json[] | { [key: string]: Json };

export interface Request {
  id: string;
  method: string;
  params?: Params;
  origin?: string;
}

export interface RpcError {
  code: number;
  message: string;
  data?: Json;
}

export type Response = { id: string; result: Json } | { id: string; error: RpcError };

export interface Event {
  event: string;
  data: Json;
}

export interface NamespaceState {
  chain: string;
  accounts: string[];
}

/** Approved namespaces with their active chain and accounts (core §4.2). */
export type SessionState = Record<string, NamespaceState>;

export interface ConnectParams {
  name: string;
  url?: string;
  icon?: string;
  namespaces: string[];
}

export interface WalletInfo {
  name: string;
  icon?: string;
}

export interface ConnectResult {
  wallet: WalletInfo;
  state: SessionState;
}

export const ErrorCode = {
  UserRejected: 4001,
  Unauthorized: 4100,
  Unsupported: 4200,
  Disconnected: 4900,
  ChainMismatch: 4901,
  LimitExceeded: -32005,
  InvalidRequest: -32600,
  InvalidParams: -32602,
  Internal: -32603,
} as const;

export function rpcError(code: number, message: string, data?: Json): RpcError {
  return data === undefined ? { code, message } : { code, message, data };
}

// ── Encoding ──

const encoder = new TextEncoder();
// ignoreBOM keeps a byte order mark in the text, so JSON.parse rejects it.
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

export function encode(message: Request | Response | Event): Uint8Array {
  const bytes = encoder.encode(JSON.stringify(message));
  if (bytes.length > MAX_MESSAGE_BYTES) throw new RangeError('WalletPair message exceeds 1 MiB');
  return bytes;
}

/** The JSON object in a message, or undefined if it is not one (core §2.3). */
export function decodeObject(bytes: Uint8Array): Record<string, unknown> | undefined {
  if (bytes.length > MAX_MESSAGE_BYTES) return undefined;
  let value: unknown;
  try {
    value = JSON.parse(decoder.decode(bytes));
  } catch {
    return undefined;
  }
  return isObject(value) ? value : undefined;
}

// ── Field rules ──

const PRINTABLE_ASCII = /^[\x20-\x7e]{1,128}$/;

export function isId(value: unknown): value is string {
  return typeof value === 'string' && PRINTABLE_ASCII.test(value);
}

export function isMethod(value: unknown): value is string {
  return typeof value === 'string' && PRINTABLE_ASCII.test(value);
}

export function isOrigin(value: unknown): value is string {
  if (typeof value !== 'string' || value.length > 2048) return false;
  try {
    const url = new URL(value);
    return (url.protocol === 'https:' || url.protocol === 'http:') && url.origin === value;
  } catch {
    return false;
  }
}

/** CAIP-2 namespace syntax. */
export function isNamespace(value: unknown): value is string {
  return typeof value === 'string' && /^[-a-z0-9]{3,8}$/.test(value);
}

// Control characters, and the bidirectional formatting characters that can
// make one name look like another.
const UNSAFE_TEXT = /[\p{Cc}؜‎‏‪-‮⁦-⁩]/u;

/** A display name: 1 to 128 characters, nothing that can disguise it (core §4.1). */
export function isName(value: unknown): value is string {
  if (typeof value !== 'string') return false;
  const length = [...value].length;
  return length >= 1 && length <= 128 && !UNSAFE_TEXT.test(value) && value.trim() === value;
}

export function isSiteUrl(value: unknown): value is string {
  if (typeof value !== 'string' || value.length > 2048) return false;
  try {
    const url = new URL(value);
    if (url.protocol === 'https:') return true;
    return url.protocol === 'http:' && (url.hostname === 'localhost' || url.hostname === '127.0.0.1');
  } catch {
    return false;
  }
}

const DATA_IMAGE = /^data:image\/(png|jpeg|webp|gif|svg\+xml);base64,[A-Za-z0-9+/]+={0,2}$/;

export function isIcon(value: unknown): value is string {
  if (typeof value !== 'string' || value.length > 65_536) return false;
  if (value.startsWith('data:')) return DATA_IMAGE.test(value);
  try {
    return new URL(value).protocol === 'https:';
  } catch {
    return false;
  }
}

// ── Messages ──

export type WalletInbound =
  | { kind: 'ignore' }
  | { kind: 'invalid'; id: string }
  | { kind: 'request'; request: Request };

/** Steps 1–3 of a Wallet's receive procedure (core §3.4). */
export function parseRequest(bytes: Uint8Array): WalletInbound {
  const value = decodeObject(bytes);
  if (!value || !isId(value.id)) return { kind: 'ignore' };
  const { id, method, params, origin } = value;
  if (!isMethod(method)) return { kind: 'invalid', id };
  if (params !== undefined && !Array.isArray(params) && !isObject(params)) return { kind: 'invalid', id };
  if (origin !== undefined && !isOrigin(origin)) return { kind: 'invalid', id };
  const request: Request = { id, method };
  if (params !== undefined) request.params = params as Params;
  if (origin !== undefined) request.origin = origin;
  return { kind: 'request', request };
}

export type DAppInbound =
  | { kind: 'ignore' }
  | { kind: 'response'; response: Response }
  | { kind: 'event'; event: Event };

/** How a DApp side classifies a message from the Wallet (core §3.4). */
export function parseFromWallet(bytes: Uint8Array): DAppInbound {
  const value = decodeObject(bytes);
  if (!value) return { kind: 'ignore' };
  if (typeof value.event === 'string') {
    if (!('data' in value)) return { kind: 'ignore' };
    return { kind: 'event', event: { event: value.event, data: value.data as Json } };
  }
  if (!isId(value.id)) return { kind: 'ignore' };
  const hasResult = 'result' in value;
  const hasError = 'error' in value;
  if (hasResult === hasError) return { kind: 'ignore' };
  if (hasResult) return { kind: 'response', response: { id: value.id, result: value.result as Json } };
  const error = value.error;
  if (!isObject(error) || !Number.isInteger(error.code) || typeof error.message !== 'string') {
    return { kind: 'ignore' };
  }
  const parsed = rpcError(error.code as number, error.message, error.data as Json | undefined);
  return { kind: 'response', response: { id: value.id, error: parsed } };
}

// ── Session ──

/** The input of walletpair_connect, or null if invalid (core §4.1). */
export function parseConnectParams(params: Params | undefined): ConnectParams | null {
  if (!Array.isArray(params) || params.length !== 1 || !isObject(params[0])) return null;
  const { name, url, icon, namespaces } = params[0];
  if (!isName(name)) return null;
  if (url !== undefined && !isSiteUrl(url)) return null;
  if (icon !== undefined && !isIcon(icon)) return null;
  if (!Array.isArray(namespaces) || namespaces.length < 1 || namespaces.length > 16) return null;
  if (!namespaces.every(isNamespace) || new Set(namespaces).size !== namespaces.length) return null;
  const parsed: ConnectParams = { name, namespaces: namespaces as string[] };
  if (url !== undefined) parsed.url = url;
  if (icon !== undefined) parsed.icon = icon;
  return parsed;
}

/** Validates chains and accounts of the namespaces this side knows. */
export interface NamespaceRules {
  isChain(chain: string): boolean;
  isAccount(account: string): boolean;
}

/**
 * The session state in a walletpair_connect result or state event, or null
 * if invalid (core §4.2). Namespaces without rules are dropped: a receiver
 * ignores what it does not know.
 */
export function parseState(value: unknown, rules: Record<string, NamespaceRules>): SessionState | null {
  if (!isObject(value)) return null;
  const state: SessionState = {};
  for (const [namespace, entry] of Object.entries(value)) {
    if (!isNamespace(namespace)) return null;
    const rule = rules[namespace];
    if (!rule) continue;
    if (!isObject(entry) || typeof entry.chain !== 'string' || !rule.isChain(entry.chain)) return null;
    const accounts = entry.accounts;
    if (!Array.isArray(accounts) || accounts.length < 1 || accounts.length > 64) return null;
    if (!accounts.every((a) => typeof a === 'string' && rule.isAccount(a))) return null;
    if (new Set(accounts).size !== accounts.length) return null;
    state[namespace] = { chain: entry.chain, accounts: [...(accounts as string[])] };
  }
  return state;
}

export function parseConnectResult(value: unknown, rules: Record<string, NamespaceRules>): ConnectResult | null {
  if (!isObject(value) || !isObject(value.wallet)) return null;
  const { name, icon } = value.wallet;
  if (!isName(name) || (icon !== undefined && !isIcon(icon))) return null;
  const state = parseState(value.state, rules);
  if (!state) return null;
  const wallet: WalletInfo = { name };
  if (icon !== undefined) wallet.icon = icon;
  return { wallet, state };
}

export function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
