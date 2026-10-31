/**
 * A key ceremony on the trusted signing page (spec 102 R3, R8) — creating a
 * key, signing in, a proof, a member proof — when the core names a `page` on
 * the operation. The web's twin of the apps' `onTheTrustedSigner`
 * (`OnboardingExecutor.swift`).
 *
 * The core names a page for a custom-domain wallet (only its page can mint or
 * use its keys), and — in the Safari package — for every ceremony: WebKit
 * refuses an extension page a `getvela.app` passkey, so the Safari wallet
 * creates, signs in and proves on `sign.getvela.app` (`$lib/signing/surface.ts`).
 *
 * The same hand-off every signature uses (`$lib/signing/page-handoff.svelte.ts`):
 * the page checked as the apps check it (R6), the card, Open, and the page's
 * answer accepted only as the core accepts it — the ceremony asked for, from
 * the page this opened (the origin its authenticator signed), over a challenge
 * the page derived itself, and for a member proof over exactly the challenge
 * this shell fetched (`trustedSignerCeremonyVerify`). The request is the
 * core's, built from the operation as the machine sent it.
 */
import {
	trustedSignerCeremonyCard,
	trustedSignerCeremonyRequest,
	trustedSignerCeremonyVerify,
	type CeremonyAnswer
} from '$lib/core/kernels';
import {
	browserDeps,
	handOff,
	PageHandoffError,
	type PageHandoffDeps
} from '$lib/signing/page-handoff.svelte';
import { PasskeyError } from './passkey';
import * as Registry from './registry';
import type { ShellOperation } from '../generated/ShellOperation';

/** The four operations a page can run as a ceremony. */
export type PageCeremony = Extract<
	ShellOperation,
	{ type: 'register_passkey' | 'authenticate_passkey' | 'sign_proof' | 'sign_member_proof' }
>;

/** Stands in for the browser in tests. */
let deps: PageHandoffDeps | undefined;
export function setCeremonyHandoffDeps(next: PageHandoffDeps | undefined): void {
	deps = next;
}

/** The page an operation's ceremony runs on, or `null` — in this shell. */
export function pageOf(operation: { page?: string | null }): string | null {
	return typeof operation.page === 'string' && operation.page !== '' ? operation.page : null;
}

function originOf(url: string): string {
	try {
		return new URL(url).origin;
	} catch {
		return '';
	}
}

/**
 * Run `operation` on its page and bring back the machine's own
 * `Registration` / `Assertion`. Throws a {@link PasskeyError} the executor
 * reports as any ceremony's failure: the person's "no" (the page closed, or
 * declined there) is `cancelled`; a page that could not be opened, did not
 * answer, refused, or answered with something the core would not take is
 * `other`, with the reason.
 */
export async function ceremonyOnPage(
	operation: PageCeremony,
	options: {
		walletName: string;
		/** A member proof: the registry's challenge this shell fetched, hex. */
		memberChallengeHex?: string | null;
		signal?: AbortSignal;
	}
): Promise<CeremonyAnswer> {
	const page = pageOf(operation);
	if (page === null) throw new PasskeyError('other', 'no signing page named for this ceremony');
	const card = trustedSignerCeremonyCard(operation);
	// A member proof needs the registry's deployment: the page computes the
	// challenge itself, and the published page reaches no network (076).
	const deployment = operation.type === 'sign_member_proof' ? await Registry.deployment() : null;
	const id = `vc-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
	const request = trustedSignerCeremonyRequest({
		operation,
		id,
		walletName: options.walletName,
		registry: Registry.registryUrl(),
		deployment
	});
	if (card === null || request === null) {
		throw new PasskeyError('other', `not a ceremony the signing page runs: ${operation.type}`);
	}
	const signerOrigin = originOf(page);
	try {
		return await handOff<CeremonyAnswer>(
			{
				kind: 'ceremony',
				page,
				signer: options.walletName,
				keyLabel: card.keyLabel,
				titleKey: card.titleKey,
				// The hand-off names its own request id; the core's goes no further.
				request: { intent: request.intent, context: request.context },
				accept: (answer) =>
					trustedSignerCeremonyVerify({
						operation,
						answer,
						signerOrigin,
						expectedMemberChallengeHex: options.memberChallengeHex ?? null
					}),
				signal: options.signal
			},
			deps ?? browserDeps
		);
	} catch (error) {
		if (!(error instanceof PageHandoffError)) throw error;
		if (error.byPerson) throw new PasskeyError('cancelled', 'The signing page was closed');
		throw new PasskeyError('other', pageFailureText(error));
	}
}

/** The reason, for the prompt's detail and the bug report — never a key's secret. */
function pageFailureText(error: PageHandoffError): string {
	switch (error.failure) {
		case 'not_opened':
			return 'The signing page did not pass its check, so it was not opened';
		case 'silent':
			return 'The signing page did not answer';
		case 'unverified':
			return `The signing page's answer did not check out (${error.code})`;
		case 'declined':
			return 'The signing page refused this request';
		default:
			return `The signing page did not finish (${error.failure})`;
	}
}
