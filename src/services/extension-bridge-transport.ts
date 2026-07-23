/**
 * ExtensionBridgeTransport — the third DAppTransport (next to RemoteInjectTransport
 * and WalletPairTransport), carrying SIGNING requests from the iOS Safari extension.
 *
 * Same-device, one-shot per rid: the extension launched the app via
 * velawallet://sign?rid and wrote sign-req-<rid>.json into the App Group; this
 * transport reads it, emits a single 'request' (which the global
 * <SigningRequestModal> renders for free), and on sendResponse writes the FROZEN
 * sign-result-<rid>.json the extension's content script polls on return.
 *
 * It reuses the app's whole signing pipeline (clear-signing, asset-sim, gas card,
 * approval-guard, passkey, bundler) unchanged — what is new is only the same-device
 * mailbox (App Group files) that replaces WalletPair's WS relay. It is NOT routed
 * through WalletPair's cross-device, long-lived session protocol.
 *
 * Fund-safety contract (docs/safari-extension/ARCHITECTURE.md §12.1.3 / §4):
 *   - ONLY 'submitted' | 'rejected' ever reach disk.
 *   - code 4001 (explicit reject) → {status:'rejected'}.
 *   - any NON-4001 error (passkey cancel, funding cancel, unsupported chain, RPC
 *     failure, …) → write NOTHING → the page falls to the recoverable 4900
 *     "check Vela" state, never a 4001 false-decline (which a dApp retries →
 *     double-spend).
 *   - stays `connected` from connect() through the result write; the
 *     'disconnected' event fires ONLY after the write completes (an early
 *     disconnect would clear incomingRequest mid-sign).
 */
import * as AppGroup from '@/modules/app-group';
import type {
  DAppTransport,
  DAppTransportEvents,
  WalletInfo,
  DAppInfo,
} from './dapp-transport';

/** The on-disk request the extension handed off (frozen shape + additive chainId). */
export interface ExtSignRequest {
  rid: string;
  method: string;
  params: unknown[];
  origin: string;
  ts: number;
  /** ADDITIVE (Phase B): the origin's granted chainId — the chain to sign against. */
  chainId?: number;
  /** ADDITIVE (§12.1.6): the origin's granted address — the account the dApp is
   *  connected to. The app reconciles the active account to THIS before signing so
   *  it never silently signs from a different account the user switched to. */
  address?: string;
}

/** The frozen result the app writes back for the extension to poll. */
interface ExtSignResult {
  rid: string;
  status: 'submitted' | 'rejected';
  userOpHash: string; // a tx/userOp hash for sends; the EIP-1271 signature hex for signatures
  ts: number;
}

function hostOf(origin: string): string {
  try {
    return new URL(origin).host || origin;
  } catch {
    return origin;
  }
}

/** Request-payload TTL (§12.1.4): a sign-req older than this is stale and must
 *  never be signed (decoupled from the result TTL, which persists for hours). */
const REQUEST_TTL_MS = 5 * 60 * 1000;

export class ExtensionBridgeTransport implements DAppTransport {
  readonly name = 'Safari Extension';

  private _connected = false;
  private _settled = false; // sendResponse is idempotent — one result per rid
  private _outcome: 'submitted' | 'rejected' | 'unknown' = 'unknown';
  private listeners = new Map<string, Set<Function>>();
  private request: ExtSignRequest | null = null;

  constructor(private readonly rid: string) {}

  get connected(): boolean {
    return this._connected;
  }

  /** The settled outcome, read by sign.tsx on 'disconnected' to pick the copy. */
  get outcome(): 'submitted' | 'rejected' | 'unknown' {
    return this._outcome;
  }

  /** The chain the extension says to sign against (per-request, from the sign-req). */
  get requestChainId(): number | undefined {
    return this.request?.chainId;
  }

  /** The dApp origin that requested the sign (for the "Return to Safari" link). */
  get requestOrigin(): string | undefined {
    return this.request?.origin;
  }

  /** The address the origin was granted (§12.1.6) — the account to sign from. */
  get requestAddress(): string | undefined {
    return this.request?.address;
  }

  /** True when connect() short-circuited because this rid was already signed. */
  private _alreadySettled = false;
  get alreadySettled(): boolean {
    return this._alreadySettled;
  }

  /** Read the handed-off request (racing the cold-launch write) and emit it. */
  async connect(): Promise<void> {
    // ANTI-DOUBLE-SUBMIT (§12.5 gate d). The sign-req is never consumed, so a cold
    // relaunch of the SAME rid — the app was killed after submitting, or the user
    // taps "返回 Vela" while the result is still in transit — would otherwise re-read
    // it and render the signing modal AGAIN, letting the user approve the SAME tx
    // twice. If a result already exists for this rid, the sign is DONE: replay its
    // outcome, never re-emit the request. (The content-side focus-poll delivers the
    // already-written result to the dApp; the app must not re-sign.)
    const prior = await this.readExistingResult();
    if (prior) {
      this._outcome = prior.status;
      this._settled = true; // block any later sendResponse from rewriting the result
      this._alreadySettled = true;
      // Surface it to sign.tsx WITHOUT a 'request' (no modal). It reads `outcome`
      // in connect().then and shows the settled state; no signing UI ever renders.
      return;
    }

    const req = await this.readSignRequest();
    if (!req) {
      // No payload within the window — surface as an error, never a phantom sign.
      this.emit('error', 'Sign request not found or expired');
      throw new Error('sign-req not found');
    }
    // Request-payload TTL (§12.1.4): never sign a stale request (a leaked/replayed
    // rid whose payload has aged past the window). Result TTL is separate (hours).
    if (typeof req.ts === 'number' && Date.now() - req.ts > REQUEST_TTL_MS) {
      this.emit('error', 'Sign request expired');
      throw new Error('sign-req expired');
    }
    this.request = req;
    this._connected = true;
    this.emit('connected', this.name);
    // The global SigningRequestModal renders the moment incomingRequest is set by
    // the provider's handleIncoming (fired here). rid IS the request id.
    this.emit('request', req.rid, req.method, req.params as any[], req.origin);
  }

