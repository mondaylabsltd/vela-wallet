/**
 * `SendOperation::PrewarmFees` — read ahead, into the fee executor's OWN
 * caches, what a quote on each of these chains will need.
 *
 * The picker is open and the person is choosing. Measured on the live relay, a
 * first quote is 4.5–6 s and all but its simulation is reads that can be held
 * for 8–15 s: the deployment read (0.8–1.0 s), the chain's gas signals
 * (0.8–1.3 s), the relay's gas quote (2.0–3.1 s) and its in-band rows
 * (1.5–4.4 s). Read now, the quote a pick starts finds them answered and is
 * left with only the simulation.
 *
 * Useless unless it fills the very caches the quote reads, so every call here
 * is the SAME reader, with the same arguments, that `FeeQuote.requestQuote`
 * (`accountIsDeployed`) and `fee-executor.ts` (`fetch_gas_price`,
 * `fetch_bundler_quote`, `fetch_in_band_quotes`, `fetch_fee_recipient`) call:
 *
 * - deployment — held for good once deployed (a contract cannot un-deploy),
 *   never while undeployed; concurrent asks share one read;
 * - gas signals and the relay's gas quote — 15 s per chain
 *   (`feeSignalsCacheTtlMs`), the quote as ONE answer for every tier;
 * - in-band rows — 8 s per chain and account (`bundler-service.ts`);
 * - Tempo's fee recipient — 30 s, in place of the relay gas quote Tempo
 *   never asks for (`fee_policy::begin_pipeline`).
 *
 * Nothing is priced and nothing comes back. Every error is swallowed: a read
 * that fails here is simply read again by the quote, which is where a failure
 * means something.
 */
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { fetchBundlerAccountInfo, fetchInBandGasQuotes } from '$lib/services/bundler-service';
import {
	accountIsDeployed,
	fetchRawBundlerQuote,
	fetchRawGasSignals
} from '$lib/services/safe-transaction';
import { isTempoChain } from '$lib/services/tempo';

/** Start a read and forget it — synchronously thrown or rejected alike. */
function quietly(read: () => Promise<unknown>): void {
	try {
		void read().catch(() => {});
	} catch {
		// A reader that throws before its promise exists warms nothing; the
		// quote will read it for real.
	}
}

/** Fire the reads for every chain and return at once. */
export function prewarmFees(account: string, chainIds: readonly number[], tier: FeeTier): void {
	for (const chainId of chainIds) {
		const tempo = isTempoChain(chainId);
		quietly(() => accountIsDeployed(account, chainId));
		// Tempo is excluded from the tip query (attodollar gas), exactly as the
		// fee machine's own `fetch_gas_price` asks — the cache is keyed by it.
		quietly(() => fetchRawGasSignals(chainId, !tempo));
		if (tempo) quietly(() => fetchBundlerAccountInfo(chainId, account));
		else quietly(() => fetchRawBundlerQuote(chainId, tier));
		quietly(() => fetchInBandGasQuotes(chainId, account));
	}
}
