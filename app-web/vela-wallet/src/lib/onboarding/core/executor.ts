/**
 * The only place onboarding touches the outside world.
 *
 * Each `ShellOperation` the core declares maps to exactly one call — a passkey
 * ceremony, a storage read or write, a registry request, a timer, a prompt.
 * There is no branching on business meaning here: if this file ever grows an
 * `if` that decides what happens next, that decision belongs in the Rust
 * machine instead.
 *
 * Failure contract: nothing rejects into the effect loop. Every rejection is
 * converted into the result variant that operation answers with, so the core
 * keeps ownership of classification.
 */

import * as Passkey from './passkey';
import { PasskeyError } from './passkey';
import * as Registry from './registry';
import { RegistryError } from './registry';
import * as Storage from './storage';
import { StorageError } from './storage';
import { publish } from './publish';
import { trustedSignerRegistryRpId } from '$lib/core/kernels';
import { groupPublicKeyFromSeed, toHex } from './wasm-client';

import type { Assertion as AssertionWire } from '../generated/Assertion';
import type { CompletionMode } from '../generated/CompletionMode';
import type { ProofPurpose } from '../generated/ProofPurpose';
import type { PromptKind } from '../generated/PromptKind';
import type { ShellOperation } from '../generated/ShellOperation';
import type { ShellResult } from '../generated/ShellResult';

/** One effect the core is waiting on. */
export type OnboardingEffect = { id: number; operation: ShellOperation };

export type ExecutorDeps = {
	/** Show a notice or ask a question. `confirmable` selects a two-button
	 *  dialog whose answer is a business decision the core acts on. */
	prompt(kind: PromptKind, confirmable: boolean): Promise<boolean>;
	/** Hand the wallet to the app and leave onboarding. */
	complete(mode: CompletionMode): Promise<void>;
};

