// EVM chains, namespace eip155: spec/networks/evm.md.

import { ErrorCode, isObject, rpcError, type NamespaceState, type Params, type RpcError } from '../protocol';
import type { Network, WalletFunction } from './types';

const CHAIN = /^eip155:[1-9][0-9]{0,31}$/;
const ACCOUNT = /^0x[0-9a-f]{40}$/;
const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const DATA = /^0x(?:[0-9a-fA-F]{2})*$/;
const QUANTITY = /^0x(?:0|[1-9a-fA-F][0-9a-fA-F]{0,63})$/;

export const isEvmChain = (chain: string): boolean => CHAIN.test(chain);
export const isEvmAccount = (account: string): boolean => ACCOUNT.test(account);

/** `eip155:137` → `0x89` */
export function chainIdOf(chain: string): string {
  return `0x${BigInt(chain.slice('eip155:'.length)).toString(16)}`;
}

/** `0x89` → `eip155:137` */
export function chainOf(chainId: string): string {
  return `eip155:${BigInt(chainId).toString(10)}`;
}

const invalid = (message = 'Invalid params'): RpcError => rpcError(ErrorCode.InvalidParams, message);
const notApproved = (): RpcError => rpcError(ErrorCode.Unauthorized, 'Account is not approved');
const otherChain = (): RpcError => rpcError(ErrorCode.ChainMismatch, 'Chain is not active');

function isApproved(address: string, state: NamespaceState): boolean {
  return state.accounts.includes(address.toLowerCase());
}

/** An EIP-155 chain ID given as a number, a decimal string or a quantity. */
function parseChainId(value: unknown): bigint | null {
  if (typeof value === 'number') return Number.isSafeInteger(value) && value > 0 ? BigInt(value) : null;
  if (typeof value !== 'string') return null;
  if (QUANTITY.test(value) || /^[1-9][0-9]{0,77}$/.test(value)) return BigInt(value);
  return null;
}

function activeChainId(state: NamespaceState): bigint {
  return BigInt(state.chain.slice('eip155:'.length));
}

function single(params: Params | undefined): Record<string, unknown> | null {
  return Array.isArray(params) && params.length === 1 && isObject(params[0]) ? params[0] : null;
}

const optional = (value: unknown, test: (v: unknown) => boolean): boolean => value === undefined || test(value);
const quantity = (v: unknown): boolean => typeof v === 'string' && QUANTITY.test(v);
const data = (v: unknown): boolean => typeof v === 'string' && DATA.test(v);
const address = (v: unknown): v is string => typeof v === 'string' && ADDRESS.test(v);
const list = (v: unknown, min: number, item: (v: unknown) => boolean): boolean => Array.isArray(v) && v.length >= min && v.every(item);

/** A chain ID a CAIP-2 chain ID can hold: 1 to 10^32 - 1 (§1). */
const chainId = (v: unknown): boolean => quantity(v) && v !== '0x0' && BigInt(v as string) < 10n ** 32n;

/** An https: URL without spaces or control characters; EIP-3085 rejects http: and file:. */
function httpsUrl(v: unknown): boolean {
  if (typeof v !== 'string' || v.length > 2048 || /[\u0000-\u0020\u007f]/.test(v)) return false;
  try {
    return new URL(v).protocol === 'https:';
  } catch {
    return false;
  }
}

/** EIP-2930: `[{ address, storageKeys: [bytes32…] }]`. */
const accessList = (v: unknown): boolean =>
  list(v, 0, (entry) => isObject(entry) && address(entry.address) && list(entry.storageKeys, 0, (k) => typeof k === 'string' && /^0x[0-9a-fA-F]{64}$/.test(k)));

/** Every type is a list of `{ name, type }`, `primaryType` is one of them, and no value is null. */
function typedDataShape(typed: Record<string, unknown>): boolean {
  const types = typed.types as Record<string, unknown>;
  const field = (f: unknown) => isObject(f) && typeof f.name === 'string' && typeof f.type === 'string';
  if (!Object.values(types).every((fields) => list(fields, 0, field))) return false;
  if (!Object.hasOwn(types, typed.primaryType as string)) return false;
  const noNull = (v: unknown): boolean => v !== null && (typeof v !== 'object' || Object.values(v as object).every(noNull));
  return noNull(typed.domain) && noNull(typed.message);
}

