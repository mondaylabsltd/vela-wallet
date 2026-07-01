/**
 * Test doubles for the RemoteInject transport's IO: a fake `EventSource` and a
 * capturing `fetch`. Installed on the global so `RemoteInjectTransport` exercises its
 * real logic against controllable IO.
 */

export class MockEventSource {
  static instances: MockEventSource[] = [];

  url: string;
  readyState = 0;
  closed = false;
  onmessage: ((e: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockEventSource.instances.push(this);
  }

  close(): void { this.closed = true; }

  /** Push a JSON message to the transport (as the relay would over SSE). */
  emit(obj: unknown): void { this.onmessage?.({ data: JSON.stringify(obj) }); }
  /** Push raw (possibly malformed) SSE data. */
  emitRaw(data: string): void { this.onmessage?.({ data }); }
  /** Fire the transport's error handler (connection failed / dropped). */
  fail(): void { this.onerror?.(); }

  static last(): MockEventSource { return this.instances[this.instances.length - 1]; }
  static reset(): void { this.instances = []; }
}

export interface CapturedFetch {
  url: string;
  method: string;
  body: any;
}

export interface MockFetchHandle {
  calls: CapturedFetch[];
  /** POSTs to /message only. */
  posts(): CapturedFetch[];
  /** Set the JSON body returned by the next (and subsequent) responses. */
  setJson(json: unknown): void;
  /** Make responses return the given HTTP status (default 200/ok). */
  setStatus(status: number): void;
  restore(): void;
}

/** Install a capturing `fetch`. Returns a handle to inspect calls + shape responses. */
export function installMockFetch(): MockFetchHandle {
  const calls: CapturedFetch[] = [];
  let json: unknown = {};
  let status = 200;
  const prev = (global as any).fetch;

  (global as any).fetch = async (url: string, init: RequestInit = {}) => {
    calls.push({
      url,
      method: (init.method ?? 'GET').toUpperCase(),
      body: init.body ? JSON.parse(init.body as string) : undefined,
    });
    return {
      ok: status >= 200 && status < 300,
      status,
      json: async () => json,
    } as Response;
  };

  return {
    calls,
    posts: () => calls.filter((c) => c.method === 'POST' && c.url.includes('/message')),
    setJson: (j) => { json = j; },
    setStatus: (s) => { status = s; },
    restore: () => { (global as any).fetch = prev; },
  };
}

/** Install the mock EventSource on the global; returns an uninstaller. */
export function installMockEventSource(): () => void {
  const prev = (global as any).EventSource;
  MockEventSource.reset();
  (global as any).EventSource = MockEventSource;
  return () => { (global as any).EventSource = prev; };
}
