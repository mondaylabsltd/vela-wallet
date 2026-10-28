// WalletPair's reference Wallet: the Wallet side of spec/core.md, with per-network signers and prompts supplied by
// whoever runs it. sub3.dev/connect runs it with a signer that forwards to the wallet app's injected provider.
//
// sub3 change (beginEffect): a signer that hands a request to another wallet cannot record the response before the
// effect, as core §6 asks: the wallet app broadcasts a transaction and only then returns its hash. Such a signer
// calls `beginEffect()` before handing the request over. If the page stops before the answer, the request is
// recorded on restart as answered with -32603 ("outcome unknown") instead of being asked again, so it is never
// performed twice; the DApp side then checks the chain (sub3's checkout pays with an order id that can succeed
// once).

import type { Transport } from './client';
import { dispose } from './dispatch';
import type { Network } from './networks/types';
import {
  ErrorCode,
  encode,
  parseRequest,
  rpcError,
  type ConnectParams,
  type Json,
  type NamespaceState,
  type Params,
  type Request,
  type Response,
  type RpcError,
  type SessionState,
} from './protocol';

/** Thrown by a signer to answer with an error. */
export class WalletError extends Error {
  constructor(
    readonly code: number,
    message: string,
  ) {
    super(message);
  }
}

export interface PerformContext {
  request: Request;
  state: NamespaceState;
  /** Changes this namespace's active chain; sends the state event (core §4.3). */
  setChain(chain: string): void;
  /** Runs `effect` once the response is recorded durably: what cannot be undone, such as broadcasting, goes here (core §6). */
  afterRecorded(effect: () => void): void;
  /** Marks, durably, that the request is being handed to something that may act on it before answering (sub3). */
  beginEffect(): void;
}

/** One network's keys and behaviour. */
export interface Signer {
  network: Network;
  chain: string;
  accounts: string[];
  perform(method: string, params: Params | undefined, context: PerformContext): Promise<Json>;
}

/** The user, scripted by the test. */
export interface Prompts {
  /** Which of the offered namespaces the user approves; none declines. `cancelled` aborts if the room closes. */
  connect(params: ConnectParams, offered: string[], origin: string | undefined, cancelled: AbortSignal): Promise<string[]>;
  /** Whether the user approves a request. `cancelled` aborts if the DApp side withdraws it (core §5). */
  approve(request: Request, cancelled: AbortSignal): Promise<boolean>;
}

/** What survives a restart (core §6). */
export interface WalletStore {
  approved: string[];
  chains: Record<string, string>;
  /** Recorded requests: the response, or null while in progress. */
  records: Map<string, Response | null>;
  /** Requests handed over by `beginEffect` and not answered yet (sub3). */
  forwarded: Set<string>;
}

export function emptyStore(): WalletStore {
  return { approved: [], chains: {}, records: new Map(), forwarded: new Set() };
}

/** The answer to a request whose outcome was lost with the page (sub3). */
export const OUTCOME_UNKNOWN = 'The wallet may have acted on this request before the page stopped; check before retrying';

export interface WalletOptions {
  name: string;
  icon?: string;
  signers: Signer[];
  prompts: Prompts;
  store?: WalletStore;
  /** Requests that may wait for the user at once before -32005. */
  maxWaiting?: number;
  /** Makes the store durable. Called after every change to it, before what depends on it (core §6). */
  save?(): void;
}

export class ReferenceWallet {
  readonly store: WalletStore;
  /** Every request received, for assertions. */
  readonly received: Request[] = [];

  readonly #transport: Transport;
  readonly #options: WalletOptions;
  readonly #networks: Network[];
  readonly #waiting = new Map<string, () => void>();
  #connectPrompt: Promise<void> | undefined;
  #connectCancel: AbortController | undefined;
  #connectWaiters: Request[] = [];
  /** walletpair_connect ids already answered: a repeat is answered without asking again (core §6). */
  readonly #answeredConnects = new Set<string>();
  #closed = false;

