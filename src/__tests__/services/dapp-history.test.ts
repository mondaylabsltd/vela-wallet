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

import { buildConnectionRecord, buildSigningRecord, capRequest, decodeSignMessage, extractSignedContent } from '@/services/dapp-history';
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

  it('captures the original request so the panel can be replayed', () => {
    const r = rec('eth_sendTransaction', [{ to: '0xabc', value: '0x2', data: '0xdead' }], '0xhash');
    expect(r.signedRequest).toEqual({ method: 'eth_sendTransaction', params: [{ to: '0xabc', value: '0x2', data: '0xdead' }] });
    expect(r.requestTruncated).toBe(false);
  });

  it('records as pending with the userOp hash when submitted', () => {
    const r = buildSigningRecord({
      method: 'eth_sendTransaction', params: [{ to: '0xabc', value: '0x1', data: '0x' }],
      result: '', from: FROM, chainId: 1, dappOrigin: 'test.app', nowMs: 1000,
      status: 'pending', userOpHash: '0xuserop',
    });
    expect(r.status).toBe('pending');
    expect(r.userOpHash).toBe('0xuserop');
    expect(r.txHash).toBe(''); // no on-chain hash yet
  });

  it('gives every record a collision-free id across timestamps', () => {
    const a = buildSigningRecord({ method: 'personal_sign', params: [HELLO_HEX], result: '', from: FROM, chainId: 1, dappOrigin: '', nowMs: 1000 });
    const b = buildSigningRecord({ method: 'personal_sign', params: [HELLO_HEX], result: '', from: FROM, chainId: 1, dappOrigin: '', nowMs: 2000 });
    expect(a.id).not.toBe(b.id);
  });
});

describe('capRequest — replay payload capture', () => {
  it('stores method + params verbatim when small', () => {
    const { signedRequest, requestTruncated } = capRequest('personal_sign', ['0x48656c6c6f', FROM]);
    expect(signedRequest).toEqual({ method: 'personal_sign', params: ['0x48656c6c6f', FROM] });
    expect(requestTruncated).toBe(false);
  });

  it('clips an oversized calldata (e.g. a deploy) and keeps the TOTAL within budget', () => {
    const huge = '0x' + 'ab'.repeat(20000); // ~40k chars, over the budget
    const { signedRequest, requestTruncated } = capRequest('eth_sendTransaction', [{ to: '0xdep', data: huge }]);
    expect(requestTruncated).toBe(true);
    const clippedData = (signedRequest.params[0] as { data: string }).data;
    expect(clippedData.length).toBeLessThan(huge.length);
    // The function selector (first bytes) survives, so the intent still resolves.
    expect(clippedData.startsWith('0xabab')).toBe(true);
    // The invariant that matters: the stored payload never exceeds the budget.
    expect(JSON.stringify(signedRequest).length).toBeLessThanOrEqual(24000);
  });

  it('bounds the total even when many medium strings each fit (fat batch)', () => {
    // 30 sub-budget calldata strings (~10k each) sum to ~300k — none individually
    // over budget, so a per-string-only cap would store the lot. The total must
    // still be clamped.
    const calls = Array.from({ length: 30 }, () => ({ data: '0x' + 'cd'.repeat(5000) }));
    const { signedRequest, requestTruncated } = capRequest('wallet_sendCalls', [{ calls }]);
    expect(requestTruncated).toBe(true);
    expect(JSON.stringify(signedRequest).length).toBeLessThanOrEqual(24000);
  });

  it('tolerates non-serializable params without throwing', () => {
    const circular: any = {}; circular.self = circular;
    const { signedRequest, requestTruncated } = capRequest('eth_sendTransaction', [circular]);
    expect(requestTruncated).toBe(true);
    expect(signedRequest.params).toEqual([]);
  });
});

describe('buildSigningRecord — intent persistence', () => {
  it('persists a provided clear-signing intent (so the label is not generic)', () => {
    const r = buildSigningRecord({
      method: 'eth_sendTransaction', params: [{ to: '0xrouter', data: '0xabcd', value: '0x0' }],
      result: '0xtx', from: FROM, chainId: 1, dappOrigin: 'app', nowMs: 1000, intent: 'Swap',
    });
    expect(r.intent).toBe('Swap');
    expect(r.type).toBe('dapp_tx');
  });

  it('leaves intent undefined when none is supplied (label falls back)', () => {
    const r = buildSigningRecord({
      method: 'eth_sendTransaction', params: [{ to: '0xrouter', data: '0xabcd' }],
      result: '0xtx', from: FROM, chainId: 1, dappOrigin: 'app', nowMs: 1000,
    });
    expect(r.intent).toBeUndefined();
  });
});

describe('buildConnectionRecord', () => {
  it('builds a connect-type audit record with no signature/tx', () => {
    const r = buildConnectionRecord({ from: FROM, chainId: 56, dappOrigin: 'pancakeswap.finance', nowMs: 5000 });
    expect(r.type).toBe('connect');
    expect(r.id).toBe('dapp-5000-connect');
    expect(r.status).toBe('confirmed');
    expect(r.dappOrigin).toBe('pancakeswap.finance');
    expect(r.chainId).toBe(56);
    expect(r.from).toBe(FROM);
    // No signed content, no tx hash, no value — it's a session marker.
    expect(r.txHash).toBe('');
    expect(r.userOpHash).toBe('');
    expect(r.signedContent).toBeUndefined();
    expect(r.signedRequest).toBeUndefined();
    expect(r.timestamp).toBe(5); // floor(5000 / 1000)
  });
});
