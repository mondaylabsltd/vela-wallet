/**
 * Whose settings these are (spec 023) — the same swap `wallet/identity.ts`
 * makes, applied to the two places the settings screens name an account.
 *
 * Everything else on these screens is still the fixture layer: the networks,
 * the latencies, the storage accounting and the endpoint URLs. The name, the
 * address and the identicon are not allowed to be, for the reason spec 019
 * recorded — a fixture name over a real address tells somebody they are signed
 * in as a stranger.
 */
import type { SettingsDesktopModel, SettingsHomeModel } from './model';
import { shortenAddress, type WalletIdentity } from '$lib/wallet/identity';
import { IDENTICON_PLACEHOLDER_SVG } from '$lib/wallet/identicon-placeholder';

/** What the prerendered page carries before the session has answered. */
export const EMPTY_ACCOUNT = {
	name: '',
	addressDisplay: '',
	addressFull: '',
	identiconSvg: IDENTICON_PLACEHOLDER_SVG
};

/**
 * The active row of the account switcher, wearing the real identity.
 *
 * Only the ACTIVE row: the other two are fixtures, and there is no honest way
 * to make them real without an account list the core does not expose yet.
 * Marking just the active one is the smallest true statement available.
 */
function switcherRows<
	T extends {
		name: string;
		addressDisplay: string;
		addressFull: string;
		identiconSvg: string;
		selected: boolean;
	}
>(rows: T[], identity: WalletIdentity): T[] {
	return rows.map((row) =>
		row.selected
			? {
					...row,
					name: identity.name,
					addressDisplay: shortenAddress(identity.address),
					addressFull: identity.address,
					identiconSvg: identity.identiconSvg
				}
			: row
	);
}

export function homeWithIdentity(
	model: SettingsHomeModel,
	identity: WalletIdentity
): SettingsHomeModel {
	return {
		...model,
		account: {
			...model.account,
			name: identity.name,
			addressDisplay: shortenAddress(identity.address),
			addressFull: identity.address,
			identiconSvg: identity.identiconSvg
		},
		accountsSheet: {
			...model.accountsSheet,
			rows: switcherRows(model.accountsSheet.rows, identity)
		}
	};
}

export function desktopWithIdentity(
	model: SettingsDesktopModel,
	identity: WalletIdentity
): SettingsDesktopModel {
	return {
		...model,
		account: { ...model.account, rows: switcherRows(model.account.rows, identity) }
	};
}