export function createOnboardingExecutor(deps: ExecutorDeps) {
	return async function execute(
		effect: OnboardingEffect,
		signal: AbortSignal
	): Promise<ShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'check_passkey_support':
				return { type: 'passkey_support', supported: Passkey.passkeySupported() };

			case 'register_passkey': {
				onThisPage(operation.page);
				// `operation.method` selects the ceremony: the person was asked
				// where the key should live, so it is minted there (founder,
				// 2026-09-26) — not wherever the browser's own sheet looks first.
				const registration = await Passkey.register(
					operation.name,
					operation.exclude_credential_ids,
					operation.method
				);
				return {
					type: 'passkey_registered',
					registration: {
						credential_id: registration.credentialId,
						attestation_object_hex: registration.attestationObjectHex,
						client_data_json_hex: registration.clientDataJSONHex,
						authenticator_attachment: registration.authenticatorAttachment,
						transports: registration.transports
					},
					now_iso: nowIso()
				};
			}

			case 'sign_proof': {
				onThisPage(operation.page);
				// The purpose only selects which challenge label the core minted;
				// the shell signs whatever it is handed, with the key just minted
				// or found, over the method it was minted or found with.
				const assertion = await Passkey.sign(
					challengeFor(operation.purpose),
					operation.credential_id,
					operation.transports,
					operation.method
				);
				return { type: 'proof_signed', assertion: toAssertion(assertion), now_iso: nowIso() };
			}

			case 'generate_group_key': {
				// The one-time software group key — the only randomness in the
				// flow, and it stays in the shell. The core only echoes it into
				// the final publish.
				const seed = new Uint8Array(32);
				crypto.getRandomValues(seed);
				const seedHex = toHex(seed, false);
				return {
					type: 'group_key_generated',
					seed_hex: seedHex,
					group_public_key_hex: groupPublicKeyFromSeed(seedHex)
				};
			}

			case 'sign_member_proof': {
				// Creation-time membership confirmation: fetch the member-mode
				// challenge (it binds only groupPublicKey + own attestation, so it
				// exists before the rest of the set does), sign against exactly
				// this credential, assemble the proof in the core. The publish
				// later replays it without another prompt.
				// Spec 102 R3: a proof is filed under the domain of the page it
				// runs on — none on the web, so always the wallet's own.
				onThisPage(operation.page);
				const challenge = await Registry.memberChallenge({
					rpId: Passkey.relyingPartyId(),
					groupPublicKey: operation.group_public_key_hex,
					publicKey: operation.public_key_hex,
					attestation: operation.attestation_hex
				});
				const assertion = await Passkey.sign(
					stripHex(challenge.challenge),
					operation.credential_id,
					operation.transports,
					operation.method
				);
				const { buildMemberProof } = await import('./wasm-client');
				return {
					type: 'member_proof_signed',
					proof: buildMemberProof(
						assertion.authenticatorDataHex,
						assertion.clientDataJSONHex,
						assertion.signatureHex
					)
				};
			}

			case 'lookup_legacy_name':
				return { type: 'legacy_name', name: await Registry.legacyName(operation.credential_id) };

			case 'authenticate_passkey': {
				onThisPage(operation.page);
				const assertion = await Passkey.authenticate(operation.method);
				return {
					type: 'passkey_authenticated',
					assertion: toAssertion(assertion),
					now_iso: nowIso()
				};
			}

			case 'load_accounts':
				return { type: 'accounts_loaded', accounts: Storage.loadAccounts() };

			case 'save_account':
				Storage.saveAccount(operation.account);
				return { type: 'account_saved' };

			case 'save_pending_upload':
				Storage.savePendingUpload(operation.record);
				return { type: 'pending_upload_saved' };

			case 'remove_pending_upload':
				Storage.removePendingUpload(operation.credential_id);
				return { type: 'pending_upload_removed' };

			case 'registry_publish': {
				const taskId = await publish({
					// One relying party for the whole unit (ruling, 2026-09-23): the
					// domain of the page the core names (spec 102 R3, a custom-domain
					// wallet's), else the wallet's own. The web names none — and a
					// member that would have to prove itself live on a page is
					// refused in `publish`, which knows which members sign live.
					rpId: trustedSignerRegistryRpId(operation.page ?? null) ?? Passkey.relyingPartyId(),
					page: operation.page ?? null,
					metadataHex: operation.metadata_hex,
					members: operation.members,
					seedHex: operation.group_seed_hex,
					groupPublicKeyHex: operation.group_public_key_hex,
					answerWhenAccepted: operation.answer_when_accepted ?? false
				});
				// Issue 409: a one-key create is answered at the 202; its
				// landing is the session's landing watch's to confirm.
				return taskId === null
					? { type: 'registry_published' }
					: { type: 'registry_accepted', task_id: taskId };
			}

			case 'registry_query_by_public_key': {
				const status = await Registry.queryByPublicKey(operation.public_key_hex);
				return {
					type: 'registry_key_status',
					registered: status.registered,
					unit_ids: status.unitIds,
					// Who vouched for it, untouched: only Gnosis's "no record" may
					// offer the rebuild (the core reads anything else as unverified).
					verified_by: status.verifiedBy
				};
			}

			case 'registry_query_unit': {
				const unit = await Registry.queryUnit(operation.unit_id);
				return { type: 'registry_unit', metadata_hex: unit.metadataHex, members: unit.members };
			}

			case 'probe_index_health':
				return { type: 'index_health', ok: await Registry.probeHealth() };

			case 'wait':
				await waitCancellably(operation.ms, signal);
				return { type: 'waited' };

			case 'prompt':
				return {
					type: 'prompt_answered',
					accepted: await deps.prompt(operation.kind, operation.confirmable)
				};

			case 'complete_onboarding':
				await deps.complete(operation.mode);
				return { type: 'onboarding_completed' };

			default: {
				// Exhaustive: a new operation stops compiling here rather than
				// silently answering nothing.
				const never: never = operation;
				throw new Error(`unhandled shell operation: ${JSON.stringify(never)}`);
			}
		}
	};
}

/**
 * Spec 102 R3: a ceremony the core sends to a page runs THERE — only a
 * custom-domain wallet's page can mint or use its keys. The web opens no page
 * (owner, 2026-09-23) and never asks for one (it sends no `signing_page_chosen`
 * and signs in with `page: null`), so a page here is refused before any
 * authenticator is asked: a platform sheet would look under the wallet's own
 * domain and find nothing, or mint a key for the wrong one.
 */
export function onThisPage(page: string | null | undefined): void {
	if (page) throw new Error(`this ceremony runs on ${page}, which only the Vela app can open`);
}

/**
 * The result variant an operation owes when its execution threw.
 *
 * This is the whole failure contract: the effect loop hands every rejection
 * here, and the core sees a described outcome rather than an exception. An
 * operation missing from this map would leave the core waiting forever, so the
 * fallthrough is deliberate and loud.
 */