  constructor(transport: Transport, options: WalletOptions) {
    this.#transport = transport;
    this.#options = options;
    this.#networks = options.signers.map((s) => s.network);
    this.store = options.store ?? emptyStore();
    for (const signer of options.signers) this.store.chains[signer.network.namespace] ??= signer.chain;
    // A request without a response was waiting for the user when the Wallet stopped. Nothing
    // was done for it, since responses are recorded before any effect, so a repeat asks again (core §6).
    for (const [id, response] of this.store.records) {
      if (response !== null) continue;
      if (this.store.forwarded.has(id)) {
        this.store.records.set(id, { id, error: rpcError(ErrorCode.Internal, OUTCOME_UNKNOWN) });
      } else {
        this.store.records.delete(id);
      }
    }
    this.store.forwarded.clear();
    transport.addEventListener('message', (event) => {
      void this.#receive((event as MessageEvent<Uint8Array>).data);
    });
    // A closed room ends the session: cancel the prompts, forget approval and records (core §2.4, §4.5).
    transport.addEventListener('state', () => {
      // Leaving (the app stopping) keeps the session; only a close ends it.
      if (transport.state !== 'closed' || (transport as { reason?: string }).reason === 'left') return;
      this.#closed = true;
      for (const cancel of [...this.#waiting.values()]) cancel();
      this.#connectCancel?.abort();
      this.store.approved = [];
      this.store.records.clear();
      this.store.forwarded.clear();
      this.#save();
    });
    // After a restart, queued events may have been lost, whatever the state (core §4.3).
    this.#sendState();
  }

  get state(): SessionState {
    const state: SessionState = {};
    for (const namespace of this.store.approved) {
      const signer = this.#signer(namespace);
      if (signer) state[namespace] = { chain: this.store.chains[namespace], accounts: [...signer.accounts] };
    }
    return state;
  }

  /** The user switches chains in the Wallet. */
  setChain(namespace: string, chain: string): void {
    this.store.chains[namespace] = chain;
    this.#save();
    if (this.store.approved.includes(namespace)) this.#sendState();
  }

  /** Sends the current state again, after the signer's accounts changed (sub3). */
  refresh(): void {
    this.#save();
    this.#sendState();
  }

  /** The user revokes the session's approval without ending it. */
  revoke(): void {
    this.store.approved = [];
    this.#save();
    this.#sendState();
  }

  /** Ids of requests waiting for the user. */
  get waiting(): string[] {
    return [...this.#waiting.keys()];
  }

  async #receive(bytes: Uint8Array): Promise<void> {
    const inbound = parseRequest(bytes);
    if (inbound.kind === 'ignore') return;
    if (inbound.kind === 'invalid') {
      return this.#send({ id: inbound.id, error: rpcError(ErrorCode.InvalidRequest, 'Invalid request') });
    }
    const { request } = inbound;
    this.received.push(request);
    const records = this.store.records;
    const disposition = dispose(request, { state: this.state, seen: (id) => records.has(id) }, this.#networks);

    switch (disposition.action) {
      case 'repeat': {
        const recorded = records.get(request.id);
        if (recorded) this.#send(recorded);
        return;
      }
      case 'error':
        return this.#send({ id: request.id, error: disposition.error });
      case 'connect':
        return this.#connect(request, disposition.params);
      case 'cancel':
        this.#waiting.get(disposition.id)?.();
        return this.#send({ id: request.id, result: null });
      case 'perform': {
        const { network, fn } = disposition;
        const signer = this.#signer(network.namespace)!;
        if (fn.readOnly) return this.#send(await this.#perform(signer, request));
        if (this.#waiting.size >= (this.#options.maxWaiting ?? 16)) {
          return this.#send({ id: request.id, error: rpcError(ErrorCode.LimitExceeded, 'Too many requests') });
        }
        records.set(request.id, null);
        this.#save();
        const shown = this.state[network.namespace];
        const approved = await this.#ask(request);
        if (this.#closed) return;
        const effects: (() => void)[] = [];
        let response: Response;
        if (!approved) {
          response = { id: request.id, error: rpcError(ErrorCode.UserRejected, 'User rejected the request') };
        } else {
          // Performed against the state the user saw: if it changed while they decided, nothing is done (core §8).
          const now = this.state[network.namespace];
          const refused = !now
            ? rpcError(ErrorCode.Unauthorized, 'No longer approved')
            : now.chain !== shown.chain
              ? rpcError(ErrorCode.ChainMismatch, 'The active chain changed')
              : fn.check(request.params, now);
          response = refused ? { id: request.id, error: refused } : await this.#perform(signer, request, effects);
        }
        records.set(request.id, response);
        this.store.forwarded.delete(request.id);
        this.#save();
        for (const effect of effects) effect();
        return this.#send(response);
      }
    }
  }

  async #perform(signer: Signer, request: Request, effects: (() => void)[] = []): Promise<Response> {
    const namespace = signer.network.namespace;
    const context: PerformContext = {
      request,
      state: this.state[namespace],
      setChain: (chain) => this.setChain(namespace, chain),
      afterRecorded: (effect) => effects.push(effect),
      beginEffect: () => {
        this.store.forwarded.add(request.id);
        this.#save();
      },
    };
    try {
      return { id: request.id, result: await signer.perform(request.method, request.params, context) };
    } catch (error) {
      const code = error instanceof WalletError ? error.code : ErrorCode.Internal;
      return { id: request.id, error: rpcError(code, (error as Error).message) };
    }
  }

  /** Waits for the user; cancellation (core §5) answers false. */
  #ask(request: Request): Promise<boolean> {
    return new Promise((resolve) => {
      const cancelled = new AbortController();
      let done = false;
      const finish = (answer: boolean) => {
        if (done) return;
        done = true;
        this.#waiting.delete(request.id);
        resolve(answer);
      };
      this.#waiting.set(request.id, () => {
        cancelled.abort();
        finish(false);
      });
      // Messages delivered together are dispatched one after another, so a cancellation that came
      // with its request is handled before this runs, and no prompt appears (core §5).
      queueMicrotask(() => {
        if (!done) this.#options.prompts.approve(request, cancelled.signal).then(finish, () => finish(false));
      });
    });
  }

