import { WebPopupTransport, isAllowedWebDAppOrigin } from '@/services/web-popup-transport';
import { VELA_WEB_CHANNEL, VELA_WEB_RESPONSE } from '../../../packages/vela-sdk/src/protocol';

function makePeer() {
  const messages: unknown[] = [];
  let closes = 0;
  const port = {
    postMessage(value: unknown) { messages.push(value); },
    close() { closes += 1; },
  } as unknown as MessagePort;
  return {
    messages,
    get closes() { return closes; },
    peer: {
      sessionId: 'session-1',
      origin: 'https://app.example',
      dapp: { name: 'Example', url: 'https://app.example' },
      request: { id: 'rpc-1', method: 'personal_sign', params: ['0x01'], chainId: 8453, address: '0xabc' },
      port,
    },
  };
}

describe('WebPopupTransport', () => {
  test('emits the origin-bound request and exposes per-request identity', async () => {
    const fixture = makePeer();
    const transport = new WebPopupTransport(fixture.peer);
    const requests: unknown[][] = [];
    transport.on('request', (...args) => requests.push(args));

    await transport.connect();

    expect(transport.connected).toBe(true);
    expect(transport.requestChainId).toBe(8453);
    expect(transport.requestAddress).toBe('0xabc');
    expect(requests).toEqual([['rpc-1', 'personal_sign', ['0x01'], 'https://app.example']]);
  });

  test('posts exactly one successful response then closes the capability port', () => {
    const fixture = makePeer();
    const transport = new WebPopupTransport(fixture.peer);
    transport.sendResponse('rpc-1', '0xsigned');
    transport.sendResponse('rpc-1', '0xduplicate');

    expect(fixture.messages).toEqual([{
      channel: VELA_WEB_CHANNEL,
      type: VELA_WEB_RESPONSE,
      sessionId: 'session-1',
      id: 'rpc-1',
      result: '0xsigned',
    }]);
    expect(fixture.closes).toBe(1);
    expect(transport.connected).toBe(false);
  });

  test('disconnect is an explicit 4001 rejection', () => {
    const fixture = makePeer();
    const transport = new WebPopupTransport(fixture.peer);
    transport.disconnect();
    expect(fixture.messages).toEqual([expect.objectContaining({ error: { code: 4001, message: 'User rejected the request' } })]);
  });
});
describe('isAllowedWebDAppOrigin', () => {
  test.each(['https://app.example', 'https://localhost', 'http://localhost:3000', 'http://127.0.0.1:5173'])('allows %s', (origin) => {
    expect(isAllowedWebDAppOrigin(origin)).toBe(true);
  });

  test.each(['http://app.example', 'javascript:alert(1)', 'null', 'not a URL'])('rejects %s', (origin) => {
    expect(isAllowedWebDAppOrigin(origin)).toBe(false);
  });
});