const functions: Record<string, WalletFunction> = {
  wallet_switchEthereumChain: {
    check(params) {
      const input = single(params);
      return input && chainId(input.chainId) ? null : invalid();
    },
  },

  wallet_addEthereumChain: {
    check(params) {
      const input = single(params);
      if (!input || !chainId(input.chainId)) return invalid();
      if (typeof input.chainName !== 'string' || !list(input.rpcUrls, 1, httpsUrl)) return invalid();
      const currency = input.nativeCurrency;
      if (!isObject(currency) || typeof currency.name !== 'string' || typeof currency.symbol !== 'string') {
        return invalid();
      }
      if (!Number.isSafeInteger(currency.decimals) || (currency.decimals as number) < 0) return invalid();
      if (!optional(input.blockExplorerUrls, (v) => list(v, 0, httpsUrl))) return invalid();
      if (!optional(input.iconUrls, (v) => list(v, 0, httpsUrl))) return invalid();
      return null;
    },
  },

  personal_sign: {
    check(params, state) {
      if (!Array.isArray(params) || params.length !== 2 || !data(params[0]) || !address(params[1])) return invalid();
      return isApproved(params[1], state) ? null : notApproved();
    },
  },

  eth_signTypedData_v4: {
    check(params, state) {
      if (!Array.isArray(params) || params.length !== 2 || !address(params[0])) return invalid();
      if (typeof params[1] !== 'string') return invalid('typedData must be a JSON string');
      let typed: unknown;
      try {
        typed = JSON.parse(params[1]);
      } catch {
        return invalid('typedData is not JSON');
      }
      if (!isObject(typed) || !isObject(typed.types) || typeof typed.primaryType !== 'string') return invalid();
      if (!Array.isArray(typed.types.EIP712Domain)) return invalid('types must include EIP712Domain');
      if (!typedDataShape(typed)) return invalid('Invalid typed data');
      if (!isObject(typed.domain) || !isObject(typed.message)) return invalid();
      if (!isApproved(params[0], state)) return notApproved();
      if (typed.domain.chainId !== undefined) {
        const chainId = parseChainId(typed.domain.chainId);
        if (chainId === null) return invalid('Invalid domain.chainId');
        if (chainId !== activeChainId(state)) return otherChain();
      }
      return null;
    },
  },

  eth_sendTransaction: {
    check(params, state) {
      const tx = single(params);
      if (!tx || !address(tx.from)) return invalid();
      if (!optional(tx.to, address) || !optional(tx.data, data) || !optional(tx.input, data)) return invalid();
      for (const field of ['value', 'gas', 'gasPrice', 'maxFeePerGas', 'maxPriorityFeePerGas', 'nonce', 'type']) {
        if (!optional(tx[field], quantity)) return invalid(`Invalid ${field}`);
      }
      if (!optional(tx.chainId, quantity)) return invalid('Invalid chainId');
      if (!optional(tx.accessList, accessList)) return invalid('Invalid accessList');
      if (!isApproved(tx.from, state)) return notApproved();
      if (tx.chainId !== undefined && BigInt(tx.chainId as string) !== activeChainId(state)) return otherChain();
      return null;
    },
  },

  wallet_sendCalls: {
    check(params, state) {
      const input = single(params);
      if (!input || typeof input.version !== 'string' || !chainId(input.chainId)) return invalid();
      if (typeof input.atomicRequired !== 'boolean') return invalid();
      if (!Array.isArray(input.calls) || input.calls.length === 0) return invalid();
      for (const call of input.calls) {
        if (!isObject(call)) return invalid();
        if (!optional(call.to, address) || !optional(call.data, data) || !optional(call.value, quantity)) {
          return invalid();
        }
      }
      if (!optional(input.id, (v) => typeof v === 'string') || !optional(input.capabilities, isObject)) {
        return invalid();
      }
      if (input.from !== undefined) {
        if (!address(input.from)) return invalid();
        if (!isApproved(input.from, state)) return notApproved();
      }
      return null;
    },
  },

  wallet_getCallsStatus: {
    readOnly: true,
    check(params) {
      return Array.isArray(params) && params.length === 1 && typeof params[0] === 'string' ? null : invalid();
    },
  },

  wallet_getCapabilities: {
    readOnly: true,
    check(params, state) {
      if (!Array.isArray(params) || params.length < 1 || params.length > 2 || !address(params[0])) return invalid();
      if (params.length === 2 && !(Array.isArray(params[1]) && params[1].every(quantity))) return invalid();
      return isApproved(params[0], state) ? null : notApproved();
    },
  },
};

export const evm: Network = {
  namespace: 'eip155',
  isChain: isEvmChain,
  isAccount: isEvmAccount,
  functions,
};
