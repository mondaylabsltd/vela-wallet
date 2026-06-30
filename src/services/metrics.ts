/**
 * Lightweight, dependency-free resilience metrics.
 *
 * In-memory counters + a small ring buffer of recent final failures, so the app
 * can answer "how often are external calls timing out / 5xx-ing / retrying, and
 * what failed last?" without any telemetry backend. Always-on and cheap (a couple
 * of object writes per call); bounded so it can never grow unboundedly.
 *
 * Two consumers:
 *   - `vela.metrics()` in the web console (dev visibility).
 *   - the one-click bug report, which attaches a sanitized snapshot so a user's
 *     report carries the failure context developers need (the closest thing to
 *     an alert when there's no metrics sink).
 *
 * Privacy: only coarse outcomes, counts, status codes and short sanitized notes
 * are stored — never URLs with keys, tokens, addresses, or payloads. Callers pass
 * already-safe labels; `sanitizeNote` is a second line of defence.
 */

export type NetOutcome =
  | 'success'
  | 'timeout'
  | 'network'
  | 'aborted'
  | 'http_4xx'
  | 'http_429'
  | 'http_5xx'
  | 'rpc_error'
  | 'retry'
  | 'final_failure';

interface ServiceCounters {
  success: number;
  timeout: number;
  network: number;
  aborted: number;
  http_4xx: number;
  http_429: number;
  http_5xx: number;
  rpc_error: number;
  retry: number;
  final_failure: number;
}

export interface FailureEntry {
  /** Epoch ms. */
  at: number;
  service: string;
  outcome: NetOutcome;
  status?: number;
  note?: string;
}

export interface MetricsSnapshot {
  since: number;
  services: Record<string, ServiceCounters>;
  recentFailures: FailureEntry[];
}

const MAX_RECENT_FAILURES = 25;

function emptyCounters(): ServiceCounters {
  return {
    success: 0, timeout: 0, network: 0, aborted: 0,
    http_4xx: 0, http_429: 0, http_5xx: 0, rpc_error: 0,
    retry: 0, final_failure: 0,
  };
}

const services: Record<string, ServiceCounters> = {};
const recentFailures: FailureEntry[] = [];
let since = Date.now();

/** Strip anything secret-shaped and clamp length, in case a caller passes raw text. */
export function sanitizeNote(note: string | undefined): string | undefined {
  if (!note) return undefined;
  return note
    .replace(/0x[0-9a-fA-F]{16,}/g, '0x…')                       // long hex (keys/sigs/calldata)
    .replace(/([?&](?:api[-_]?key|key|token|secret|access[-_]?token)=)[^&\s]+/gi, '$1***')
    .slice(0, 160);
}

/**
 * Record one external-call outcome for `service` (e.g. 'rpc', 'bundler',
 * 'keyindex', 'fiat', 'descriptor'). `final_failure` also lands in the recent
 * ring buffer for diagnostics / bug reports.
 */
export function recordNet(
  service: string,
  outcome: NetOutcome,
  detail?: { status?: number; note?: string },
): void {
  const c = services[service] ?? (services[service] = emptyCounters());
  c[outcome] = (c[outcome] ?? 0) + 1;
  if (outcome === 'final_failure') {
    recentFailures.push({
      at: Date.now(),
      service,
      outcome,
      status: detail?.status,
      note: sanitizeNote(detail?.note),
    });
    if (recentFailures.length > MAX_RECENT_FAILURES) recentFailures.shift();
  }
}

/** Map an HTTP status to a coarse outcome (429 and 5xx are called out). */
export function outcomeForStatus(status: number): NetOutcome {
  if (status === 429) return 'http_429';
  if (status >= 500) return 'http_5xx';
  if (status >= 400) return 'http_4xx';
  return 'success';
}

/** A point-in-time copy safe to log or attach to a bug report. */
export function getMetricsSnapshot(): MetricsSnapshot {
  return {
    since,
    services: JSON.parse(JSON.stringify(services)),
    recentFailures: recentFailures.slice(),
  };
}

/** A compact human-readable summary (used by the console and bug-report body). */
export function formatMetricsSummary(): string {
  const names = Object.keys(services);
  if (names.length === 0) return 'no external calls recorded yet';
  const lines: string[] = [];
  for (const name of names) {
    const c = services[name];
    const total = c.success + c.timeout + c.network + c.http_4xx + c.http_429 + c.http_5xx + c.rpc_error;
    const ok = c.success;
    const rate = total > 0 ? Math.round((ok / total) * 100) : 100;
    const parts = [`ok ${ok}/${total} (${rate}%)`];
    if (c.timeout) parts.push(`timeout ${c.timeout}`);
    if (c.http_429) parts.push(`429 ${c.http_429}`);
    if (c.http_5xx) parts.push(`5xx ${c.http_5xx}`);
    if (c.rpc_error) parts.push(`rpcErr ${c.rpc_error}`);
    if (c.retry) parts.push(`retry ${c.retry}`);
    if (c.final_failure) parts.push(`FINAL_FAIL ${c.final_failure}`);
    lines.push(`  ${name}: ${parts.join(', ')}`);
  }
  return lines.join('\n');
}

/** Reset all counters (e.g. before reproducing an issue). */
export function resetMetrics(): void {
  for (const k of Object.keys(services)) delete services[k];
  recentFailures.length = 0;
  since = Date.now();
}

/** Add `vela.metrics()` / `vela.metricsReset()` to the console namespace. */
export function installMetricsConsole(): void {
  const g = globalThis as any;
  g.vela = Object.assign(g.vela ?? {}, {
    metrics() {
      const summary = formatMetricsSummary();
      const fails = recentFailures.slice(-8).map((f) =>
        `    ${new Date(f.at).toISOString().slice(11, 19)} ${f.service} ${f.outcome}${f.status ? ' ' + f.status : ''}${f.note ? ' — ' + f.note : ''}`,
      );
      console.log(['[vela] resilience metrics:', summary, ...(fails.length ? ['  recent final failures:', ...fails] : [])].join('\n'));
      return getMetricsSnapshot();
    },
    metricsReset() {
      resetMetrics();
      console.log('[vela] metrics reset');
      return 'reset';
    },
  });
}
