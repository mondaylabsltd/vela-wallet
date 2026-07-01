import {
  enableTevmFallback,
  isTevmFallbackEnabled,
  setTevmLoader,
  tevmSimulate,
} from '@/services/sim-engine-tevm';

afterEach(() => {
  enableTevmFallback(false);
  setTevmLoader(null);
});

describe('Tevm fallback loading', () => {
  test('stays inert when no loader is registered', async () => {
    enableTevmFallback(true);

    await expect(tevmSimulate(
      '0x' + '11'.repeat(20),
      [{ to: '0x' + '22'.repeat(20), data: '0x' }],
      1,
    )).resolves.toBeNull();
    expect(isTevmFallbackEnabled()).toBe(true);
  });

  test('does not invoke a registered loader while disabled', async () => {
    const loader = jest.fn();
    setTevmLoader(loader);

    await expect(tevmSimulate(
      '0x' + '11'.repeat(20),
      [{ to: '0x' + '22'.repeat(20), data: '0x' }],
      1,
    )).resolves.toBeNull();
    expect(loader).not.toHaveBeenCalled();
  });
});
