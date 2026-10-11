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
import { networkForChainId } from '$lib/services/networks';
import { getFailedRpcChains } from '$lib/services/rpc-pool';
import { fill } from '$lib/wallet/messages';

/**
 * The networks a report lists as unreachable, as a person reads them: each by
 * its NAME, and one with no name in the corpus's words for that
 * (`assets.chainFallback`, "Chain 1625" / 「链 1625」) — never a bare id.
 *
 * The owner's phone listed 「无法连接的 RPC 1625」 after every launch, which
 * reads as a count (device walk of 2026-10-11, item 9). The web named its
 * networks already, but a chain it had no name for came out as the English
 * "Chain 1625" whatever the page's language, under a label that called the
 * list "RPCs". The label is "Unreachable networks" now, and the fallback is
 * the page's own.
 *
 * (Why 1625 was in the list at all — the `.g` name service's chain, a network
 * the wallet does not have — is fixed where it was decided: the core's pool no
 * longer counts a chain with no endpoint as one that failed.)
 */
export function unreachableNetworkNames(
	chainIds: Iterable<number>,
	chainFallback: string
): string[] {
	return [...chainIds].map((chainId) => {
		const name = networkForChainId(chainId)?.displayName.trim();
		return name ? name : fill(chainFallback, { chainId });
	});
}

/**
 * This device, now. `workerFailures` are the extension worker's counters
 * (spec 082 RB14), read asynchronously by the caller — empty outside it.
 * `chainFallback` is the page's `bugReport.chainFallback`.
 */
export function deviceFacts(
	language: string,
	workerFailures: readonly string[],
	chainFallback: string
): DeviceFacts {
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
		unreachable: unreachableNetworkNames(getFailedRpcChains(), chainFallback),
		failures
	};
}
