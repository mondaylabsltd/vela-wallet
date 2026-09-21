/**
 * Full-screen states (spec 015 US1/US2 + spec 018 US1/US2): one prerendered
 * page per mock — wallet H1–H8 / D1–D3 and contacts C1–C6 / DC1–DC6 —
 * fixture-driven and offline.
 */
import { error } from '@sveltejs/kit';
import {
	resolveContactsMessages,
	resolveWalletFlowMessages,
	resolveExploreMessages,
	resolveSigningMessages,
	resolveSettingsMessages,
	resolveWalletMessages
} from '$lib/i18n/engine.server';
import { toLocale } from '$lib/i18n/locales';
import {
	buildDesktopState,
	buildMobileState,
	DESKTOP_STATES,
	MOBILE_STATES
} from '$lib/wallet/fixtures';
import {
	buildDesktopState as buildContactsDesktopState,
	buildMobileState as buildContactsMobileState,
	DESKTOP_STATES as CONTACTS_DESKTOP_STATES,
	MOBILE_STATES as CONTACTS_MOBILE_STATES
} from '$lib/contacts/fixtures';
import {
	buildDesktopFlowState,
	buildDesktopScan,
	buildFlowState,
	buildMobileScan,
	DESKTOP_FLOW_STATES,
	MOBILE_FLOW_STATES
} from '$lib/flows/fixtures';
import { identiconSvgFor } from '$lib/wallet/identicon.server';
import type { DesktopStateId, MobileStateId } from '$lib/wallet/model';
import type { DesktopContactsStateId, MobileContactsStateId } from '$lib/contacts/model';
import type { DesktopFlowStateId, FlowStateId } from '$lib/flows/model';
import {
	buildDesktopState as buildExploreDesktopState,
	buildMobileState as buildExploreMobileState,
	DESKTOP_STATES as EXPLORE_DESKTOP_STATES,
	exploreSidebar,
	MOBILE_STATES as EXPLORE_MOBILE_STATES
} from '$lib/explore/fixtures';
import { ALL_STATES as SIGNING_STATES, buildSigningState } from '$lib/signing/fixtures';
import type { ExploreDesktopStateId, ExploreStateId } from '$lib/explore/model';
import type { SigningStateId } from '$lib/signing/model';
import {
	buildDesktopState as buildSettingsDesktopState,
	buildMobileState as buildSettingsMobileState,
	DESKTOP_STATES as SETTINGS_DESKTOP_STATES,
	MOBILE_STATES as SETTINGS_MOBILE_STATES
} from '$lib/settings/fixtures';
import type { DesktopSettingsStateId, MobileSettingsStateId } from '$lib/settings/model';
import type { WalletFlowMessages } from '$lib/flows/messages';
import type { ScanNoticeMessages } from '$lib/flows/core/scanner.svelte';
import type { EntryGenerator, PageServerLoad } from './$types';

/**
 * The scanner behind the start page's scan button (issue 273): S1's model and
 * only the sentences it can say, not the whole flow corpus.
 */
function exploreScan(m: WalletFlowMessages) {
	return {
		model: buildMobileScan(m),
		messages: {
			'home.invalidQrTitle': m['home.invalidQrTitle'],
			'componentsUi.scanner.noQrFoundMsg': m['componentsUi.scanner.noQrFoundMsg'],
			'componentsUi.scanner.permissionText': m['componentsUi.scanner.permissionText'],
			'componentsUi.scanner.noCamera': m['componentsUi.scanner.noCamera'],
			'componentsUi.scanner.insecureOrigin': m['componentsUi.scanner.insecureOrigin'],
			'componentsUi.scanner.cameraUnavailable': m['componentsUi.scanner.cameraUnavailable']
		} satisfies ScanNoticeMessages
	};
}

export const entries: EntryGenerator = () =>
	(['zh', 'en'] as const).flatMap((locale) =>
		[
			...MOBILE_STATES,
			...DESKTOP_STATES,
			...CONTACTS_MOBILE_STATES,
			...CONTACTS_DESKTOP_STATES,
			...MOBILE_FLOW_STATES,
			...DESKTOP_FLOW_STATES,
			...EXPLORE_MOBILE_STATES,
			...EXPLORE_DESKTOP_STATES,
			...SIGNING_STATES,
			...SETTINGS_MOBILE_STATES,
			...SETTINGS_DESKTOP_STATES
		].map((state) => ({ locale, state }))
	);

