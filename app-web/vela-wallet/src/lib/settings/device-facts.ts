/**
 * What this device may say about itself in a report — the five preview lines
 * and the payload's `environment` are built from this and nothing else
 * (`services/bug-report.ts`, {@link DeviceFacts}).
 *
 * One builder for every door into the report: Settings → Send feedback, and a
 * relay stop's "Report this" on the wallet (issue 466). No address, no
 * balance, no endpoint or RPC URL, no raw `vela.*` value; the failure lines
 * are the net counters' CLASSES (`rpc:final_failure ×3`), which say what
 * broke without saying where.
 */
import { BUILD_COMMIT, BUILD_VERSION } from '$lib/build/info';
import { webClient, webOs, webPlatform, type DeviceFacts } from '$lib/services/bug-report';
import { netCounters } from '$lib/services/metrics';
import { chainName } from '$lib/services/networks';
import { getFailedRpcChains } from '$lib/services/rpc-pool';

/**
 * This device, now. `workerFailures` are the extension worker's counters
 * (spec 082 RB14), read asynchronously by the caller — empty outside it.
 */
export function deviceFacts(language: string, workerFailures: readonly string[]): DeviceFacts {
	const failures: string[] = [];
	for (const [key, count] of netCounters()) {
		if (key.endsWith(':final_failure') && count > 0) failures.push(`${key} ×${count}`);
	}
	failures.push(...workerFailures);
	return {
		version: BUILD_VERSION,
		// 078 §E: the issue title's "[Web]" / "[Extension]", and the short
		// "Chrome 151 on macOS" its Platform line reads.
		client: webClient(),
		os: webOs(),
		commit: BUILD_COMMIT,
		platform: webPlatform(),
		language,
		unreachable: [...getFailedRpcChains()].map((id) => chainName(id)),
		failures
	};
}
