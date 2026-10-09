/**
 * Possession-proven publish of a wallet's founding key set as one registry
 * group.
 *
 * The mechanism a `registry_publish` operation runs: take (or mint) the
 * one-time software group key, ask the server for the group and member
 * challenges, replay each member's creation-time proof — or sign live for one
 * that has none — build every proof in the Rust core, register, and wait for
 * the task to land.
 *
 * All randomness and all authenticator ceremonies live here in the shell; all
 * byte layout and P-256 math live in the core. This module holds neither.
 */

import { buildGroupProof, buildMemberProof, groupPublicKeyFromSeed, toHex } from './wasm-client';
import * as Registry from './registry';
import * as Passkey from './passkey';
import type { RegistryProof } from '../generated/RegistryProof';
import type { RegistryPublishMember } from '../generated/RegistryPublishMember';

/**
 * One member as the REGISTRY wants it.
 *
 * Deliberately spelled out rather than spread from `RegistryPublishMember`.
 * The core's wire type is snake_case because it is generated from Rust; the
 * registry's HTTP API is camelCase. Spreading the core type into the request
 * body sends `public_key_hex` where the server reads `publicKey`, and the
 * server answers `members[0]: publicKey is required` — after the person has
 * already minted and confirmed every key. The two vocabularies meet here, in
 * one function, and nowhere else.
 */
type RegistryApiMember = {
	publicKey: string;
	attestation?: string;
	credentialId?: string;
	authenticatorAttachment?: string;
	transports?: string;
	proof: RegistryProof;
};

export type PublishArgs = {
	rpId: string;
	/**
	 * Spec 102 R3: the page a member with no replayable proof would sign its
	 * live proof on — a custom-domain wallet's. The web opens no page, so such
	 * a member is refused here, named. `null`: the app.
	 */
	page?: string | null;
	/** The group's opaque metadata blob, already hex-encoded by the core. */
	metadataHex: string;
	/** The founding passkeys, in canonical founding order. */
	members: RegistryPublishMember[];
	/** The group key minted at onboarding start. Member proofs collected at
	 *  creation are only valid under the SAME group key, so the pair travels
	 *  together; empty on the login re-publish, which mints a fresh one. */
	seedHex: string;
	groupPublicKeyHex: string;
	/** Issue 409: the core's `answer_when_accepted` — a one-key create is
	 *  answered at the registry's 202 and its landing confirmed later by the
	 *  session's landing watch. Absent ⇒ wait for the landing, as always. */
	answerWhenAccepted?: boolean;
};

/** What the register's answer leaves to do. */
export type AfterRegister =
	/** The group has a receipt on-chain (or the identical one already had). */
	| { kind: 'landed' }
	/** Answer the core now with this task (issue 409). */
	| { kind: 'accepted'; taskId: string }
	/** Poll this task until the group has landed, then answer. */
	| { kind: 'await_landing'; taskId: string };

/**
 * The one new rule of issue 409, apart from the HTTP so it can be tested:
 * when the core asked to be answered on acceptance, the 202's task id IS the
 * answer; otherwise the landing is waited for, as it always was. `done` up
 * front means the identical group was already on-chain — idempotent by
 * content hash, and just as landed as a fresh one.
 */
export function afterRegister(
	accepted: { id?: string; status: string },
	answerWhenAccepted: boolean
): AfterRegister {
	if (accepted.status === 'done') return { kind: 'landed' };
	if (!accepted.id) throw new Error('register was accepted without a task id');
	return answerWhenAccepted
		? { kind: 'accepted', taskId: accepted.id }
		: { kind: 'await_landing', taskId: accepted.id };
}

function stripHex(value: string): string {
	return value.startsWith('0x') ? value.slice(2) : value;
}

/**
 * Resolves with the registry task when the core asked to be answered on
 * acceptance and the group has not landed yet; `null` once it has landed.
 */
export async function publish(args: PublishArgs): Promise<string | null> {
	if (args.members.length === 0) throw new Error('registry publish needs at least one member');

	let seedHex = args.seedHex;
	let groupPublicKey = args.groupPublicKeyHex;
	if (!seedHex || !groupPublicKey) {
		const seed = new Uint8Array(32);
		crypto.getRandomValues(seed);
		seedHex = toHex(seed, false);
		groupPublicKey = groupPublicKeyFromSeed(seedHex);
	}

	const challenge = await Registry.groupChallenge({
		rpId: args.rpId,
		metadata: args.metadataHex,
		groupPublicKey,
		members: args.members.map((member) => ({
			publicKey: member.public_key_hex,
			attestation: member.attestation_hex
		}))
	});

	const proven: RegistryApiMember[] = [];
	for (const member of args.members) {
		let proof = member.proof ?? undefined;
		if (!proof) {
			const derived = challenge.members.find(
				(candidate) => candidate.publicKey.toLowerCase() === member.public_key_hex.toLowerCase()
			);
			if (!derived) {
				throw new Error(`registry challenge is missing member ${member.public_key_hex}`);
			}
			// Spec 102 R3: a custom-domain wallet's member proves itself on its
			// page, and the web opens none (owner, 2026-09-23). No platform sheet
			// can see that key — asking the OS would find nothing, and signing
			// with the wallet's own rpId would produce an assertion the registry
			// can never verify. So it stops here, named.
			if (args.page) {
				throw new Error(`this key proves itself on ${args.page}, which only the Vela app can open`);
			}
			const assertion = await Passkey.sign(stripHex(derived.challenge), member.credential_id);
			proof = buildMemberProof(
				assertion.authenticatorDataHex,
				assertion.clientDataJSONHex,
				assertion.signatureHex
			) as RegistryProof;
		}
		proven.push({
			publicKey: member.public_key_hex,
			attestation: member.attestation_hex,
			credentialId: member.credential_id,
			authenticatorAttachment: member.authenticator_attachment,
			transports: member.transports,
			proof
		});
	}

	// The group key silently closes over the content hash.
	const group = buildGroupProof(seedHex, args.rpId, challenge.groupChallenge.challenge) as {
		proof: RegistryProof;
	};

	const accepted = await Registry.registerGroup({
		rpId: args.rpId,
		metadata: args.metadataHex,
		groupPublicKey,
		groupProof: group.proof,
		members: proven
	});

	const next = afterRegister(accepted, args.answerWhenAccepted ?? false);
	switch (next.kind) {
		case 'landed':
			return null;
		case 'accepted':
			return next.taskId;
		case 'await_landing':
			await Registry.awaitTask(next.taskId);
			return null;
	}
}
