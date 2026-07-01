/**
 * RemoteInjectTransport — the SSE + POST bridge to a dApp relay (US 5.1).
 *
 * Exercised against a mock EventSource + capturing fetch so the real transport logic
 * (connect handshake, request routing, response/info POST shapes, teardown, reconnect)
 * is covered with zero network. Complements:
 *   - dapp-transport.test.ts   — parseRemoteInjectURL (the connect-link parser)
 *   - e2e/support/relay.mjs     — the relay side of the same wire protocol (smoke)
 */
import { RemoteInjectTransport, type RemoteInjectSession } from '@/services/dapp-transport';
import { NET_TIMEOUTS } from '@/services/net';
import {
  MockEventSource,
  installMockEventSource,
  installMockFetch,
  type MockFetchHandle,
} from '../support/mock-event-source';

const SESSION: RemoteInjectSession = {
  serverUrl: 'http://localhost:8788',
  sessionId: 'sid',
  nonce: 'n1',
  secret: 'k1',
};

const flush = () => Promise.resolve().then(() => Promise.resolve());

async function connected(t: RemoteInjectTransport) {
  const p = t.connect();
  MockEventSource.last().emit({ type: 'ready' });
  await p;
}

describe('RemoteInjectTransport', () => {
  let uninstallES: () => void;
  let fetchMock: MockFetchHandle;

  beforeEach(() => {
    uninstallES = installMockEventSource();
    fetchMock = installMockFetch();
  });
  afterEach(() => {
    uninstallES();
    fetchMock.restore();
  });

  it('opens the SSE with the correct authenticated URL and resolves on ready', async () => {
    const t = new RemoteInjectTransport(SESSION);
    const onConnected = jest.fn();
    t.on('connected', onConnected);

    const p = t.connect();
    expect(MockEventSource.last().url).toBe(
      'http://localhost:8788/sse?session=sid&role=mobile&n=n1&k=k1',
    );
    MockEventSource.last().emit({ type: 'ready' });
    await p;

    expect(t.connected).toBe(true);
    expect(onConnected).toHaveBeenCalledWith('Remote Bridge');
  });

  it('rejects when the SSE errors before authenticating', async () => {
    const t = new RemoteInjectTransport(SESSION);
    const p = t.connect();
    MockEventSource.last().fail();
    await expect(p).rejects.toThrow('Failed to connect to bridge');
    expect(t.connected).toBe(false);
  });

  it('rejects with a timeout if ready never arrives', async () => {
    jest.useFakeTimers();
    try {
      const t = new RemoteInjectTransport(SESSION);
      const p = t.connect();
      jest.advanceTimersByTime(NET_TIMEOUTS.sseOpen + 10);
      await expect(p).rejects.toThrow('Bridge connection timed out');
      expect(MockEventSource.last().closed).toBe(true);
    } finally {
      jest.useRealTimers();
    }
  });

  it('emits request events for incoming JSON-RPC and ignores malformed frames', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);

    const onReq = jest.fn();
    t.on('request', onReq);

    MockEventSource.last().emitRaw('this is not json'); // must not throw
    MockEventSource.last().emit({ type: 'request', id: '7', method: 'personal_sign', params: ['0xab'], origin: 'https://d.app' });

    expect(onReq).toHaveBeenCalledTimes(1);
    expect(onReq).toHaveBeenCalledWith('7', 'personal_sign', ['0xab'], 'https://d.app');
  });

  it('drops a request frame missing id or method', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);
    const onReq = jest.fn();
    t.on('request', onReq);
    MockEventSource.last().emit({ type: 'request', method: 'eth_chainId' }); // no id
    MockEventSource.last().emit({ type: 'request', id: '1' }); // no method
    expect(onReq).not.toHaveBeenCalled();
  });

  it('POSTs a well-formed response (result and error variants)', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);

    t.sendResponse('7', '0xdeadbeef');
    t.sendResponse('8', undefined, { code: 4001, message: 'User rejected' });
    await flush();

    const posts = fetchMock.posts();
    expect(posts[0].url).toBe('http://localhost:8788/message?session=sid&role=mobile&n=n1&k=k1');
    expect(posts[0].body).toEqual({ type: 'response', id: '7', result: '0xdeadbeef' });
    expect(posts[1].body).toEqual({ type: 'response', id: '8', error: { code: 4001, message: 'User rejected' } });
  });

  it('defaults a null result rather than omitting it', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);
    t.sendResponse('9'); // no result, no error
    await flush();
    expect(fetchMock.posts()[0].body).toEqual({ type: 'response', id: '9', result: null });
  });

  it('POSTs wallet info as a connect frame', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);
    t.pushWalletInfo({ address: '0xabc', chainId: 100, name: 'Acct', accounts: [] });
    await flush();
    expect(fetchMock.posts()[0].body).toEqual({ type: 'connect', address: '0xabc', chainId: 100 });
  });

  it('fetches dApp metadata, and returns null on 404 or partial metadata', async () => {
    const t = new RemoteInjectTransport(SESSION);

    fetchMock.setJson({ metadata: { name: 'Test dApp', url: 'https://d.app', icon: 'i' } });
    const info = await t.fetchDAppInfo();
    expect(info).toEqual({ name: 'Test dApp', url: 'https://d.app', icon: 'i' });
    expect(fetchMock.calls.some((c) => c.url === 'http://localhost:8788/session/sid?n=n1&k=k1')).toBe(true);

    fetchMock.setStatus(404);
    expect(await t.fetchDAppInfo()).toBeNull();

    fetchMock.setStatus(200);
    fetchMock.setJson({ metadata: { name: 'Only name' } }); // missing url
    expect(await t.fetchDAppInfo()).toBeNull();
  });

  it('tears down on disconnect and notifies once', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);
    const es = MockEventSource.last();
    const onDisc = jest.fn();
    t.on('disconnected', onDisc);

    t.disconnect();
    await flush();

    expect(es.closed).toBe(true);
    expect(t.connected).toBe(false);
    expect(fetchMock.posts().some((p) => p.body.type === 'disconnect')).toBe(true);
    expect(onDisc).toHaveBeenCalled();
  });

  it('surfaces relay disconnect and error frames as events', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);
    const onDisc = jest.fn();
    const onErr = jest.fn();
    t.on('disconnected', onDisc);
    t.on('error', onErr);

    MockEventSource.last().emit({ type: 'error', message: 'relay exploded' });
    expect(onErr).toHaveBeenCalledWith('relay exploded');

    MockEventSource.last().emit({ type: 'disconnect' });
    expect(onDisc).toHaveBeenCalled();
  });

  it('emits disconnected when the stream drops after connecting', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);
    const onDisc = jest.fn();
    t.on('disconnected', onDisc);

    MockEventSource.last().fail(); // post-connect error → lost connection
    expect(t.connected).toBe(false);
    expect(onDisc).toHaveBeenCalled();
  });

  it('unsubscribes listeners via the returned disposer', async () => {
    const t = new RemoteInjectTransport(SESSION);
    await connected(t);
    const onReq = jest.fn();
    const off = t.on('request', onReq);
    off();
    MockEventSource.last().emit({ type: 'request', id: '1', method: 'eth_chainId', params: [] });
    expect(onReq).not.toHaveBeenCalled();
  });
});
