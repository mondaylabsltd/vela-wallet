// What a Wallet does with a parsed request: steps 4–9 of core §3.4.
//
// Used by the reference Wallet in tests and checked against
// spec/test-vectors/core.json; a Wallet in any language follows the same steps.

import {
  ErrorCode,
  isId,
  parseConnectParams,
  rpcError,
  type ConnectParams,
  type Request,
  type RpcError,
  type SessionState,
} from './protocol';
import type { Network, WalletFunction } from './networks/types';

export type Disposition =
  | { action: 'repeat' }
  | { action: 'error'; error: RpcError }
  | { action: 'connect'; params: ConnectParams }
  | { action: 'cancel'; id: string }
  | { action: 'perform'; network: Network; fn: WalletFunction };

export interface WalletSession {
  /** The current session state; `{}` until the user approves. */
  state: SessionState;
  /** Whether a request with this id was already received in this session. */
  seen(id: string): boolean;
}

/** The namespace a function belongs to, among the given networks. */
export function networkOf(method: string, networks: readonly Network[]): Network | undefined {
  return networks.find((network) => Object.hasOwn(network.functions, method));
}

export function dispose(request: Request, session: WalletSession, networks: readonly Network[]): Disposition {
  const error = (code: number, message: string): Disposition => ({ action: 'error', error: rpcError(code, message) });
  const { method, params } = request;

  if (session.seen(request.id)) return { action: 'repeat' };

  if (method === 'walletpair_connect') {
    const parsed = parseConnectParams(params);
    return parsed ? { action: 'connect', params: parsed } : error(ErrorCode.InvalidParams, 'Invalid params');
  }
  if (method === 'walletpair_cancel') {
    if (!Array.isArray(params) || params.length !== 1 || !isId(params[0])) {
      return error(ErrorCode.InvalidParams, 'Invalid params');
    }
    return { action: 'cancel', id: params[0] };
  }

  if (Object.keys(session.state).length === 0) return error(ErrorCode.Unauthorized, 'Not connected');

  const network = networkOf(method, networks);
  if (!network) return error(ErrorCode.Unsupported, 'Unsupported method');
  const state = session.state[network.namespace];
  if (!state) return error(ErrorCode.Unauthorized, 'Namespace is not approved');

  const fn = network.functions[method];
  const refused = fn.check(params, state);
  return refused ? { action: 'error', error: refused } : { action: 'perform', network, fn };
}
