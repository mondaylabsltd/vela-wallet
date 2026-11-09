/**
 * R22: where a network's RPC settings open — the network's own editor in
 * Settings, with "Only use my node" right under its RPC field
 * (`?network=<chainId>`, read by the settings route). The signing sheet's
 * hint under a simulation nobody independent confirmed links here, opened
 * beside the sheet so the request on it is not lost.
 */
import { resolve } from '$app/paths';

export function ownNodeSettingsHref(locale: string, chainId: number): string {
	return `${resolve('/[locale]/settings', { locale })}?network=${chainId}`;
}
