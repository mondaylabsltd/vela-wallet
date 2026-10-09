/**
 * Spec 102 (D4) — the hand-off card's boards (HO1–HO3), the design the apps'
 * signing sheets build to when an account reviews and signs on a trusted page.
 *
 * The web never draws the card (it opens no page, P2-11), so these are boards
 * only. They borrow CS1's header — the site and the network stay on screen,
 * the minimal context D4 keeps — and replace the body: no preview, because
 * the page is the authority.
 *
 * Every fact on the card is the core's (P2b-W5): the key's name is the
 * account's `SigningPlan.key_label` (D-17), the time on the integrity line is
 * `checked_time` (D-13), and the fee row is `handoff_fee` over the same fee
 * and speed views a sheet drives (D-18) — so a board cannot show a card the
 * core would not.
 */
import { handoffFeeRow, signingPlan } from '$lib/core/kernels';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { KeyLabel } from '$lib/core/generated/KeyLabel';
import { feeParts } from '$lib/flows/fee-line';
import { boardCheckTime } from '$lib/settings/board-check';
import { OFFICIAL_PAGE } from '$lib/settings/fixtures';
import { IDENTITY } from '$lib/wallet/fixtures';
import { fill } from '$lib/wallet/messages';
import { integrityLineModel } from '$lib/settings/venue';
import type { HandoffMessages } from './messages';
import type { SigningModel } from './model';

export const HANDOFF_STATES = ['ho1', 'ho2', 'ho3'] as const;
export type HandoffStateId = (typeof HANDOFF_STATES)[number];

/**
 * The drawn account as stored: on `getvela.app`, reviewing on the official
 * page, signed in with `key` — a USB key the person named (HO1, HO3), or a
 * phone whose key still carries the wallet's name (HO2).
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

/** The fee the sheet settled before Open: 0.00042 ETH at the standard speed, on Ethereum. */
export const HANDOFF_FEE: FeeView = {
	busy: false,
	failed: null,
	fee: {
		chain_id: 1,
		total_wei: '420000000000000',
		max_fee_per_gas: '2000000000',
		network_fee_per_gas: '2000000000',
		relayer_fee_per_gas: '0',
		bundler_gas_price: '2000000000',
		in_band_gas_basis: '2000000000',
		effective_gas_price: null,
		max_gas_price: null,
		total_gas: '210000',
		deployed: true,
		tier: 'standard',
		quoted: true,
		fee_asset: { type: 'native' },
		fee_recipient: null
	},
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: true,
	no_coin_pays: false
};

/** The speed control the sheet drove: folded, at the person's default. */
export const HANDOFF_SPEED: FeeSpeedView = {
	tier: 'standard',
	preferred: 'standard',
	previews: [],
	open: false,
	picked: false,
	free: false,
	free_note: false,
	single: false,
	gas_price_line: false,
	options: []
};

/** "Confirm with {key}": the key's own label, else its place, in words. */
function keyText(label: KeyLabel, m: HandoffMessages): string {
	return label.name ?? m.places[label.place_key] ?? '';
}

/**
 * - HO1: the official page matches Vela's published build list — Open. The
 *   key is the one the person named, so the card names it.
 * - HO2: the bytes served are not a published version — Open is shut, and the
 *   line says why (the core's `Mismatch`: "…Not opened."). The key carries
 *   the wallet's name, so the card names its place instead.
 * - HO3: opened; Vela waits for the page's answer, with the way back to it.
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
	const integrity = integrityLineModel(
		{
			key: `componentsUi.signing.integrity.${refused ? 'mismatch' : 'matches'}`,
			version: refused ? '7d41e0b9' : '0ba8ee8c'
		},
		boardCheckTime(language),
		m
	);
	if (integrity === undefined) throw new Error('no words for the integrity line');
	const row = handoffFeeRow(HANDOFF_FEE, HANDOFF_SPEED);
	const speed = row?.tier_key == null ? undefined : m.tiers[row.tier_key];
	return {
		...base,
		blocks: [],
		handoff: {
			title: m.title,
			key: fill(m.key, { key: keyText(plan.key_label, m) }),
			fee:
				row === null
					? undefined
					: {
							label: m.feeLabel,
							value: [feeParts(row.fee, HANDOFF_FEE.options).coin, speed]
								.filter((part) => part !== undefined)
								.join(' · ')
						},
			page: { name: m.official, host: 'sign.getvela.app' },
			integrity,
			open: { label: m.open, enabled: !refused },
			waiting:
				state === 'ho3' ? { title: m.waiting, hint: m.waitingHint, reopen: m.reopen } : undefined
		}
	};
}