export function operationFailure(effect: OnboardingEffect, error: unknown): ShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'check_passkey_support':
			return { type: 'passkey_support', supported: false };

		case 'register_passkey':
		case 'sign_proof':
		case 'authenticate_passkey':
			return passkeyFailure(error);

		case 'sign_member_proof':
			// Mixed: the ceremony and the challenge fetch can each fail, and the
			// core branches differently on the two. Classify by what actually
			// threw rather than by which operation it was.
			return error instanceof RegistryError ? indexFailure(error) : passkeyFailure(error);

		case 'generate_group_key':
		case 'load_accounts':
		case 'save_account':
		case 'save_pending_upload':
		case 'remove_pending_upload':
			return { type: 'storage_failed', message: describe(error) };

		case 'registry_publish':
		case 'registry_query_by_public_key':
		case 'registry_query_unit':
			return indexFailure(error);

		case 'lookup_legacy_name':
			// Best-effort and read-only: a lost name degrades the label, never
			// the flow.
			return { type: 'legacy_name', name: null };

		case 'probe_index_health':
			return { type: 'index_health', ok: false };

		case 'wait':
			return { type: 'waited' };

		case 'prompt':
			// A dismissed dialog is a refusal, not an error.
			return { type: 'prompt_answered', accepted: false };

		case 'complete_onboarding':
			// The handover already happened as far as the core is concerned; a
			// failure here is the app's to survive, not the machine's.
			return { type: 'onboarding_completed' };

		default: {
			const never: never = operation;
			throw new Error(`no failure variant for operation: ${JSON.stringify(never)}`);
		}
	}
}

function passkeyFailure(error: unknown): ShellResult {
	if (error instanceof PasskeyError) {
		// A classified failure's copy comes from the classification; only
		// `other` carries the platform's own words, because those go into the
		// bug report and must not be prettified.
		return {
			type: 'passkey_failed',
			kind: error.kind,
			message: error.kind === 'other' ? error.message : null
		};
	}
	return { type: 'passkey_failed', kind: 'other', message: describe(error) };
}

function indexFailure(error: unknown): ShellResult {
	// `network` is the one bit of classification only a shell can supply: a
	// request that never arrived is not the same as one the server refused.
	const network = error instanceof RegistryError ? error.network : true;
	return { type: 'index_failed', message: describe(error), network };
}

/**
 * The challenge a proof purpose signs over.
 *
 * The label strings are preserved verbatim from the shipping Expo client —
 * they are part of the wire, not decoration. The two recovery purposes share a
 * label on purpose: what must differ between the two signatures is the
 * challenge BYTES, and the timestamp tail supplies that. The invariant is not
 * trusted to the shell either way — `recover_public_key_from_assertions`
 * returns nothing unless the two assertions pin down exactly one key, so a
 * repeated challenge fails closed in the core.
 */
function challengeFor(purpose: ProofPurpose): string {
	const label = purpose === 'verify' ? 'vela-verify-' : 'vela-recover-';
	return Passkey.bytesToHex(new TextEncoder().encode(label + Date.now()));
}

function toAssertion(assertion: Passkey.Assertion): AssertionWire {
	return {
		credential_id: assertion.credentialId,
		// `signature_der_hex`, not `signature_hex`: the browser hands back a DER
		// signature and the core normalises it (including low-S) itself. Naming
		// it otherwise would invite a shell to "helpfully" convert first.
		signature_der_hex: assertion.signatureHex,
		authenticator_data_hex: assertion.authenticatorDataHex,
		client_data_json_hex: assertion.clientDataJSONHex,
		// Absent, not empty: no user handle is a different fact from an empty
		// one, and the core's name resolution branches on it.
		user_id_hex: assertion.userIdHex ?? null,
		authenticator_attachment: assertion.authenticatorAttachment
	};
}

/** `wait` is the core's only clock, and the core can cancel it. */
function waitCancellably(ms: number, signal: AbortSignal): Promise<void> {
	return new Promise((resolve, reject) => {
		if (signal.aborted) return reject(signal.reason);
		const timer = setTimeout(() => {
			signal.removeEventListener('abort', onAbort);
			resolve();
		}, ms);
		function onAbort() {
			clearTimeout(timer);
			reject(signal.reason);
		}
		signal.addEventListener('abort', onAbort, { once: true });
	});
}

function stripHex(value: string): string {
	return value.startsWith('0x') ? value.slice(2) : value;
}

function nowIso(): string {
	return new Date().toISOString();
}

function describe(error: unknown): string {
	if (error instanceof StorageError || error instanceof RegistryError) return error.message;
	if (error instanceof Error) return error.message;
	return String(error);
}
