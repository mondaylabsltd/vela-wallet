/**
 * The Clear Signer's one conversation at a time — APP-RESIDENT (spec 071,
 * extended by 075).
 *
 * A request can start from the dApp sheet, the wallet's own send, the key
 * backup, and — since 075 — from creating a wallet or signing in. The person
 * waits for the page the same way in each: where is it, the pairing code if it
 * is on another device, "waiting for the Clear Signer…", open it again,
 * cancel. So that state lives here, once, and the sheet
 * (`ClearSignerSheet.svelte`, drawn by `<SigningHost>` and by the onboarding
 * screens) renders it.
 *
 * What is the core's stays the core's: the request (`clearSignerRequest` /
 * `clearSignerCeremonyRequest`), the page (`sign_pref`), the verdict
 * (`clearSignerVerify` / `clearSignerVerifyCeremony`). What this adds is the
 * person's side — where their signer is, the six digits they compare, and the
 * sentence a refusal ends with — plus the one rule a session needs: a flow's
 * requests share one page visit, and the visit ends when the flow does.
 */

import { loadCore } from '$lib/core/client';
import { clearSignerRequest, type ClearSignerInput, type ClearSignerKey } from '$lib/core/kernels';
import type { Assertion } from '$lib/onboarding/core/passkey';
import { signPreference } from '$lib/settings/core/sign-pref.svelte';
import {
	openPostMessageChannel,
	openTunnelChannel,
	signerPageUrl,
	type ClearSignerChannel,
	type ClearSignerHost,
	type ClearSignerReply,
	type TunnelChannel,
	type TunnelSocket
} from '../clear-signer-channel';
import {
	ClearSignerRefusedError,
	noticeOf,
	signOnChannel,
	type ClearSignerNotice
} from '../clear-signer';

/** Where the person keeps their Clear Signer for this flow. */
export type ClearSignerWhere = 'this_device' | 'other_device';

export interface ClearSignerSessionView {
	/** The question is up: this device, or another one? Nothing has opened yet. */
	asking: boolean;
	/**
	 * Pairing with another device: the link to show as a QR and to copy, and
	 * the six digits once both ends have derived them (`null` while the other
	 * device has not arrived, or after it dropped out).
	 */
	pairing: { link: string; code: string | null } | null;
	/** The page has the request and this wallet is waiting on its answer. */
	waiting: boolean;
	/** How the last request ended, until the person dismisses it. */
	notice: ClearSignerNotice | null;
}

const IDLE: ClearSignerSessionView = {
	asking: false,
	pairing: null,
	waiting: false,
	notice: null
};

class ClearSignerSession {
	view = $state<ClearSignerSessionView>(IDLE);

	/** Test seams: the browser a window is opened in, and the tunnel's socket. */
	host: ClearSignerHost | undefined = undefined;
	sockets: ((url: string) => TunnelSocket) | undefined = undefined;

	#channel: ClearSignerChannel | null = null;
	/** The page this channel reaches, for the core's verdicts. */
	#origin = '';
	/** The page address the open channel was opened for (`…/sign.html?ch=…`). */
	#page = '';
	/** The person's answer to "where", pending. */
	#asking: ((where: ClearSignerWhere | null) => void) | null = null;
	/** The person pressed Cancel: they know, so no sentence follows. */
	#cancelled = false;
	/** The tunnel channel, while one is up: only it has a code to confirm. */
	#tunnel: TunnelChannel | null = null;

