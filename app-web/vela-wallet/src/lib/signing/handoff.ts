/**
 * Spec 102 (D4) — the hand-off card's boards (HO1–HO3), the design the apps'
 * signing sheets build to when an account reviews and signs on a trusted page.
 *
 * The web never draws the card (it opens no page, P2-11), so these are boards
 * only. They borrow CS1's header — the site and the network stay on screen,
 * the minimal context D4 keeps — and replace the body: no preview, because
 * the page is the authority.
 */
import { IDENTITY } from '$lib/wallet/fixtures';
import { fill } from '$lib/wallet/messages';
import { integrityLineModel } from '$lib/settings/venue';
import type { HandoffMessages } from './messages';
import type { SigningModel } from './model';

export const HANDOFF_STATES = ['ho1', 'ho2', 'ho3'] as const;
export type HandoffStateId = (typeof HANDOFF_STATES)[number];

/**
 * - HO1: the official page matches Vela's published build list — Open.
 * - HO2: the bytes served are not a published version — Open is shut, and the
 *   line says why (the core's `Mismatch`: "…Not opened.").
 * - HO3: opened; Vela waits for the page's answer, with the way back to it.
 */
export function buildHandoffState(
	state: HandoffStateId,
	base: SigningModel,
	m: HandoffMessages
): SigningModel {
	const refused = state === 'ho2';
	const integrity = integrityLineModel(
		{
			key: `componentsUi.signing.integrity.${refused ? 'mismatch' : 'matches'}`,
			version: refused ? '7d41e0b9' : '0ba8ee8c'
		},
		'14:32',
		m
	);
	if (integrity === undefined) throw new Error('no words for the integrity line');
	return {
		...base,
		blocks: [],
		handoff: {
			title: m.title,
			key: fill(m.key, { key: IDENTITY.name }),
			page: { name: m.official, host: 'sign.getvela.app' },
			integrity,
			open: { label: m.open, enabled: !refused },
			waiting:
				state === 'ho3' ? { title: m.waiting, hint: m.waitingHint, reopen: m.reopen } : undefined
		}
	};
}
