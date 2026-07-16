import { describe, expect, it } from 'vitest';
import { isTransactionSubmissionMethod } from './provider-methods';

describe('provider transaction submission methods', () => {
  it.each(['eth_sendTransaction', 'wallet_sendTransaction'])(
    'routes %s through the Safe execution relayer',
    (method) => {
      expect(isTransactionSubmissionMethod(method)).toBe(true);
    },
  );

  it('does not treat unrelated wallet methods as transaction submissions', () => {
    expect(isTransactionSubmissionMethod('wallet_switchEthereumChain')).toBe(false);
    expect(isTransactionSubmissionMethod('eth_sendRawTransaction')).toBe(false);
  });
});