export const load: PageServerLoad = ({ params }) => {
	const locale = toLocale(params.locale ?? '');
	if (locale === undefined) error(404, `unsupported locale "${params.locale}"`);

	if ((MOBILE_STATES as string[]).includes(params.state)) {
		const messages = resolveWalletMessages(locale);
		const state = params.state as MobileStateId;
		return { kind: 'mobile' as const, model: buildMobileState(state, messages, identiconSvgFor) };
	}
	if ((DESKTOP_STATES as string[]).includes(params.state)) {
		const messages = resolveWalletMessages(locale);
		const state = params.state as DesktopStateId;
		return { kind: 'desktop' as const, model: buildDesktopState(state, messages, identiconSvgFor) };
	}
	if ((CONTACTS_MOBILE_STATES as string[]).includes(params.state)) {
		const messages = resolveContactsMessages(locale);
		const state = params.state as MobileContactsStateId;
		return {
			kind: 'contacts-mobile' as const,
			model: buildContactsMobileState(state, messages, identiconSvgFor)
		};
	}
	if ((CONTACTS_DESKTOP_STATES as string[]).includes(params.state)) {
		const messages = resolveContactsMessages(locale);
		const state = params.state as DesktopContactsStateId;
		return {
			kind: 'contacts-desktop' as const,
			model: buildContactsDesktopState(state, messages, identiconSvgFor)
		};
	}
	if ((SETTINGS_MOBILE_STATES as string[]).includes(params.state)) {
		const messages = resolveSettingsMessages(locale);
		const state = params.state as MobileSettingsStateId;
		return {
			kind: 'settings-mobile' as const,
			model: buildSettingsMobileState(state, messages, identiconSvgFor)
		};
	}
	if ((SETTINGS_DESKTOP_STATES as string[]).includes(params.state)) {
		const messages = resolveSettingsMessages(locale);
		const state = params.state as DesktopSettingsStateId;
		// The app sidebar is spec 015's, with 设置 selected — the settings page
		// is a destination inside the wallet shell, not a shell of its own.
		const wallet = buildDesktopState('d1', resolveWalletMessages(locale), identiconSvgFor);
		return {
			kind: 'settings-desktop' as const,
			model: buildSettingsDesktopState(state, messages, identiconSvgFor),
			sidebar: {
				...wallet.sidebar,
				nav: wallet.sidebar.nav.map((item) => ({ ...item, selected: item.id === 'settings' }))
			}
		};
	}
	if ((MOBILE_FLOW_STATES as string[]).includes(params.state)) {
		const messages = resolveWalletFlowMessages(locale);
		const state = params.state as FlowStateId;
		return {
			kind: 'flow-mobile' as const,
			model: buildFlowState(state, messages, identiconSvgFor)
		};
	}
	if ((DESKTOP_FLOW_STATES as string[]).includes(params.state)) {
		const messages = resolveWalletFlowMessages(locale);
		const state = params.state as DesktopFlowStateId;
		return {
			kind: 'flow-desktop' as const,
			model: buildDesktopFlowState(state, messages, identiconSvgFor),
			// `ds1` is a modal over the window, not a panel — the page needs
			// the scanner model to draw it that way.
			scan: buildDesktopScan(messages),
			wallet: buildDesktopState('d1', resolveWalletMessages(locale), identiconSvgFor)
		};
	}
	// spec 022. The explore states carry a signing model too: E4's page can
	// raise the sheet, and DE4 IS the third column holding one — a browser
	// whose signing request is somewhere else is not the thing being reviewed.
	if ((EXPLORE_MOBILE_STATES as readonly string[]).includes(params.state)) {
		const state = params.state as ExploreStateId;
		return {
			kind: 'explore-mobile' as const,
			model: buildExploreMobileState(state, resolveExploreMessages(locale), identiconSvgFor),
			copy: resolveExploreMessages(locale),
			signing: buildSigningState('cs12', resolveSigningMessages(locale), identiconSvgFor),
			scan: exploreScan(resolveWalletFlowMessages(locale))
		};
	}
	if ((EXPLORE_DESKTOP_STATES as readonly string[]).includes(params.state)) {
		const state = params.state as ExploreDesktopStateId;
		const walletMessages = resolveWalletMessages(locale);
		return {
			kind: 'explore-desktop' as const,
			model: buildExploreDesktopState(state, resolveExploreMessages(locale), identiconSvgFor),
			copy: resolveExploreMessages(locale),
			sidebar: exploreSidebar(buildDesktopState('d1', walletMessages, identiconSvgFor).sidebar),
			signing: buildSigningState('cs12', resolveSigningMessages(locale), identiconSvgFor)
		};
	}
	if ((SIGNING_STATES as readonly string[]).includes(params.state)) {
		const state = params.state as SigningStateId;
		return {
			kind: 'signing' as const,
			model: buildExploreMobileState('e4', resolveExploreMessages(locale), identiconSvgFor),
			copy: resolveExploreMessages(locale),
			signing: buildSigningState(state, resolveSigningMessages(locale), identiconSvgFor)
		};
	}
	error(404, `unknown state "${params.state}"`);
};
