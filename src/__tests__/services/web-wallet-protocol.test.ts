import {
  VELA_WEB_CHANNEL,
  VELA_WEB_INIT,
  VELA_WEB_READY,
  VELA_WEB_RESPONSE,
  isVelaWebInit,
  isVelaWebReady,
  isVelaWebResponse,
} from '../../../packages/vela-sdk/src/protocol';

describe('Vela Web Wallet protocol guards', () => {
  test('accepts a complete origin-handshake request', () => {
    expect(isVelaWebInit({
      channel: VELA_WEB_CHANNEL,
      type: VELA_WEB_INIT,
      sessionId: 'session-1',
      request: { id: 'rpc-1', method: 'personal_sign', params: ['0x01'], chainId: 8453 },
      dapp: { name: 'Example' },
    })).toBe(true);
  });

  test.each([
    ['missing session', { channel: VELA_WEB_CHANNEL, type: VELA_WEB_INIT, request: {}, dapp: { name: 'x' } }],
    ['invalid chain', { channel: VELA_WEB_CHANNEL, type: VELA_WEB_INIT, sessionId: 's', request: { id: '1', method: 'x', params: [], chainId: 0 }, dapp: { name: 'x' } }],
    ['object params', { channel: VELA_WEB_CHANNEL, type: VELA_WEB_INIT, sessionId: 's', request: { id: '1', method: 'x', params: {}, chainId: 1 }, dapp: { name: 'x' } }],
    ['wrong channel', { channel: 'attacker', type: VELA_WEB_INIT, sessionId: 's', request: { id: '1', method: 'x', params: [], chainId: 1 }, dapp: { name: 'x' } }],
    ['malformed address', { channel: VELA_WEB_CHANNEL, type: VELA_WEB_INIT, sessionId: 's', request: { id: '1', method: 'x', params: [], chainId: 1, address: '0xabc' }, dapp: { name: 'x' } }],
    ['oversized method', { channel: VELA_WEB_CHANNEL, type: VELA_WEB_INIT, sessionId: 's', request: { id: '1', method: 'x'.repeat(101), params: [], chainId: 1 }, dapp: { name: 'x' } }],
  ])('rejects %s', (_label, value) => expect(isVelaWebInit(value)).toBe(false));

  test('ready and response messages are session-bound', () => {
    expect(isVelaWebReady({ channel: VELA_WEB_CHANNEL, type: VELA_WEB_READY, sessionId: 's' })).toBe(true);
    expect(isVelaWebResponse({ channel: VELA_WEB_CHANNEL, type: VELA_WEB_RESPONSE, sessionId: 's', id: '1', result: '0x' })).toBe(true);
    expect(isVelaWebResponse({ channel: VELA_WEB_CHANNEL, type: VELA_WEB_RESPONSE, sessionId: 's', id: '1', error: { code: '4001', message: 'no' } })).toBe(false);
  });
});