  async #connect(request: Request, params: ConnectParams): Promise<void> {
    const supported = this.#networks.map((n) => n.namespace);
    const offered = params.namespaces.filter((ns) => supported.includes(ns));
    if (offered.length === 0) {
      return this.#send({ id: request.id, error: rpcError(ErrorCode.Unsupported, 'No requested namespace is supported') });
    }
    const missing = offered.filter((ns) => !this.store.approved.includes(ns));
    if (missing.length === 0 || this.#answeredConnects.has(request.id)) return this.#answerConnect(request);

    // One prompt per session; everyone waiting gets its outcome (core §4.1).
    this.#connectWaiters.push(request);
    this.#connectPrompt ??= (async () => {
      const cancelled = new AbortController();
      this.#connectCancel = cancelled;
      let granted: string[] = [];
      try {
        granted = await this.#options.prompts.connect(params, missing, request.origin, cancelled.signal);
      } catch {
        // A failed prompt declines.
      }
      this.#connectCancel = undefined;
      this.#connectPrompt = undefined;
      if (this.#closed) return;
      const added = granted.filter((ns) => missing.includes(ns) && !this.store.approved.includes(ns));
      if (added.length) {
        this.store.approved.push(...added);
        this.#save();
        // The state changed: the event goes before the responses (core §4.3).
        this.#sendState();
      }
      const waiters = this.#connectWaiters;
      this.#connectWaiters = [];
      for (const waiter of waiters) this.#answerConnect(waiter);
    })();
  }

  #answerConnect(request: Request): void {
    this.#answeredConnects.add(request.id);
    const state = this.state;
    if (Object.keys(state).length === 0) {
      return this.#send({ id: request.id, error: rpcError(ErrorCode.UserRejected, 'User rejected the request') });
    }
    const wallet: { name: string; icon?: string } = { name: this.#options.name };
    if (this.#options.icon) wallet.icon = this.#options.icon;
    this.#send({ id: request.id, result: { wallet, state } as unknown as Json });
  }

  #sendState(): void {
    this.#send({ event: 'state', data: this.state as unknown as Json });
  }

  #save(): void {
    this.#options.save?.();
  }

  #signer(namespace: string): Signer | undefined {
    return this.#options.signers.find((s) => s.network.namespace === namespace);
  }

  #send(message: Response | { event: string; data: Json }): void {
    try {
      this.#transport.send(encode(message));
    } catch {
      // The room has closed.
    }
  }
}

export type { RpcError };
