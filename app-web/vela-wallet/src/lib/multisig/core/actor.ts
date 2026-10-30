/**
 * r5reg-5: who of this device's members does the work — proposes, approves,
 * executes — when the device holds more than one member of a multisig. The
 * sheet offers the choice ("As"); left alone, it is the member used here last
 * for this multisig, else the first. Never another one, silently.
 *
 * Remembered per multisig in this browser only (a convenience: a choice not
 * kept is the first member again, and the "As" choice shows it).
 */
import { sameAddress } from './kernel';

const KEY = 'vela.multisig.lastActor:';

class LastActor {
	/** The member of `multisig` used here last, if kept. */
	of(multisig: string): string | null {
		try {
			return localStorage.getItem(KEY + multisig.toLowerCase());
		} catch {
			return null;
		}
	}

	/** `member` of `multisig` did the work here just now. */
	note(multisig: string, member: string): void {
		try {
			localStorage.setItem(KEY + multisig.toLowerCase(), member.toLowerCase());
		} catch {
			/* not kept: the first member is offered next time */
		}
	}
}

export const lastActor = new LastActor();

/** Who proposes: `picked` (the person's pick on the review), else the one used last, else the first. */
export function proposerOf<T extends { address: string }>(
	here: T[],
	picked: string | null,
	multisig: string
): T | undefined {
	return (
		(picked ? here.find((o) => sameAddress(o.address, picked)) : undefined) ??
		defaultActor(here, multisig)
	);
}

/** The member offered first among `candidates`: the one used here last, else the first. */
export function defaultActor<T extends { address: string }>(
	candidates: T[],
	multisig: string
): T | undefined {
	const last = lastActor.of(multisig);
	return (last ? candidates.find((o) => sameAddress(o.address, last)) : undefined) ?? candidates[0];
}
