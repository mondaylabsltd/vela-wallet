/**
 * The network answering again, after a send failed as the network's (round 6).
 *
 * A submit whose reads before the passkey did not get through ends on its
 * review with the network's words — "Couldn't reach Arc Testnet — nothing was
 * signed or sent" — and Confirm & Send shut (`SendTxErrorKey::ChainUnreachable`
 * / `ChainBusy`). It stayed shut after the network came back, until the person
 * found "Try again". While that line is up, this asks the form's network one
 * bounded read at a time — every {@link RECOVERY_EVERY_MS}, and at once when
 * the browser says it is online again — and the first answer is told to the
 * core (`chain_answered`), which opens the confirm; the shell asks the fee
 * again with it. "Online" alone is not an answer: the line stays until the
 * network itself has answered, so it never says more than is known.
 */
import { poolRpcCall } from '$lib/services/rpc-pool';

/** How often the network is asked while its failure is on the review. */
export const RECOVERY_EVERY_MS = 5_000;
/** How long one read may take before it counts as no answer. */
export const RECOVERY_READ_MS = 4_000;

export interface RecoveryDeps {
	/** One read of `chainId` — resolves when the network answered it. */
	read: (chainId: number) => Promise<void>;
	setTimeout: (run: () => void, ms: number) => unknown;
	clearTimeout: (handle: unknown) => void;
	/** The browser's "online" again; returns the unsubscribe. */
	onOnline: (run: () => void) => () => void;
}

/** A block number from the pool, or a throw: an error body is no answer. */
async function readHead(chainId: number): Promise<void> {
	const response = await poolRpcCall('eth_blockNumber', [], chainId);
	if (response.error || typeof response.result !== 'string') {
		throw new Error(response.error?.message ?? 'no block number');
	}
}

const browserDeps: RecoveryDeps = {
	read: readHead,
	setTimeout: (run, ms) => setTimeout(run, ms),
	clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
	onOnline: (run) => {
		if (typeof window === 'undefined') return () => {};
		window.addEventListener('online', run);
		return () => window.removeEventListener('online', run);
	}
};

/**
 * Ask `chainId` until it answers, then `answered(chainId)` once. One read is
 * out at a time, each bounded by {@link RECOVERY_READ_MS}. Returns the stop —
 * called when the line goes for any other reason (the person's own "Try
 * again", leaving the review).
 */
export function watchChainRecovery(
	chainId: number,
	answered: (chainId: number) => void,
	deps: RecoveryDeps = browserDeps
): () => void {
	let stopped = false;
	let reading = false;
	let next: unknown = null;

	const schedule = () => {
		if (stopped || next !== null) return;
		next = deps.setTimeout(() => {
			next = null;
			void ask();
		}, RECOVERY_EVERY_MS);
	};
	const ask = async () => {
		if (stopped || reading) return;
		reading = true;
		let bound: unknown = null;
		const ok = await Promise.race([
			deps.read(chainId).then(
				() => true,
				() => false
			),
			new Promise<boolean>((resolve) => {
				bound = deps.setTimeout(() => resolve(false), RECOVERY_READ_MS);
			})
		]);
		if (bound !== null) deps.clearTimeout(bound);
		reading = false;
		if (stopped) return;
		if (ok) {
			stop();
			answered(chainId);
			return;
		}
		schedule();
	};
	const unOnline = deps.onOnline(() => {
		if (next !== null) {
			deps.clearTimeout(next);
			next = null;
		}
		void ask();
	});
	function stop() {
		stopped = true;
		if (next !== null) deps.clearTimeout(next);
		next = null;
		unOnline();
	}
	schedule();
	return stop;
}
