/**
 * Tests for the dev fault-injection console — specifically the rate-limit fault
 * added to validate graceful degradation UX. Rate-limiting must be a DISTINCT
 * fault from a hard failure: the read fails, but transiently.
 */
import { installFaultConsole, rpcShouldRateLimit, rpcShouldFail } from '@/services/dev/fault-injection';

interface VelaApi {
  rateLimitRpc: (c: number | 'all') => string;
  failRpc: (c: number | 'all') => string;
  clear: () => string;
  status: () => string;
}
function vela(): VelaApi {
  installFaultConsole(); // idempotent
  return (globalThis as unknown as { vela: VelaApi }).vela;
}

beforeEach(() => { vela().clear(); });
afterAll(() => { vela().clear(); });

describe('fault injection — rate-limit simulation', () => {
  test('no faults → rpcShouldRateLimit is false', () => {
    expect(rpcShouldRateLimit(137)).toBe(false);
  });

  test('vela.rateLimitRpc(chainId) targets a single chain', () => {
    vela().rateLimitRpc(137);
    expect(rpcShouldRateLimit(137)).toBe(true);
    expect(rpcShouldRateLimit(1)).toBe(false);
  });

  test('rate-limiting is NOT a hard failure (distinct fault)', () => {
    vela().rateLimitRpc(137);
    expect(rpcShouldRateLimit(137)).toBe(true);
    expect(rpcShouldFail(137)).toBe(false);
  });

  test("vela.rateLimitRpc('all') targets every chain", () => {
    vela().rateLimitRpc('all');
    expect(rpcShouldRateLimit(1)).toBe(true);
    expect(rpcShouldRateLimit(999)).toBe(true);
  });

  test('vela.clear() resets the rate-limit fault', () => {
    vela().rateLimitRpc('all');
    vela().clear();
    expect(rpcShouldRateLimit(1)).toBe(false);
  });

  test('status() surfaces the active rate-limit fault', () => {
    const s = vela().rateLimitRpc(137);
    expect(s).toMatch(/rate-limited/i);
  });
});
