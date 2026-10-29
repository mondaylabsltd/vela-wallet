/**
 * Execution previews ({@link previewExecution}), shared: the proposal sheet
 * draws its fee line, its "This would fail" and its picker from them, and the
 * home says "Can't run now" from the same ones — one look per execution and
 * executor, kept a short while, never a second answer to the same question.
 */
import {
	previewExecution,
	type ExecutionBlock,
	type ExecutionInput,
	type ExecutionPreview
} from './execute';

/** How long a priced or failing look stands before it is taken again. */
export const PREVIEW_FRESH_MS = 15_000;
/** A look that could not price the fee is taken again sooner. */
export const UNPRICED_FRESH_MS = 5_000;
/**
 * Round 5: the longest one look is waited for. "Approve and execute" once
 * stayed disabled four minutes on a look that never answered; past this the
 * look is said as not checked (`unchecked`) — approving goes ahead, and
 * Execute looks again before its passkey. A look that answers later still
 * lands.
 */
export const PREVIEW_CAP_MS = 15_000;

/** One execution by one executor, at the multisig's state it was looked at in. */
export function previewKey(input: ExecutionInput): string {
	return [
		input.chainId,
		input.multisig.toLowerCase(),
		input.executor.address.toLowerCase(),
		input.members.deployed,
		input.members.nonce.toString(),
		input.members.threshold,
		JSON.stringify(input.tx),
		input.approvals
			.map((a) => a.owner.toLowerCase())
			.sort()
			.join(',')
	].join('|');
}

interface Entry {
	at: number;
	preview: ExecutionPreview;
}

class Preflight {
	#shown = $state<Record<string, ExecutionPreview>>({});
	/** The same entries, read without subscribing (an effect that checks must not loop). */
	#entries = new Map<string, Entry>();
	#running = new Map<string, Promise<ExecutionPreview>>();
	#look: (input: ExecutionInput) => Promise<ExecutionPreview> = (input) => previewExecution(input);
	#capMs = PREVIEW_CAP_MS;

	/** The last look at this execution, or `null` before the first lands. Reactive. */
	get(input: ExecutionInput): ExecutionPreview | null {
		return this.#shown[previewKey(input)] ?? null;
	}

	/** Look (again) unless the last look is fresh. Never throws. */
	check(input: ExecutionInput, freshMs = PREVIEW_FRESH_MS): Promise<ExecutionPreview> {
		const key = previewKey(input);
		const hit = this.#entries.get(key);
		const age = hit ? Date.now() - hit.at : Infinity;
		const soon = hit?.preview.kind === 'unpriced' || hit?.preview.kind === 'unchecked';
		const fresh = soon ? Math.min(freshMs, UNPRICED_FRESH_MS) : freshMs;
		if (hit && age < fresh) return Promise.resolve(hit.preview);
		const running = this.#running.get(key);
		if (running) return running;
		const keep = (preview: ExecutionPreview) => {
			this.#entries.set(key, { at: Date.now(), preview });
			this.#shown = { ...this.#shown, [key]: preview };
			return preview;
		};
		let capped = false;
		let timer: ReturnType<typeof setTimeout> | undefined;
		const answered = this.#look(input)
			.catch((): ExecutionPreview => ({ kind: 'unpriced' }))
			.then((preview) => {
				// Past the cap it lands all the same — unless a newer look has.
				if (!capped || this.#entries.get(key)?.preview.kind === 'unchecked') keep(preview);
				return preview;
			});
		const look = Promise.race([
			answered,
			new Promise<ExecutionPreview>((done) => {
				timer = setTimeout(() => {
					capped = true;
					done(keep({ kind: 'unchecked' }));
				}, this.#capMs);
			})
		]).finally(() => {
			clearTimeout(timer);
			this.#running.delete(key);
		});
		this.#running.set(key, look);
		return look;
	}

	/** Tests: the look itself, and a clean slate. */
	_reset(
		look?: (input: ExecutionInput) => Promise<ExecutionPreview>,
		capMs = PREVIEW_CAP_MS
	): void {
		this.#shown = {};
		this.#entries.clear();
		this.#running.clear();
		this.#look = look ?? ((input) => previewExecution(input));
		this.#capMs = capMs;
	}
}

export const preflight = new Preflight();

/**
 * Why a transaction cannot run now, from the looks at its executors on this
 * device: it would fail (any look says so — the transaction's own fault), or
 * every executor was priced and none holds the fee. `null` while any look is
 * missing or unpriced, or when someone here can run it.
 */
export function blockOf(
	previews: (ExecutionPreview | null)[]
): { kind: 'fails'; why: ExecutionBlock } | { kind: 'nobodyPays' } | null {
	const fails = previews.find((p) => p?.kind === 'fails');
	if (fails?.kind === 'fails') return { kind: 'fails', why: fails.why };
	if (previews.length === 0) return null;
	if (previews.every((p) => p?.kind === 'priced' && !p.canPay)) return { kind: 'nobodyPays' };
	return null;
}
