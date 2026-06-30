/**
 * Tests for the resilience metrics module.
 */
import {
  recordNet,
  getMetricsSnapshot,
  formatMetricsSummary,
  outcomeForStatus,
  sanitizeNote,
  resetMetrics,
} from '@/services/metrics';

describe('metrics', () => {
  beforeEach(() => resetMetrics());

  test('counts outcomes per service', () => {
    recordNet('rpc', 'success');
    recordNet('rpc', 'success');
    recordNet('rpc', 'timeout');
    recordNet('bundler', 'final_failure');
    const snap = getMetricsSnapshot();
    expect(snap.services.rpc.success).toBe(2);
    expect(snap.services.rpc.timeout).toBe(1);
    expect(snap.services.bundler.final_failure).toBe(1);
  });

  test('final_failure lands in the bounded recent-failures ring', () => {
    for (let i = 0; i < 40; i++) recordNet('rpc', 'final_failure', { note: `fail ${i}` });
    const snap = getMetricsSnapshot();
    // ring is capped at 25
    expect(snap.recentFailures.length).toBe(25);
    // keeps the most recent
    expect(snap.recentFailures[snap.recentFailures.length - 1].note).toBe('fail 39');
  });

  test('non-failure outcomes do not fill the ring', () => {
    recordNet('rpc', 'success');
    recordNet('rpc', 'retry');
    expect(getMetricsSnapshot().recentFailures.length).toBe(0);
  });

  test('getMetricsSnapshot returns a copy (not live references)', () => {
    recordNet('rpc', 'success');
    const snap = getMetricsSnapshot();
    recordNet('rpc', 'success');
    expect(snap.services.rpc.success).toBe(1); // unaffected by later writes
  });

  test('outcomeForStatus maps 429 / 5xx / 4xx / ok', () => {
    expect(outcomeForStatus(429)).toBe('http_429');
    expect(outcomeForStatus(503)).toBe('http_5xx');
    expect(outcomeForStatus(404)).toBe('http_4xx');
    expect(outcomeForStatus(200)).toBe('success');
  });

  test('sanitizeNote strips long hex and key/token params', () => {
    const note = sanitizeNote('failed at 0x1234567890abcdef1234 with ?apikey=SECRET token');
    expect(note).not.toContain('1234567890abcdef1234');
    expect(note).not.toContain('SECRET');
    expect(note).toContain('0x…');
  });

  test('formatMetricsSummary is human-readable and reports success rate', () => {
    recordNet('rpc', 'success');
    recordNet('rpc', 'timeout');
    const s = formatMetricsSummary();
    expect(s).toContain('rpc:');
    expect(s).toMatch(/ok 1\/2/);
  });

  test('resetMetrics clears everything', () => {
    recordNet('rpc', 'success');
    recordNet('rpc', 'final_failure');
    resetMetrics();
    const snap = getMetricsSnapshot();
    expect(Object.keys(snap.services).length).toBe(0);
    expect(snap.recentFailures.length).toBe(0);
  });
});
