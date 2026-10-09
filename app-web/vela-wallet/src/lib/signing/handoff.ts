/**
 * Spec 102 (D4) — the hand-off card's boards (HO1–HO4), the design the apps'
 * signing sheets build to when an account reviews and signs on a trusted page.
 *
 * The web never draws the card (it opens no page, P2-11), so these are boards
 * only. They borrow CS1's sheet — the site and the network stay on screen, the
 * minimal context D4 keeps, and so do its fee row and signing account (D-18:
 * the fee and speed are chosen in the app before the page opens, so the card
 * does not restate them) — and replace the preview and the confirm with the
 * card: no preview, because the page is the authority.
 *
 * Every fact on the card is the core's (P2b-W5): the key row is the account's
 * `SigningPlan.key_label` (D-17) — its label from `label_key`, its value the
 * key's own name or its place — and the time on the integrity line is
 * `checked_time` (D-13) — so a board cannot show a card the core would not.
 */
import { signingPlan } from '$lib/core/kernels';
import type { KeyLabel } from '$lib/core/generated/KeyLabel';
import { boardCheckTime } from '$lib/settings/board-check';
import { OFFICIAL_PAGE } from '$lib/settings/fixtures';
import { IDENTITY } from '$lib/wallet/fixtures';
import { integrityLineModel } from '$lib/settings/venue';
import type { HandoffMessages } from './messages';
import type { HandoffModel, SigningModel } from './model';

export const HANDOFF_STATES = ['ho1', 'ho2', 'ho3', 'ho4'] as const;
export type HandoffStateId = (typeof HANDOFF_STATES)[number];

/**
 * The drawn account as stored: on `getvela.app`, reviewing on the official
 * page, signed in with `key` — a USB key the person named (HO1, HO3, HO4),
 * or a phone whose key still carries the wallet's name (HO2).
 */
function account(key: { name: string; method: 'security_key' | 'hybrid'; transports: string }) {
	const publicKey = '04' + '11'.repeat(64);
	return {
		id: 'ho01',
		name: IDENTITY.name,
		address: IDENTITY.addressFull,
		public_key_hex: publicKey,
		created_at_iso: '2026-10-09T00:00:00.000Z',
		keys: [
			{
				credential_id: 'ho01',
				public_key_hex: publicKey,
				name: key.name,
				transports: key.transports
			}
		],
		sign_in_key: { credential_id: 'ho01', method: key.method, transports: key.transports },
		signing_domain: 'getvela.app',
		signing_venue: { type: 'page', url: OFFICIAL_PAGE }
	};
}

const NAMED_KEY = account({ name: 'YubiKey 5C', method: 'security_key', transports: 'usb,nfc' });
const WALLET_NAMED_KEY = account({ name: IDENTITY.name, method: 'hybrid', transports: 'hybrid' });

/** The key row: `label_key`'s words | the key's own name, else its place (D-17). */
function keyRow(label: KeyLabel, m: HandoffMessages): HandoffModel['key'] {
	return {
		label: m.keyLabels[label.label_key] ?? '',
		value: label.name ?? m.places[label.place_key] ?? ''
	};
}

/** Each board's check: the core's `IntegrityLine` key and the version it names. */
const CHECKS: Record<HandoffStateId, { state: string; version: string }> = {
	ho1: { state: 'matches', version: '0ba8ee8c' },
	ho2: { state: 'mismatch', version: '7d41e0b9' },
	ho3: { state: 'matches', version: '0ba8ee8c' },
	ho4: { state: 'checking', version: '' }
};

/**
 * - HO1: the official page matches Vela's published build list — Open. The
 *   key is the one the person named, so the row names it.
 * - HO2: the bytes served are not a published version — Open is shut, and the
 *   line says why (the core's `Mismatch`: "…Not opened."). The key carries
 *   the wallet's name, so the row names its place instead.
 * - HO3: opened; Vela waits for the page's answer, with the way back to it.
 * - HO4: HO1 while its check runs (a check older than a day, D-4): Open is
 *   shut until it lands, and the line holds the room the verdict will take,
 *   so Open is where HO1 draws it.
 */
export function buildHandoffState(
	state: HandoffStateId,
	base: SigningModel,
	m: HandoffMessages,
	language = 'en'
): SigningModel {
	const refused = state === 'ho2';
	const plan = signingPlan(refused ? WALLET_NAMED_KEY : NAMED_KEY);
	if (plan === null) throw new Error('the core could not read the drawn account');
	const check = CHECKS[state];
	const integrity = integrityLineModel(
		{ key: `componentsUi.signing.integrity.${check.state}`, version: check.version },
		boardCheckTime(language),
		m
	);
	if (integrity === undefined) throw new Error('no words for the integrity line');
	return {
		...base,
		blocks: [],
		handoff: {
			title: m.title,
			key: keyRow(plan.key_label, m),
			page: { name: m.official, host: 'sign.getvela.app' },
			integrity,
			open: { label: m.open, enabled: check.state === 'matches' },
			waiting:
				state === 'ho3' ? { title: m.waiting, hint: m.waitingHint, reopen: m.reopen } : undefined
		}
	};
}
