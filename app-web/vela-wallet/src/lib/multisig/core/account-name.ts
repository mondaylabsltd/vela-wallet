/**
 * r9-12 / r8-25 / r4D-5: ONE name per address in every place it is named —
 * the header, the switcher, Activity, Settings, the sheets, the invite. The
 * core's rule (`multisig_rules::names::label`, served as the accounts
 * machine's `member_labels`, this device's own accounts included): the name
 * the account published in the passkey index, else what a multisig here
 * keeps, else this device's local name, else the short address. A shell
 * never picks: `local` is only the fallback until the machine has named it.
 */
import { multisig } from './store.svelte';

export function accountName(address: string, local: string): string {
	return multisig.view?.member_labels?.[address.toLowerCase()] || local;
}
