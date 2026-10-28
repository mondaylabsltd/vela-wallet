/**
 * One send at a time per network and account, across this browser's tabs
 * (round 3, send-15).
 *
 * Two tabs that confirm a send from the same account on the same network read
 * the same nonce, sign it twice, and the chain takes one: the other fails as
 * "another transaction went first". The Web Locks API serialises the part
 * that must not overlap — nonce read → sign → submit — and the lock is held
 * on after the submit until that operation is no longer in flight here (its
 * verdict, or the tracker's stall line), bounded, so the tab that goes next
 * reads the nonce after it.
 *
 * No Web Locks (an older browser, a non-secure context): today's behaviour —
 * the relay's `nonce_in_flight`, the core's held confirm and the "went
 * first" ending are what keep the two apart. A lock is a courtesy, never a
 * correctness gate.
 */

/** The lock's name: one per network and account, lower-cased. */
export function sendLockName(chainId: number, account: string): string {
	return `vela.send:${chainId}:${account.toLowerCase()}`;
}

/** How long a submitted operation keeps the lock, at most. */
export const SEND_LOCK_HOLD_MS = 120_000;

/** Lets the lock go. Idempotent. */
export type SendLockRelease = () => void;

interface LockManagerLike {
	request(
		name: string,
		options: { ifAvailable?: boolean; signal?: AbortSignal },
		callback: (lock: unknown) => Promise<void> | void
	): Promise<unknown>;
}

function lockManager(): LockManagerLike | null {
	const locks = (globalThis.navigator as { locks?: LockManagerLike } | undefined)?.locks;
	return locks && typeof locks.request === 'function' ? locks : null;
}

/**
 * Take the lock, holding it until the release is called. Resolves with
 * `null` when there is no Web Locks API (go on as before). When another tab
 * holds it, `onQueued` is told once, and the promise waits its turn; `signal`
 * gives up the wait (rejects with its reason).
 */
export function acquireSendLock(
	name: string,
	options: { onQueued?: () => void; signal?: AbortSignal } = {}
): Promise<SendLockRelease | null> {
	const locks = lockManager();
	if (!locks) return Promise.resolve(null);
	const hold = (resolve: (release: SendLockRelease) => void) => (lock: unknown) => {
		if (!lock) return;
		return new Promise<void>((done) => {
			let released = false;
			resolve(() => {
				if (released) return;
				released = true;
				done();
			});
		});
	};
	return new Promise<SendLockRelease | null>((resolve, reject) => {
		let taken = false;
		const take = (release: SendLockRelease) => {
			taken = true;
			resolve(release);
		};
		locks
			.request(name, { ifAvailable: true }, (lock) => {
				if (lock) return hold(take)(lock);
				// Another tab is sending from this account here: say so, and wait.
				options.onQueued?.();
				void locks
					.request(name, options.signal ? { signal: options.signal } : {}, hold(take))
					.catch((error: unknown) => {
						if (!taken) reject(error);
					});
			})
			.catch((error: unknown) => {
				// The API is there but refused (a sandboxed frame): as if absent.
				if (!taken) resolve(null);
				else console.warn('[send] lock:', error);
			});
	});
}

/**
 * Hold `release` until `settled()` says the operation is no longer in flight
 * — checked whenever `subscribe` calls back — or `SEND_LOCK_HOLD_MS` passes.
 */
export function releaseWhen(
	release: SendLockRelease,
	settled: () => boolean,
	subscribe: (listener: () => void) => () => void,
	holdMs = SEND_LOCK_HOLD_MS
): void {
	if (settled()) {
		release();
		return;
	}
	let done = false;
	let unsubscribe: (() => void) | null = null;
	const finish = () => {
		if (done) return;
		done = true;
		clearTimeout(timer);
		unsubscribe?.();
		release();
	};
	const timer = setTimeout(finish, holdMs);
	unsubscribe = subscribe(() => {
		if (settled()) finish();
	});
	if (done) unsubscribe();
}
