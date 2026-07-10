// Pure-logic tests for the in-app dApp browser request router.

import {
  classifyBrowserRequest,
  decideBrowserRequest,
  isConnectMethod,
  isInsecurePublicOrigin,
  shouldBlockInsecureSigning,
} from '@/services/wallet-browser-router';

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

describe('isInsecurePublicOrigin', () => {
  test('https is always secure', () => {
    expect(isInsecurePublicOrigin('https://app.uniswap.org')).toBe(false);
  });

  test('public http is insecure', () => {
    expect(isInsecurePublicOrigin('http://evil.example.com')).toBe(true);
    expect(isInsecurePublicOrigin('http://1.2.3.4')).toBe(true); // public IP
  });

  test('localhost / loopback / .local / private-LAN http are exempt (dev + on-device test dApp)', () => {
    expect(isInsecurePublicOrigin('http://localhost:3000')).toBe(false);
    expect(isInsecurePublicOrigin('http://127.0.0.1:8080')).toBe(false);
    expect(isInsecurePublicOrigin('http://mac.local')).toBe(false);
    expect(isInsecurePublicOrigin('http://192.168.1.42:19006')).toBe(false);
    expect(isInsecurePublicOrigin('http://10.0.0.5')).toBe(false);
    expect(isInsecurePublicOrigin('http://172.16.0.9')).toBe(false);
    expect(isInsecurePublicOrigin('http://172.31.255.255')).toBe(false);
  });

  test('172.x outside 16–31 is public', () => {
    expect(isInsecurePublicOrigin('http://172.15.0.1')).toBe(true);
    expect(isInsecurePublicOrigin('http://172.32.0.1')).toBe(true);
  });

  test('unparseable origin is treated as insecure', () => {
    expect(isInsecurePublicOrigin('not a url')).toBe(true);
  });
});

describe('shouldBlockInsecureSigning', () => {
  test('blocks signing methods on public http', () => {
    expect(shouldBlockInsecureSigning('eth_sendTransaction', 'http://evil.example.com')).toBe(true);
    expect(shouldBlockInsecureSigning('personal_sign', 'http://evil.example.com')).toBe(true);
    expect(shouldBlockInsecureSigning('eth_signTypedData_v4', 'http://evil.example.com')).toBe(true);
  });

  test('allows signing on https and on localhost/LAN', () => {
    expect(shouldBlockInsecureSigning('eth_sendTransaction', 'https://app.uniswap.org')).toBe(false);
    expect(shouldBlockInsecureSigning('personal_sign', 'http://localhost:3000')).toBe(false);
    expect(shouldBlockInsecureSigning('eth_sendTransaction', 'http://192.168.1.10:8080')).toBe(false);
  });

  test('never blocks non-signing methods, even on public http', () => {
    expect(shouldBlockInsecureSigning('eth_chainId', 'http://evil.example.com')).toBe(false);
    expect(shouldBlockInsecureSigning('eth_accounts', 'http://evil.example.com')).toBe(false);
    expect(shouldBlockInsecureSigning('wallet_switchEthereumChain', 'http://evil.example.com')).toBe(false);
  });
});

describe('decideBrowserRequest', () => {
  const base = {
    isMainFrame: true,
    granted: [] as string[],
    hasActiveAccount: true,
    pendingConsentOrigin: null as string | null,
  };

  describe('state reads (respond, never prompt)', () => {
    test('eth_accounts reflects the grant', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_accounts', origin: 'https://a.io', granted: ['0xabc'] }))
        .toEqual({ kind: 'respond', result: ['0xabc'] });
      expect(decideBrowserRequest({ ...base, method: 'eth_accounts', origin: 'https://a.io', granted: [] }))
        .toEqual({ kind: 'respond', result: [] });
    });

    test('a subframe already reads [] (caller passes granted=[]) → respond []', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_accounts', origin: 'https://a.io', isMainFrame: false, granted: [] }))
        .toEqual({ kind: 'respond', result: [] });
    });
  });

  describe('connect gating', () => {
    test('main-frame, no grant, has account → open the consent sheet', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_requestAccounts', origin: 'https://a.io' }))
        .toEqual({ kind: 'open-consent' });
    });

    test('already granted → respond immediately (no prompt)', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_requestAccounts', origin: 'https://a.io', granted: ['0xabc'] }))
        .toEqual({ kind: 'respond', result: ['0xabc'] });
    });

    test('cross-origin iframe connect → 4100 Unauthorized frame', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_requestAccounts', origin: 'https://ad.evil', isMainFrame: false }))
        .toEqual({ kind: 'reject', code: 4100, message: 'Unauthorized frame' });
    });

    test('no active account → 4001', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_requestAccounts', origin: 'https://a.io', hasActiveAccount: false }))
        .toEqual({ kind: 'reject', code: 4001, message: 'No account available' });
    });

    test('second connect from the SAME origin while a sheet is open → merge (never hang)', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_requestAccounts', origin: 'https://a.io', pendingConsentOrigin: 'https://a.io' }))
        .toEqual({ kind: 'merge-consent' });
    });

    test('connect from a DIFFERENT origin while a sheet is open → reject (one sheet at a time)', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_requestAccounts', origin: 'https://b.io', pendingConsentOrigin: 'https://a.io' }))
        .toEqual({ kind: 'reject', code: 4001, message: 'Another connection request is pending' });
    });

    test('wallet_requestPermissions behaves like a connect method', () => {
      expect(decideBrowserRequest({ ...base, method: 'wallet_requestPermissions', origin: 'https://a.io' }))
        .toEqual({ kind: 'open-consent' });
    });
  });

  describe('forward vs insecure-signing block', () => {
    test('read-only RPC forwards', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_call', origin: 'https://a.io' }))
        .toEqual({ kind: 'forward' });
    });

    test('signing on https forwards', () => {
      expect(decideBrowserRequest({ ...base, method: 'personal_sign', origin: 'https://a.io' }))
        .toEqual({ kind: 'forward' });
    });

    test('signing on public http is blocked 4100', () => {
      expect(decideBrowserRequest({ ...base, method: 'eth_sendTransaction', origin: 'http://evil.io' }))
        .toEqual({ kind: 'reject', code: 4100, message: 'Signing is disabled on insecure (http) sites' });
    });

    test('signing on LAN http still forwards (dev / on-device test dApp)', () => {
      expect(decideBrowserRequest({ ...base, method: 'personal_sign', origin: 'http://192.168.50.40:8080' }))
        .toEqual({ kind: 'forward' });
    });

    test('wallet_switchEthereumChain forwards even on http (not a signing method)', () => {
      expect(decideBrowserRequest({ ...base, method: 'wallet_switchEthereumChain', origin: 'http://evil.io' }))
        .toEqual({ kind: 'forward' });
    });
  });
});
