/**
 * Feedback / bug-report deep links.
 *
 * Single builder for the prefilled GitHub bug form so every entry point — the
 * Settings "Feedback" row and the contextual "Report it" link on the RPC-trouble
 * banner — attaches the same actionable context. The `environment` query param
 * maps to the `environment` field id in .github/ISSUE_TEMPLATE/bug.yml, so the
 * form opens pre-populated and the reporter doesn't have to describe their setup
 * (the single biggest lever on whether a report is actionable).
 */
import { Platform } from 'react-native';
import { APP_VERSION, GIT_COMMIT } from '@/constants/build-info';
import { chainName } from '@/models/network';
import { LANGUAGE_NATIVE_NAMES, type AppLanguage } from '@/i18n';
import { getFailedRpcChains } from '@/services/rpc-pool';

export const VELA_REPO_URL = 'https://github.com/mondaylabsltd/vela-wallet';

export interface BugReportContext {
  /** Extra "- key: value" environment lines (e.g. the specific failing chain/RPC). */
  extraLines?: string[];
}

/** Build a prefilled bug-report URL with device + (live) failure context. */
export function buildBugReportURL(language: AppLanguage, ctx: BugReportContext = {}): string {
  const failed = [...getFailedRpcChains()];
  const lines = [
    `- App version: ${APP_VERSION} (${GIT_COMMIT})`,
    `- Platform: ${Platform.OS} ${Platform.Version}`,
    `- Language: ${LANGUAGE_NATIVE_NAMES[language]} (${language})`,
    ...(failed.length ? [`- RPC unreachable: ${failed.map((id) => `${chainName(id)} (${id})`).join(', ')}`] : []),
    ...(ctx.extraLines ?? []),
  ];
  const query = [
    'template=bug.yml',
    `environment=${encodeURIComponent(lines.join('\n'))}`,
  ].join('&');
  return `${VELA_REPO_URL}/issues/new?${query}`;
}
