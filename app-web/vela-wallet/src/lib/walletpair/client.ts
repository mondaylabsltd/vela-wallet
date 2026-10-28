// The DApp side of a WalletPair session over a PeerConnect connection:
// calls, walletpair_connect, the state event, cancellation, and resending
// after a break (spec/core.md §4–§6).

import {
  ErrorCode,
  encode,
  parseConnectResult,
  parseFromWallet,
  parseState,
  rpcError,
  type ConnectResult,
  type Json,
  type NamespaceRules,
  type Params,
  type Request,
  type Response,
  type RpcError,
  type SessionState,
  type WalletInfo,
} from './protocol';

/** What the client needs from a PeerConnect `Connection`. */
export interface Transport extends EventTarget {
  readonly state: 'offline' | 'waiting' | 'connected' | 'closed';
  send(data: Uint8Array): number;
}

export class CallError extends Error {
  readonly code: number;
  readonly data?: Json;

  constructor(error: RpcError) {
    super(error.message);
    this.name = 'CallError';
    this.code = error.code;
    this.data = error.data;
  }
}

export interface DAppInfo {
  name: string;
  url?: string;
  icon?: string;
}

interface Pending {
  request: Request;
  resolve(result: Json): void;
  reject(error: CallError): void;
}

/**
 * Events: `state` (session state or wallet changed), `pending` (the set of
 * pending request ids changed; persist it to cancel them after a restart),
 * `ended` (the room closed).
 */
export class WalletPairClient extends EventTarget {
  /** The session state; `{}` until approved (core §4.2). */
  state: SessionState = {};
  wallet: WalletInfo | undefined;
  ended = false;

  readonly #transport: Transport;
  readonly #rules: Record<string, NamespaceRules>;
  readonly #pending = new Map<string, Pending>();
  #wasConnected = false;
  #connected = false;

  constructor(transport: Transport, rules: Record<string, NamespaceRules>) {
    super();
    this.#transport = transport;
    this.#rules = rules;
    transport.addEventListener('message', (event) => this.#receive((event as MessageEvent<Uint8Array>).data));
    transport.addEventListener('state', () => this.#transportState());
    this.#transportState();
  }

  get approved(): boolean {
    return Object.keys(this.state).length > 0;
  }

  /** Ids of pending network calls: what to cancel after a restart (core §5). */
  get pendingIds(): string[] {
    return [...this.#pending.values()]
      .filter(({ request }) => !request.method.startsWith('walletpair_'))
      .map(({ request }) => request.id);
  }

  /** Asks for the namespaces; resolves with the Wallet's current state (core §4.1). */
  async connect(dapp: DAppInfo, namespaces: string[], origin?: string): Promise<ConnectResult> {
    // The state was taken from the result when it arrived (#receive).
    return (await this.call('walletpair_connect', [{ ...dapp, namespaces }], origin)) as unknown as ConnectResult;
  }

  /** Calls a function on the Wallet. Rejects with a CallError carrying its code. */
  call(method: string, params?: Params, origin?: string): Promise<Json> {
    return this.start(method, params, origin).result;
  }

  /** Like `call`, but also returns the request id, for `cancel`. */
  start(method: string, params?: Params, origin?: string): { id: string; result: Promise<Json> } {
    const request: Request = { id: crypto.randomUUID(), method };
    if (params !== undefined) request.params = params;
    if (origin !== undefined) request.origin = origin;
    const result = new Promise<Json>((resolve, reject) => {
      if (this.ended) {
        reject(new CallError(rpcError(ErrorCode.Disconnected, 'Session ended')));
        return;
      }
      try {
        this.#transport.send(encode(request));
      } catch (error) {
        reject(new CallError(rpcError(ErrorCode.Internal, `Cannot send: ${(error as Error).message}`)));
        return;
      }
      this.#pending.set(request.id, { request, resolve, reject });
      this.dispatchEvent(new Event('pending'));
    });
    return { id: request.id, result };
  }

  /**
   * Withdraws calls (core §5): the Wallet removes the prompt and answers 4001,
   * unless it had already answered, and then its answer stands, so a call is
   * settled by the Wallet, never here. Also accepts ids from before a restart.
   */
  cancel(ids: string[]): void {
    if (this.ended) return;
    for (const id of ids) this.call('walletpair_cancel', [id]).catch(() => {});
  }

  /** Ends the session from this side: pending calls are answered with 4900 at once (core §4.5). */
  close(): void {
    this.#end();
  }

  /** Ids of pending calls whose request matches. */
  find(match: (request: Request) => boolean): string[] {
    return [...this.#pending.values()].filter((p) => match(p.request)).map((p) => p.request.id);
  }

  #receive(bytes: Uint8Array): void {
    const message = parseFromWallet(bytes);
    if (message.kind === 'response') {
      const { response } = message;
      const pending = this.#pending.get(response.id);
      if (!pending) return;
      this.#pending.delete(response.id);
      this.dispatchEvent(new Event('pending'));
      if (pending.request.method === 'walletpair_connect') return this.#connectAnswered(pending, response);
      if ('error' in response) pending.reject(new CallError(response.error));
      else pending.resolve(response.result);
    } else if (message.kind === 'event' && message.event.event === 'state') {
      const state = parseState(message.event.data, this.#rules);
      if (state) this.#setState(state);
    }
  }

  /**
   * A walletpair_connect answer changes the state as it arrives, before any
   * event delivered after it (core §4.3); an error leaves the session
   * unapproved (core §4.5).
   */
  #connectAnswered(pending: Pending, response: Response): void {
    if ('error' in response) {
      this.#setState({});
      return pending.reject(new CallError(response.error));
    }
    const result = parseConnectResult(response.result, this.#rules);
    if (!result) return pending.reject(new CallError(rpcError(ErrorCode.Internal, 'Invalid walletpair_connect result')));
    this.wallet = result.wallet;
    this.#setState(result.state);
    pending.resolve(result as unknown as Json);
  }

  #setState(state: SessionState): void {
    this.state = state;
    this.dispatchEvent(new Event('state'));
  }

  #transportState(): void {
    const state = this.#transport.state;
    if (state === 'closed') {
      this.#end();
      return;
    }
    const connected = state === 'connected';
    if (connected && !this.#connected) {
      // Back after a break: the Wallet may have restarted and lost responses (core §6).
      if (this.#wasConnected) {
        for (const { request } of this.#pending.values()) {
          try {
            this.#transport.send(encode(request));
          } catch {
            // The room is closing; #end will answer everything.
          }
        }
      }
      this.#wasConnected = true;
    }
    this.#connected = connected;
  }

  #end(): void {
    if (this.ended) return;
    this.ended = true;
    for (const pending of this.#pending.values()) {
      pending.reject(new CallError(rpcError(ErrorCode.Disconnected, 'Session ended')));
    }
    this.#pending.clear();
    this.dispatchEvent(new Event('pending'));
    this.dispatchEvent(new Event('ended'));
  }
}
