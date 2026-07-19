import { canCoverNativeTransfer } from '@/screens/wallet/send-utils';

describe('canCoverNativeTransfer', () => {
  it('reserves the quoted fee exactly once', () => {
    const eth = 10n ** 18n;
    // 0.02 ETH balance − 0.001 ETH transfer leaves 0.019 ETH. A 0.006 ETH
    // reviewed quote fits; multiplying that quote again would incorrectly reject it.
    expect(canCoverNativeTransfer(eth / 1_000n, eth / 50n, (6n * eth) / 1_000n)).toBe(true);
  });

  it('rejects a transfer when the quoted fee really does not fit', () => {
    const eth = 10n ** 18n;
    expect(canCoverNativeTransfer(eth / 1_000n, eth / 50n, (19_001n * eth) / 1_000_000n)).toBe(false);
  });
});
