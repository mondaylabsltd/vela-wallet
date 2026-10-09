/**
 * Every failed fee run on the signing sheet, and every recovery, as a `fee:`
 * line and a panel counter (spec 082 G61, RJ12).
 *
 * The sheet used to run its own re-quote scheduler here (`FeeRequoteTimer`,
 * spec 079) and keep the failure on screen through each re-ask itself
 * (`heldFeeFailure`). Both are the core's now (PR 2 note 1): a failure that
 * can pass is asked again by the fee machine on its own schedule — 3 s, 6 s,
 * then every 8 s, through the `start_ttl` timer the fee session answers — and
 * `FeeView.failure` says it once for the row and the footer, kept through the
 * re-ask (`retrying`). A second scheduler here would ask twice.
 *
 * What is left is the diagnosis a bug report needs: which chain, which cause,
 * whether the core asks again, and how many failed runs a recovery took.
 * Never a host, never a value.
 */
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import type { FeeView } from '$lib/core/generated/FeeView';
import { countPanelFailure } from '$lib/services/bug-report';

/**
 * A fee failure as a log word and a counter class (spec 082 RJ12, G61):
 * the wire name, or `chain_read` / `chain_read_rate_limited` for the one
 * failure that is an object.
 */
export function feeFailureCause(failure: FeeFailure): string {
	if (typeof failure === 'string') return failure;
	return failure.chain_read.rate_limited ? 'chain_read_rate_limited' : 'chain_read';
}

/** What the log reads of a fee view. */
export type FeeFailureInput = Pick<FeeView, 'busy' | 'failed' | 'failure' | 'fee'>;

export interface FeeFailureLogDeps {
	/** Test seam; `console.log` by default. */
	log?: (line: string) => void;
	/** Test seam; the bug report's panel counter by default. */
	count?: (cause: string) => void;
}

export class FeeFailureLog {
	readonly #log: (line: string) => void;
	readonly #count: (cause: string) => void;
	/** Runs that ended failed since the fee last stood. */
	#runs = 0;
	/** The last view said a run ended failed (and no run has gone out since). */
	#failed = false;

	constructor(deps: FeeFailureLogDeps = {}) {
		this.#log = deps.log ?? ((line) => console.log(line));
		this.#count = deps.count ?? ((cause) => countPanelFailure(`fee.quote_failed.${cause}`));
	}

	/**
	 * Feed every fee view of the request on the sheet (`null`: no request, or
	 * another request's view). A run that ends failed is said once — however
	 * many views repeat it — and a fee that lands after failures says how
	 * many it took.
	 */
	observe(fee: FeeFailureInput | null, chainId: number | null): void {
		if (fee === null) {
			this.#runs = 0;
			this.#failed = false;
			return;
		}
		// A run is out (the core's own re-ask, a tap, a new speed): the next
		// failure is a new one.
		if (fee.busy) {
			this.#failed = false;
			return;
		}
		const chain = String(chainId ?? 'unknown');
		if (fee.failed !== null) {
			if (this.#failed) return;
			this.#failed = true;
			this.#runs += 1;
			const cause = feeFailureCause(fee.failed);
			this.#count(cause);
			const next = fee.failure?.auto_retry ? 'the core asks again' : 'no re-quote';
			this.#log(`fee: quote failed chain=${chain} cause=${cause} run #${this.#runs}, ${next}`);
			return;
		}
		this.#failed = false;
		if (fee.fee !== null && this.#runs > 0) {
			this.#log(`fee: quote back chain=${chain} after ${this.#runs} failed runs`);
		}
		if (fee.fee !== null) this.#runs = 0;
	}
}
