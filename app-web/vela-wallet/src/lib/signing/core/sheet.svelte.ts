/**
 * The signing sheet's two per-request machines (spec 026 T243).
 *
 * `clear_signing` and `approval_guard` are per-REQUEST, not app-resident: the
 * clear-signing machine resolves one request at a time and supersedes anything
 * in flight, and the guard's editor belongs to the approval on screen. This
 * class owns exactly one of each for the request `sign_request` is currently
 * showing, and disposes them the moment it goes away.
 *
 * Nothing here decides anything either: it forwards the arrived request to
 * both machines and republishes their views. Which of them has something to
 * say — and what — is theirs.
 */
import { loadCore } from '$lib/core/client';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { GuardEvent } from '$lib/core/generated/GuardEvent';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { SignRequestView } from '$lib/core/generated/SignRequestView';
import { createApprovalGuardSession, type ApprovalGuardSession } from './guard-session';
import { createClearSigningSession, type ClearSigningSession } from './clear-session';
import { toClearLocale } from './clear-types';

/** The machines' own initial projections — mirrored until their first views land. */
export const INITIAL_CLEAR_VIEW: ClearSigningView = {
	resolving: false,
	resolved: false,
	result: null,
	message: null,
	surface: 'none',
	confirm: { type: 'confirm' },
	blind_typed: null,
	danger_haptic: false
};

export const INITIAL_GUARD_VIEW: GuardView = {
	surface: 'none',
	detected: null,
	meta: { symbol: '…', decimals: 18, verified: false, loading: false },
	editor: null,
	confirm_allowed: true,
	rewritten_params_json: null,
	unlimited_consented: false,
	increase_total: null,
	decimals_unverified: false,
	expired: false,
	batch: null
};

/** A transaction's three params, as the two machines need them. */
interface TxParams {
	to: string | null;
	data: string | null;
	value: string | null;
}

function txParams(paramsJson: string): TxParams | null {
	try {
		const params = JSON.parse(paramsJson) as unknown[];
		const tx = params[0] as { to?: string; data?: string; value?: string } | undefined;
		if (!tx || typeof tx !== 'object') return null;
		return { to: tx.to ?? null, data: tx.data ?? null, value: tx.value ?? null };
	} catch {
		return null;
	}
}

class SigningSheet {
	clear = $state<ClearSigningView>(INITIAL_CLEAR_VIEW);
	guard = $state<GuardView>(INITIAL_GUARD_VIEW);

	#clearSession: ClearSigningSession | null = null;
	#guardSession: ApprovalGuardSession | null = null;
	#requestId: string | null = null;

	/**
	 * A request arrived (or changed). Both machines are rebuilt for it — the
	 * clear-signing one because it supersedes rather than accumulates, the
	 * guard because its editor belongs to this approval and no other.
	 */
	async present(request: SignRequestView, walletAddress: string | null): Promise<void> {
		if (this.#requestId === request.id) return;
		this.dismiss();
		this.#requestId = request.id;
		await loadCore();
		// The request may have gone while the core loaded.
		if (this.#requestId !== request.id) return;

		this.#clearSession = createClearSigningSession({
			onView: (view) => (this.clear = view),
			onError: (error) => console.error('[clear_signing] core fault:', error)
		});
		this.#guardSession = createApprovalGuardSession({
			onView: (view) => (this.guard = view),
			onError: (error) => console.error('[approval_guard] core fault:', error)
		});

		// The person's own presets. The core owns WHICH number is shown; these
		// say how digits group, which is a locale fact the shell reads.
		const locale = toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' });
		if (request.kind === 'typed_data') {
			const params = JSON.parse(request.params_json) as unknown[];
			const typed = params.find((p) => typeof p === 'string' || typeof p === 'object');
			this.#clearSession.start({
				type: 'resolve_typed_data',
				typed_data_json: typeof typed === 'string' ? typed : JSON.stringify(typed ?? {}),
				chain_id: request.chain_id,
				locale
			});
		} else if (request.kind === 'personal_sign') {
			const params = JSON.parse(request.params_json) as string[];
			this.#clearSession.start({
				type: 'message_presented',
				method: 'personal_sign',
				params: params.map((p) => String(p)),
				request_origin: request.origin
			});
		} else {
			const tx = txParams(request.params_json);
			this.#clearSession.start({
				type: 'resolve_transaction',
				to: tx?.to ?? null,
				data: tx?.data ?? null,
				value: tx?.value ?? null,
				chain_id: request.chain_id,
				locale
			});
		}

		// The guard sees every request: whether one contains an approval — and
		// whether that approval is unbounded — is its ruling, not a filter here.
		this.#guardSession.start({
			type: 'approval_detected',
			method: request.method,
			params_json: request.params_json,
			chain_id: request.chain_id,
			wallet_address: walletAddress,
			read_only: false,
			now_ms: Date.now()
		});
	}

	/** A guard event from the sheet's chips. */
	dispatchGuard(event: GuardEvent): void {
		this.#guardSession?.dispatch(event);
	}

	/** The request is gone: both machines go with it. */
	dismiss(): void {
		this.#clearSession?.dispose();
		this.#guardSession?.dispose();
		this.#clearSession = null;
		this.#guardSession = null;
		this.#requestId = null;
		this.clear = INITIAL_CLEAR_VIEW;
		this.guard = INITIAL_GUARD_VIEW;
	}
}

export const signingSheet = new SigningSheet();
