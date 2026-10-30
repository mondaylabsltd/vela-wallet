/**
 * Why a passkey or a send did not happen, as the shell saw it (`live.ts`):
 * JSON in a result's `detail`, which the core hands back in its views. Kept
 * apart from the pipelines so a sheet reads it without loading them.
 */

export type ShellFailure = (
	| { kind: 'venue_blocked'; block: unknown }
	| { kind: 'page_declined' }
	| { kind: 'page_sign_failed'; page: string }
	| { kind: 'page_approval'; failure: string; code: string | null; line: unknown }
	| { kind: 'passkey'; failure: string; timedOut: boolean }
	| { kind: 'passkey_check'; detail: string }
	| { kind: 'not_sent' }
	| { kind: 'cannot_pay'; need: string; have: string }
	| { kind: 'unknown'; message: string }
) & {
	/** What the error itself said, for a sheet that says it as it was (`error.message`). */
	message?: string;
};

export function shellFailure(failure: ShellFailure): string {
	return JSON.stringify(failure);
}

/** A `detail` the shell wrote, or `null` for one it did not (the core's own words). */
export function readShellFailure(detail: string | null | undefined): ShellFailure | null {
	if (!detail || !detail.startsWith('{')) return null;
	try {
		const parsed = JSON.parse(detail) as { kind?: unknown };
		return typeof parsed.kind === 'string' ? (parsed as ShellFailure) : null;
	} catch {
		return null;
	}
}

/**
 * A failure in the words the error itself had (`error.message`), as the
 * sheets said it before the machines; a detail the shell did not write is the
 * core's own words, said as they are.
 */
export function failureText(detail: string | null | undefined): string {
	const said = readShellFailure(detail);
	if (!said) return detail ?? '';
	return typeof said.message === 'string' ? said.message : (detail ?? '');
}