  /** A frozen result already written for this rid (the sign completed on a prior
   *  launch), or null. Used to short-circuit a re-launch and prevent re-signing. */
  private async readExistingResult(): Promise<ExtSignResult | null> {
    try {
      const json = await AppGroup.readFile(`sign-result-${this.rid}.json`);
      if (!json) return null;
      const r = JSON.parse(json) as ExtSignResult;
      return r && (r.status === 'submitted' || r.status === 'rejected') ? r : null;
    } catch {
      return null;
    }
  }

  /**
   * Write the result the extension polls for. SYNC void (interface) → fire-and-forget
   * async write, mirroring RemoteInjectTransport.sendResponse. Idempotent via _settled.
   */
  sendResponse(id: string, result?: any, error?: { code: number; message: string }): void {
    if (this._settled) return;
    this._settled = true;
    void this.writeResult(result, error).finally(() => {
      // Only AFTER the write (or its skip) do we go disconnected — never before,
      // or incomingRequest would clear mid-sign.
      this._connected = false;
      this.emit('disconnected');
    });
  }

  private async writeResult(result?: any, error?: { code: number; message: string }): Promise<void> {
    let payload: ExtSignResult | null = null;
    if (error) {
      // ONLY an explicit user reject (4001) becomes a durable 'rejected'. Every
      // other error writes NOTHING → the page's poll times out to 4900 (check
      // Vela), which is recoverable and never a false decline.
      if (error.code === 4001) {
        payload = { rid: this.rid, status: 'rejected', userOpHash: '0x', ts: Date.now() };
      }
    } else {
      // Success: `result` is a tx/userOp hash (sends) or an EIP-1271 signature hex
      // (personal_sign / typed-data). Either way it rides in userOpHash; content.js
      // resolves the dApp's promise with it verbatim.
      payload = {
        rid: this.rid,
        status: 'submitted',
        userOpHash: typeof result === 'string' ? result : String(result ?? '0x'),
        ts: Date.now(),
      };
    }
    this._outcome = payload ? payload.status : 'unknown';
    if (!payload) return; // non-4001 error → no file (recoverable 4900)
    try {
      await AppGroup.writeFile(`sign-result-${this.rid}.json`, JSON.stringify(payload));
    } catch (e) {
      // Off-iOS this throws; on-device a failed write leaves the page to recover
      // via the 4900 path + Vela Activity. Never crash the sign.
      console.warn('[ExtensionBridge] result write failed:', e);
    }
  }

  /** No live channel back to the page — the extension pulls state on focus. */
  pushWalletInfo(_info: WalletInfo): void {
    /* no-op */
  }

  /** Per-request dApp identity is the origin (there is no relay session metadata). */
  async fetchDAppInfo(): Promise<DAppInfo | null> {
    if (!this.request) return null;
    return { name: hostOf(this.request.origin), url: this.request.origin };
  }

  disconnect(): void {
    // No emit: the completion 'disconnected' is fired by sendResponse after the
    // write. An external disconnect just tears down without a spurious event.
    this._connected = false;
  }

  on<K extends keyof DAppTransportEvents>(event: K, listener: DAppTransportEvents[K]): () => void {
    let set = this.listeners.get(event);
    if (!set) this.listeners.set(event, (set = new Set()));
    set.add(listener as Function);
    return () => {
      this.listeners.get(event)?.delete(listener as Function);
    };
  }

  private emit(event: string, ...args: any[]): void {
    const set = this.listeners.get(event);
    if (!set) return;
    for (const fn of [...set]) {
      try {
        (fn as Function)(...args);
      } catch (e) {
        console.warn(`[ExtensionBridge] listener error for ${event}:`, e);
      }
    }
  }

  // Poll the App Group for the handed-off request. The content→background→native
  // write races the cold app launch, so a soft miss (null) is a retry signal.
  private async readSignRequest(intervalMs = 150, timeoutMs = 3000): Promise<ExtSignRequest | null> {
    const name = `sign-req-${this.rid}.json`;
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const json = await AppGroup.readFile(name);
      if (json) {
        try {
          return JSON.parse(json) as ExtSignRequest;
        } catch {
          return null; // present but corrupt — don't spin
        }
      }
      await new Promise((r) => setTimeout(r, intervalMs));
    }
    return null;
  }
}
