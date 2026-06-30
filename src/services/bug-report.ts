/**
 * One-click bug report — client side.
 *
 * Collects a SCRUBBED diagnostic snapshot and submits it to the getvela.app
 * backend proxy (which creates/▲1s a GitHub issue with a server-side token), so a
 * non-technical user with no GitHub account can still file a useful bug. If the
 * backend is unconfigured or unreachable, it transparently falls back to the
 * prefilled GitHub-issue URL ([[feedback.ts]] `buildBugReportURL`).
 *
 * Consent + privacy: nothing is sent until the user taps "Report". Call
 * `buildReportPreview()` to show EXACTLY what will be sent first (this is a
 * wallet — trust matters). The snapshot is coarse: app version, platform,
 * language, failing chains, and resilience metrics. It NEVER includes private
 * keys, seed phrases, signatures, full tokens, RPC keys, or addresses.
 */
import { Platform } from 'react-native';
import { APP_VERSION, GIT_COMMIT } from '@/constants/build-info';
import { chainName } from '@/models/network';
import { LANGUAGE_NATIVE_NAMES, type AppLanguage } from '@/i18n';
import { getFailedRpcChains } from '@/services/rpc-pool';
import { formatMetricsSummary, getMetricsSnapshot } from '@/services/metrics';
import { buildBugReportURL, type BugReportContext } from '@/services/feedback';
import { fetchWithTimeout, NET_TIMEOUTS } from '@/services/net';

/** Backend proxy endpoint (server holds the GitHub token). */
const BUG_REPORT_ENDPOINT = 'https://getvela.app/api/bug-report';

export interface BugReportInput {
  /** Required: what went wrong (free text from the user). */
  what: string;
  /** Optional: steps to reproduce. */
  steps?: string;
  /** Optional: which area of the app (Home/Send/Connect/Settings/…). */
  area?: string;
  language: AppLanguage;
}

export interface BugReportResult {
  ok: boolean;
  /** GitHub issue URL when submitted (or deduped onto an existing issue). */
  url?: string;
  /** GitHub issue number when submitted. */
  number?: number;
  /** True when +1'd onto an existing open issue rather than creating a new one. */
  deduped?: boolean;
  /** When ok=false: a prefilled GitHub URL the caller can open as a fallback. */
  fallbackUrl?: string;
  /** Coarse reason for telemetry/UX (never shown raw to the user). */
  reason?: string;
}

/** Sanitized environment lines (no secrets) — shared with the URL fallback. */
function environmentLines(language: AppLanguage): string[] {
  const failed = [...getFailedRpcChains()];
  return [
    `- App version: ${APP_VERSION} (${GIT_COMMIT})`,
    `- Platform: ${Platform.OS} ${Platform.Version}`,
    `- Language: ${LANGUAGE_NATIVE_NAMES[language]} (${language})`,
    ...(failed.length ? [`- RPC unreachable: ${failed.map((id) => `${chainName(id)} (${id})`).join(', ')}`] : []),
  ];
}

/** Compact, already-sanitized resilience diagnostics from the metrics module. */
function diagnosticsBlock(): string {
  const snap = getMetricsSnapshot();
  const lines = [formatMetricsSummary()];
  if (snap.recentFailures.length) {
    lines.push('recent final failures:');
    for (const f of snap.recentFailures.slice(-6)) {
      const t = new Date(f.at).toISOString().slice(11, 19);
      lines.push(`  ${t} ${f.service} ${f.outcome}${f.status ? ' ' + f.status : ''}${f.note ? ' — ' + f.note : ''}`);
    }
  }
  return lines.join('\n');
}

/**
 * A short, stable fingerprint so repeated reports of the same failure dedupe onto
 * one issue. Non-cryptographic (dedup only): area + the most recent final-failure
 * signature + app version. Deliberately coarse so genuinely-same bugs collapse.
 */
function fingerprint(area: string | undefined): string {
  const snap = getMetricsSnapshot();
  const last = snap.recentFailures[snap.recentFailures.length - 1];
  const sig = `${area ?? ''}|${last ? `${last.service}:${last.outcome}:${last.status ?? ''}` : 'no-fail'}|${APP_VERSION}`;
  let h = 5381;
  for (let i = 0; i < sig.length; i++) h = ((h << 5) + h + sig.charCodeAt(i)) | 0;
  return 'fp' + (h >>> 0).toString(36);
}

/** Exactly what will be sent — show this to the user before they tap Report. */
export function buildReportPreview(input: BugReportInput): string {
  return [
    input.what.trim(),
    ...(input.steps?.trim() ? ['', 'Steps:', input.steps.trim()] : []),
    ...(input.area ? ['', `Area: ${input.area}`] : []),
    '',
    ...environmentLines(input.language),
    '',
    'Diagnostics:',
    diagnosticsBlock(),
  ].join('\n');
}

/**
 * Submit a bug report. Tries the backend proxy first (works for users without a
 * GitHub account); on any failure returns ok=false plus a `fallbackUrl` the
 * caller can open in the browser (the prefilled GitHub issue form).
 */
export async function submitBugReport(input: BugReportInput): Promise<BugReportResult> {
  const env = environmentLines(input.language);
  const ctx: BugReportContext = { extraLines: [] };
  const fallbackUrl = buildBugReportURL(input.language, ctx);

  const what = input.what.trim();
  if (!what) return { ok: false, reason: 'empty', fallbackUrl };

  try {
    const res = await fetchWithTimeout(
      BUG_REPORT_ENDPOINT,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          what,
          steps: input.steps?.trim() || undefined,
          area: input.area,
          environment: env.join('\n'),
          diagnostics: diagnosticsBlock(),
          fingerprint: fingerprint(input.area),
        }),
      },
      { timeoutMs: NET_TIMEOUTS.bundlerRest },
    );

    if (res.ok) {
      const data = (await res.json()) as { url?: string; number?: number; deduped?: boolean };
      if (data.url) return { ok: true, url: data.url, number: data.number, deduped: data.deduped };
      return { ok: false, reason: 'bad_response', fallbackUrl };
    }

    // 503 not_configured (token not provisioned yet) → use URL fallback silently.
    let reason = `http_${res.status}`;
    try {
      const body = (await res.json()) as { error?: string };
      if (body.error) reason = body.error;
    } catch { /* ignore */ }
    return { ok: false, reason, fallbackUrl };
  } catch (err) {
    return { ok: false, reason: err instanceof Error ? err.name : 'network', fallbackUrl };
  }
}
