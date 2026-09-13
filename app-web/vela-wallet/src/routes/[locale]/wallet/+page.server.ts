import { error } from '@sveltejs/kit';
import {
	resolveSettingsMessages,
	resolveSigningMessages,
	resolveWalletFlowMessages,
	resolveWalletMessages
} from '$lib/i18n/engine.server';
import { pickRescueMessages } from '$lib/settings/live';
import { SUPPORTED_LOCALES, toLocale } from '$lib/i18n/locales';
import { buildDesktopState, buildMobileState } from '$lib/wallet/fixtures';
import { buildDesktopFlowState, buildDesktopScan, buildFlowState } from '$lib/flows/fixtures';
import { DESKTOP_FLOW_STATES, MOBILE_FLOW_STATES } from '$lib/flows/fixtures';
import { identiconSvgFor } from '$lib/wallet/identicon.server';
import { EMPTY_HEADER } from '$lib/wallet/identity';
import type { EntryGenerator, PageServerLoad } from './$types';

/**
 * The signed-in wallet's own route (spec 019).
 *
 * Prerendered per locale like every other page: the body is still the spec-015
 * fixture layer and its copy is static, so all of it resolves here. What is NOT
 * resolved here is WHOSE wallet it is — the header ships empty and the browser
 * fills it from the session, because a prerendered fixture address would flash
 * a stranger's wallet at the person for a frame after hydration.
 *
 * The guard lives in the page, not in a `load`: prerendered pages have no
 * server request to redirect, and the only thing that knows whether this
 * browser has a wallet is the session core running in it.
 */
export const entries: EntryGenerator = () => SUPPORTED_LOCALES.map((locale) => ({ locale }));

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);
	const messages = resolveWalletMessages(locale);
	const settingsMessages = resolveSettingsMessages(locale);

	const home = buildMobileState('h1', messages, identiconSvgFor);
	const desktop = buildDesktopState('d1', messages, identiconSvgFor);

	/**
	 * Spec 021: every wallet-flow state, prerendered alongside the home.
	 *
	 * All of it resolves here for the same reason the home does — the copy is
	 * static and the data is still fixtures — and all of it ships at once
	 * because the flows are pushed screens inside this one route, not routes
	 * of their own. The identity is the only thing the browser fills in.
	 */
	const flowMessages = resolveWalletFlowMessages(locale);
	const flows = Object.fromEntries(
		MOBILE_FLOW_STATES.map((id) => [id, buildFlowState(id, flowMessages, identiconSvgFor)])
	);
	const desktopFlows = Object.fromEntries(
		DESKTOP_FLOW_STATES.map((id) => [id, buildDesktopFlowState(id, flowMessages, identiconSvgFor)])
	);

	// No explore data here (spec 022 founder call): 探索 is the in-app dApp
	// browser, and this client IS a browser tab — it cannot host one. The
	// vocabulary still ships for the gallery, which is the design source the
	// three native clients are reviewed against.
	return {
		walletMessages: messages,
		home: { ...home, header: EMPTY_HEADER },
		desktop: { ...desktop, sidebar: { ...desktop.sidebar, header: EMPTY_HEADER } },
		flows,
		desktopFlows,
		// The send overlays word themselves from the same manifest the fixtures
		// were built with (spec 026), so a live screen and its drawn twin can
		// never disagree about a label.
		flowMessages,
		// The signing sheet's copy (spec 026): the same manifest the gallery
		// boards are built from, so a live sheet and its drawn twin cannot
		// disagree about a word.
		signingMessages: resolveSigningMessages(locale),
		desktopScan: buildDesktopScan(flowMessages),
		// The rescue sheets (spec 028 Phase 8) are settings components opened
		// over the wallet and the send; they speak the settings corpus, and only
		// the slice they need ships with this page.
		rescueMessages: pickRescueMessages(settingsMessages),
		// The account switcher behind the header's name (founder call,
		// 2026-09-05) is the settings sheet, so it speaks that corpus too.
		accountsMessages: settingsMessages.accounts
	};
};
