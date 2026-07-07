// Pure-logic tests for the in-app dApp browser request router.

import { classifyBrowserRequest, isConnectMethod } from '@/services/wallet-browser-router';

const PERM = [{ parentCapability: 'eth_accounts' }];

describe('classifyBrowserRequest', () => {
  test('eth_accounts reflects the grant and never prompts', () => {
    expect(classifyBrowserRequest('eth_accounts', [])).toEqual({ kind: 'respond', result: [] });
    expect(classifyBrowserRequest('eth_accounts', ['0xabc'])).toEqual({ kind: 'respond', result: ['0xabc'] });
  });

  test('wallet_getPermissions mirrors the grant', () => {
    expect(classifyBrowserRequest('wallet_getPermissions', [])).toEqual({ kind: 'respond', result: [] });
    expect(classifyBrowserRequest('wallet_getPermissions', ['0xabc'])).toEqual({ kind: 'respond', result: PERM });
  });

  test('eth_requestAccounts: granted → respond accounts, ungranted → consent', () => {
    expect(classifyBrowserRequest('eth_requestAccounts', ['0xabc'])).toEqual({ kind: 'respond', result: ['0xabc'] });
    expect(classifyBrowserRequest('eth_requestAccounts', [])).toEqual({ kind: 'consent' });
  });

  test('wallet_requestPermissions: granted → respond permissions, ungranted → consent', () => {
    expect(classifyBrowserRequest('wallet_requestPermissions', ['0xabc'])).toEqual({ kind: 'respond', result: PERM });
    expect(classifyBrowserRequest('wallet_requestPermissions', [])).toEqual({ kind: 'consent' });
  });

  test('signing + read-only + switch all forward to the pipeline', () => {
    for (const m of [
      'eth_sendTransaction',
      'personal_sign',
      'eth_signTypedData_v4',
      'wallet_sendCalls',
      'eth_call',
      'eth_getBalance',
      'wallet_switchEthereumChain',
      'eth_chainId',
    ]) {
      expect(classifyBrowserRequest(m, ['0xabc'])).toEqual({ kind: 'forward' });
    }
  });
});

describe('isConnectMethod', () => {
  test('true only for the two connect methods', () => {
    expect(isConnectMethod('eth_requestAccounts')).toBe(true);
    expect(isConnectMethod('wallet_requestPermissions')).toBe(true);
    expect(isConnectMethod('eth_accounts')).toBe(false);
    expect(isConnectMethod('personal_sign')).toBe(false);
  });
});
