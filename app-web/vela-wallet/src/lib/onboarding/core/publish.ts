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
import { clearSignerMemberProof } from './clear-signer-ceremony';
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
	/** The group's opaque metadata blob, already hex-encoded by the core. */
	metadataHex: string;
	/** The founding passkeys, in canonical founding order. */
	members: RegistryPublishMember[];
	/** The group key minted at onboarding start. Member proofs collected at
	 *  creation are only valid under the SAME group key, so the pair travels
	 *  together; empty on the login re-publish, which mints a fresh one. */
	seedHex: string;
	groupPublicKeyHex: string;
};

function stripHex(value: string): string {
	return value.startsWith('0x') ? value.slice(2) : value;
}

export async function publish(args: PublishArgs): Promise<void> {
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
			// Spec 075: a member that lives behind a Clear Signer page signs
			// THERE. No platform sheet can see that key, so a live proof asked of
			// the OS would find nothing — and the page it must be asked on is the
			// one the record names, never whichever page Settings happens to hold.
			const behindPage = member.signer_origin ?? '';
			const assertion =
				behindPage === ''
					? await Passkey.sign(stripHex(derived.challenge), member.credential_id)
					: await clearSignerMemberProof({
							credentialIdHex: member.credential_id,
							publicKeyHex: member.public_key_hex,
							attestationHex: member.attestation_hex,
							groupPublicKeyHex: groupPublicKey,
							signerOrigin: behindPage,
							// The challenge this publish's own set was issued, not a
							// fresh member-mode one: GROUP mode binds the whole set.
							challengeHex: derived.challenge
						});
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

	// `done` up front means the identical group was already on-chain —
	// idempotent by content hash, and just as landed as a fresh one.
	if (accepted.status === 'done') return;
	if (!accepted.id) throw new Error('register was accepted without a task id');
	await Registry.awaitTask(accepted.id);
}
