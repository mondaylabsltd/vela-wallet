/**
 * Guards the "every approved dApp request is recorded" contract.
 *
 * The bug this prevents: isSigningMethod() approved eth_sign / wallet_sendCalls
 * but the history-save block only handled three methods, so those signed
 * successfully yet never appeared in the Connections panel. buildSigningRecord
 * must produce a record for ANY method isSigningMethod() accepts.
 */
// Break the react-native import chain (dapp-history → network → storage,
// and use-dapp-signing → passkey). Mirrors the dapp-signing test's mock set.
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/modules/passkey', () => ({}));

import { buildSigningRecord, decodeSignMessage, extractSignedContent } from '@/services/dapp-history';
import { isSigningMethod } from '@/hooks/use-dapp-signing';

const FROM = '0x1111111111111111111111111111111111111111';
const HELLO_HEX = '0x48656c6c6f'; // "Hello"

function rec(method: string, params: unknown[], result: unknown = '') {
  return buildSigningRecord({ method, params, result, from: FROM, chainId: 1, dappOrigin: 'test.app', nowMs: 1000 });
}

describe('decodeSignMessage', () => {
  it('decodes printable hex to text', () => {
    expect(decodeSignMessage(HELLO_HEX)).toBe('Hello');
  });
  it('passes plain (non-hex) text through', () => {
    expect(decodeSignMessage('just text')).toBe('just text');
  });
  it('keeps hex when it decodes to binary (invalid UTF-8)', () => {
    expect(decodeSignMessage('0xff')).toBe('0xff');
  });
  it('keeps hex when it decodes to control chars', () => {
    expect(decodeSignMessage('0x00')).toBe('0x00');
  });
});

describe('extractSignedContent', () => {
  it('personal_sign reads the message (params[0])', () => {
    expect(extractSignedContent('personal_sign', [HELLO_HEX, FROM])).toBe('Hello');
  });
  it('eth_sign reads the reversed data (params[1])', () => {
    expect(extractSignedContent('eth_sign', [FROM, HELLO_HEX])).toBe('Hello');
  });
  it('signTypedData pretty-prints the typed data', () => {
    const out = extractSignedContent('eth_signTypedData_v4', [FROM, '{"domain":{"name":"X"}}']);
    expect(out).toContain('domain');
    expect(out).toContain('"name": "X"');
  });
  it('eth_sendTransaction returns calldata, undefined when empty', () => {
    expect(extractSignedContent('eth_sendTransaction', [{ data: '0xdead' }])).toBe('0xdead');
    expect(extractSignedContent('eth_sendTransaction', [{ data: '0x' }])).toBeUndefined();
  });
});

describe('buildSigningRecord — per method', () => {
  it('eth_sendTransaction → dapp_tx with hash, recipient, value', () => {
    const r = rec('eth_sendTransaction', [{ to: '0xabc', value: '0x2', data: '0xdead' }], '0xhash');
    expect(r.type).toBe('dapp_tx');
    expect(r.txHash).toBe('0xhash');
    expect(r.to).toBe('0xabc');
    expect(r.value).toBe('0x2');
    expect(r.signedContent).toBe('0xdead');
  });
  it('wallet_sendCalls → dapp_tx (batched), content captured', () => {
    const r = rec('wallet_sendCalls', [{ calls: [{ to: '0xabc' }] }], undefined);
    expect(r.type).toBe('dapp_tx');
    expect(r.txHash).toBe('');
    expect(r.signedContent).toContain('calls');
  });
  it('personal_sign → sign_message with decoded text', () => {
    const r = rec('personal_sign', [HELLO_HEX, FROM]);
    expect(r.type).toBe('sign_message');
    expect(r.signedContent).toBe('Hello');
  });
  it('eth_sign → sign_message (previously dropped!)', () => {
    const r = rec('eth_sign', [FROM, HELLO_HEX]);
    expect(r.type).toBe('sign_message');
    expect(r.signedContent).toBe('Hello');
  });
  it('eth_signTypedData_v4 → sign_typed_data', () => {
    const r = rec('eth_signTypedData_v4', [FROM, '{"domain":{}}']);
    expect(r.type).toBe('sign_typed_data');
    expect(r.signedContent).toContain('domain');
  });
});

describe('buildSigningRecord — contract with isSigningMethod', () => {
  // Every method the app APPROVES must yield a saved record (non-empty type).
  const APPROVED = [
    'eth_sendTransaction',
    'wallet_sendCalls',
    'personal_sign',
    'eth_sign',
    'eth_signTypedData',
    'eth_signTypedData_v4',
  ];

  it.each(APPROVED)('%s is approved AND produces a record', (method) => {
    expect(isSigningMethod(method)).toBe(true);
    const r = rec(method, [{ to: '0xabc' }, '{}']);
    expect(r.type).toBeTruthy();
    expect(['dapp_tx', 'sign_message', 'sign_typed_data']).toContain(r.type);
    expect(r.from).toBe(FROM);
    expect(r.status).toBe('confirmed');
    expect(r.id).toMatch(/^dapp-\d+-/);
  });

  it('gives every record a collision-free id across timestamps', () => {
    const a = buildSigningRecord({ method: 'personal_sign', params: [HELLO_HEX], result: '', from: FROM, chainId: 1, dappOrigin: '', nowMs: 1000 });
    const b = buildSigningRecord({ method: 'personal_sign', params: [HELLO_HEX], result: '', from: FROM, chainId: 1, dappOrigin: '', nowMs: 2000 });
    expect(a.id).not.toBe(b.id);
  });
});
