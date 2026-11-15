/**
 * Whose wallet the browser is connected as — swapped into the fixture model at
 * runtime (spec 022, the same swap spec 019 makes on the wallet screen).
 *
 * Two things must never be a fixture once somebody is signed in: the account a
 * site can see, and the address it is being shown. A connection panel that
 * named a stranger's account would be the wallet lying about what it just
 * granted, which is worse than showing nothing.
 */
import { shortenAddress, type WalletIdentity } from '#lib/wallet/identity.ts';
import type { ExploreDesktopModel, ExploreHomeModel } from './model';

/** The phone model, wearing the signed-in wallet's identity. */
export function exploreWithIdentity(
	model: ExploreHomeModel,
	identity: WalletIdentity
): ExploreHomeModel {
	return {
		...model,
		browser: {
			...model.browser,
			account: { name: identity.name, identiconSvg: identity.identiconSvg }
		},
		menus: {
			...model.menus,
			connection: {
				...model.menus.connection,
				connection: {
					...model.menus.connection.connection,
					account: {
						name: identity.name,
						address: shortenAddress(identity.address),
						identiconSvg: identity.identiconSvg
					}
				}
			}
		},
		sheet:
			model.sheet?.kind === 'connection'
				? {
						...model.sheet,
						connection: {
							...model.sheet.connection,
							account: {
								name: identity.name,
								address: shortenAddress(identity.address),
								identiconSvg: identity.identiconSvg
							}
						}
					}
				: model.sheet
	};
}

/** The desktop model — toolbar chip and third-column panel alike. */
export function exploreDesktopWithIdentity(
	model: ExploreDesktopModel,
	identity: WalletIdentity
): ExploreDesktopModel {
	return {
		...model,
		browser: {
			...model.browser,
			account: { name: identity.name, identiconSvg: identity.identiconSvg }
		},
		connection: {
			...model.connection,
			account: {
				name: identity.name,
				address: shortenAddress(identity.address),
				identiconSvg: identity.identiconSvg
			}
		}
	};
}

/** The signing sheet's signer row, for the same reason. */
export function signerWithIdentity<
	T extends { signer: { label: string; name: string; identiconSvg: string } }
>(model: T, identity: WalletIdentity): T {
	return {
		...model,
		signer: { ...model.signer, name: identity.name, identiconSvg: identity.identiconSvg }
	};
}
