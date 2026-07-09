// Unit tests for the Safari-extension sign bus — the single-listener singleton
// that hands an inbound /sign rid from the deep-link trampoline to the always-
// mounted <ExtensionSignController>. Exactly ONE controller listens at a time, so
// the bus must: register the latest listener, replace (never stack) on re-register,
// unsubscribe cleanly, and no-op safely when nothing is listening.
import { onExtensionSign, requestExtensionSign } from '@/services/extension-sign-bus';

// The bus holds module-level state; null it out between tests so each starts clean.
afterEach(() => {
  const reset = onExtensionSign(() => {});
  reset();
});

describe('extension-sign-bus', () => {
  it('delivers the rid to the registered listener', () => {
    const seen: string[] = [];
    onExtensionSign((rid) => seen.push(rid));
    requestExtensionSign('rid-1');
    expect(seen).toEqual(['rid-1']);
  });

  it('a second onExtensionSign REPLACES the first — only the latest listener fires', () => {
    const first = jest.fn();
    const second = jest.fn();
    onExtensionSign(first);
    onExtensionSign(second);
    requestExtensionSign('rid-2');
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledWith('rid-2');
  });

  it('the returned unsubscribe clears the listener — later requests are no-ops', () => {
    const fn = jest.fn();
    const unsubscribe = onExtensionSign(fn);
    unsubscribe();
    requestExtensionSign('rid-3');
    expect(fn).not.toHaveBeenCalled();
  });

  it("unsubscribe only clears if it is still the current listener (a stale unsub can't detach a newer one)", () => {
    const first = jest.fn();
    const second = jest.fn();
    const unsubFirst = onExtensionSign(first);
    onExtensionSign(second); // second is now current
    unsubFirst(); // stale — must NOT detach second
    requestExtensionSign('rid-4');
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith('rid-4');
  });

  it('requestExtensionSign with no listener does not throw', () => {
    const unsubscribe = onExtensionSign(() => {});
    unsubscribe(); // ensure no listener is registered
    expect(() => requestExtensionSign('rid-5')).not.toThrow();
  });
});