	/** The page in force for a request that does not name its own. */
	async #defaultUrl(): Promise<string> {
		await Promise.all([loadCore(), signPreference.ready()]);
		return signPreference.view.signer_url;
	}

	/**
	 * The channel for `signerUrl`, opening one — and asking the person where
	 * their signer is — when there is none. `null` when they backed out.
	 */
	async #channelFor(signerUrl: string): Promise<ClearSignerChannel | null> {
		// The PAGE address decides, not the string it was asked for: a flow
		// names the same page two ways — Settings' `…/` and the origin the core
		// stamped on the key — and a session that compared the raw text would
		// close the page between a create and its member proof, and ask the
		// person where their signer is all over again.
		const page = signerPageUrl(signerUrl);
		if (this.#channel !== null && !this.#channel.ended && this.#page === page) {
			return this.#channel;
		}
		this.#closeChannel();
		const where = await this.#askWhere();
		if (where === null) return null;
		this.#page = page;
		this.#origin = originOf(signerUrl);
		if (where === 'this_device') {
			this.#channel = openPostMessageChannel({ signerUrl, host: this.host });
			this.view = { ...this.view, asking: false, pairing: null };
			return this.#channel;
		}
		// A tunnel that cannot even be addressed is the likeliest failure of the
		// whole route, and the person gets its own sentence rather than "something
		// went wrong": the room is what could not be reached, and this device's
		// own page is still there.
		let tunnel: TunnelChannel;
		try {
			tunnel = await openTunnelChannel({
				tunnelUrl: signPreference.view.tunnel_url,
				signerUrl,
				onLink: (link) => {
					this.view = { ...this.view, asking: false, pairing: { link, code: null } };
				},
				onCode: (code) => {
					const pairing = this.view.pairing;
					if (pairing !== null) this.view = { ...this.view, pairing: { ...pairing, code } };
				},
				sockets: this.sockets
			});
		} catch (error) {
			this.#page = '';
			throw new ClearSignerRefusedError(
				'tunnel_down',
				error instanceof Error ? error.message : String(error)
			);
		}
		this.#tunnel = tunnel;
		this.#channel = tunnel;
		return tunnel;
	}

	/** Put the question up and wait. A second question supersedes the first. */
	#askWhere(): Promise<ClearSignerWhere | null> {
		this.#asking?.(null);
		return new Promise((resolve) => {
			this.#asking = resolve;
			this.view = { asking: true, pairing: null, waiting: false, notice: null };
		});
	}

	/** The person answered the "where" question. */
	answerWhere(where: ClearSignerWhere): void {
		const asking = this.#asking;
		this.#asking = null;
		if (asking === null) return;
		// The pairing sheet or the popup follows this tap, which is also the
		// user activation a popup needs.
		this.view = { ...this.view, asking: false };
		asking(where);
	}

	/** The person says both screens show the same six digits. */
	confirmCode(): void {
		this.#tunnel?.confirm();
		const pairing = this.view.pairing;
		// The code is answered: the wait is now on the other device's person.
		if (pairing !== null) this.view = { ...this.view, pairing: null, waiting: true };
	}

	#closeChannel(): void {
		this.#channel?.end();
		this.#channel = null;
		this.#tunnel = null;
		this.#page = '';
	}

	/** Run one request on the flow's channel, opening one if needed. */
	async #run(
		request: { intent: unknown; context: unknown },
		signerUrl: string
	): Promise<{ reply: ClearSignerReply; signerOrigin: string }> {
		let channel: ClearSignerChannel | null;
		try {
			channel = await this.#channelFor(signerUrl);
		} catch (error) {
			// The channel could not be opened at all (an unreachable tunnel). Its
			// sentence is shown, and the request is answered rather than hanging.
			const refused = error instanceof ClearSignerRefusedError ? error : null;
			const code = refused?.code ?? 'malformed';
			this.noteRefusal(code);
			return {
				reply: { kind: 'refused', code, detail: refused?.detail ?? String(error) },
				signerOrigin: ''
			};
		}
		if (channel === null) {
			// They backed out of the question: a cancelled sheet, nothing more.
			this.#cancelled = true;
			this.view = IDLE;
			return { reply: { kind: 'refused', code: 'declined', detail: '' }, signerOrigin: '' };
		}
		this.#cancelled = false;
		// The page THIS request went to, read before the wait: a later request
		// may open another page, and the core must judge the answer against the
		// origin that actually saw it.
		const origin = this.#origin;
		// While the pairing code is up the person is not waiting on the page
		// yet; every other moment is a wait.
		this.view = { ...this.view, waiting: this.view.pairing === null, notice: null };
		const reply = await channel.ask(request);
		if (this.#channel !== channel) {
			// Superseded: the next request owns the sheet now.
			return {
				reply: { kind: 'refused', code: 'declined', detail: 'superseded' },
				signerOrigin: origin
			};
		}
		if (reply.kind === 'refused') {
			// A dead channel is not reused; a page that merely said no stays open
			// for the rest of the flow.
			if (channel.ended) this.#closeChannel();
			this.view = {
				asking: false,
				pairing: null,
				waiting: false,
				notice: this.#cancelled ? null : noticeOf(reply.code)
			};
		} else {
			this.view = { asking: false, pairing: null, waiting: false, notice: null };
		}
		return { reply, signerOrigin: origin };
	}

	/**
	 * Sign `digest` on the Clear Signer. Resolves with the assertion the core
	 * accepted — the same shape a passkey's is, so the caller continues exactly
	 * as it would have — or rejects with `ClearSignerRefusedError`.
	 *
	 * `signerUrl` is the page a KEY lives behind (`signRoute`); without one the
	 * page from Settings.
	 */
	async sign(
		input: ClearSignerInput,
		digest: Uint8Array,
		keys: ClearSignerKey[],
		signerUrl?: string
	): Promise<Assertion> {
		// One at a time: a new signature supersedes a forgotten one, as a
		// second passkey ceremony aborts the first.
		this.#closeChannel();
		const url = signerUrl && signerUrl !== '' ? signerUrl : await this.#defaultUrl();
		const request = clearSignerRequest(input);
		// A channel that cannot be opened (an unreachable tunnel) rejects with its
		// own sentence; every caller already treats that as "nothing was signed".
		let channel: ClearSignerChannel | null;
		try {
			channel = await this.#channelFor(url);
		} catch (error) {
			const refused =
				error instanceof ClearSignerRefusedError
					? error
					: new ClearSignerRefusedError('malformed', String(error));
			this.noteRefusal(refused.code);
			throw refused;
		}
		if (channel === null) {
			this.view = IDLE;
			throw new ClearSignerRefusedError('declined', 'no signer chosen');
		}
		this.#cancelled = false;
		this.view = { ...this.view, waiting: this.view.pairing === null, notice: null };
		const outcome = await signOnChannel(channel, request, digest, keys);
		if (this.#channel !== channel) {
			throw outcome.kind === 'accepted'
				? new ClearSignerRefusedError('declined', 'superseded')
				: new ClearSignerRefusedError(outcome.code, outcome.detail);
		}
		// A signature is a flow of one: the page has nothing left to do.
		this.#closeChannel();
		if (outcome.kind === 'accepted') {
			this.view = IDLE;
			return outcome.assertion;
		}
		this.view = {
			asking: false,
			pairing: null,
			waiting: false,
			notice: this.#cancelled ? null : noticeOf(outcome.code)
		};
		throw new ClearSignerRefusedError(outcome.code, outcome.detail);
	}

	/**
	 * Put one KEY CEREMONY on the flow's channel (075). Unlike a signature the
	 * channel stays open: a create is followed by its member proof, a sign-in
	 * by two recovery proofs, on the same page visit. `endFlow()` closes it.
	 */
	async ceremony(
		request: { intent: unknown; context: unknown },
		signerUrl?: string
	): Promise<{ reply: ClearSignerReply; signerOrigin: string }> {
		const url = signerUrl && signerUrl !== '' ? signerUrl : await this.#defaultUrl();
		return this.#run(request, url);
	}

	/** The flow is over (or abandoned): say goodbye to the page. */
	endFlow(): void {
		this.#asking?.(null);
		this.#asking = null;
		this.#closeChannel();
		if (this.view.notice === null) this.view = IDLE;
		else this.view = { asking: false, pairing: null, waiting: false, notice: this.view.notice };
	}

	/**
	 * The page answered and the CORE would not take it (a wrong challenge, a
	 * foreign key). The channel knows nothing of that verdict, so the caller
	 * that asked for it says which sentence the sheet ends on.
	 */
	noteRefusal(code: string): void {
		this.view = {
			asking: false,
			pairing: null,
			waiting: false,
			notice: this.#cancelled ? null : noticeOf(code)
		};
	}

	reopen(): void {
		this.#channel?.reopen();
	}

	cancel(): void {
		this.#cancelled = true;
		this.#asking?.(null);
		this.#asking = null;
		this.#channel?.cancel();
		this.#closeChannel();
		this.view = IDLE;
	}

	dismiss(): void {
		this.view = { ...this.view, notice: null };
	}
}

/** The origin of a page address; empty when it is not one. */
function originOf(url: string): string {
	try {
		return new URL(url).origin;
	} catch {
		return '';
	}
}

export const clearSignerSession = new ClearSignerSession();
