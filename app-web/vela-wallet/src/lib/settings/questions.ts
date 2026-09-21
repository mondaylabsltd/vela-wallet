/**
 * The questions a destructive row asks before it acts (spec 058; spec 072 for
 * the wide layout and for networks).
 *
 * Each is built from what the row already says — its name, its group's
 * warning, its own action word — so no sentence is invented for it, and the
 * phone's sheet and the desktop's dialog ask the same thing in the same
 * words. The title names what goes: an untitled "Disconnect?" never said
 * which site.
 */
import type { ConfirmSheetModel, NetworkRowModel, StorageModel } from './model';

/**
 * A storage row's Clear. "Contacts and groups · Clear" removed the whole
 * address book on one click, with nothing in between.
 */
export function storageClearQuestion(
	storage: StorageModel,
	id: string,
	cancel: string
): ConfirmSheetModel | undefined {
	const group = storage.groups.find((g) => g.items.some((item) => item.id === id));
	const item = group?.items.find((entry) => entry.id === id);
	if (group === undefined || item === undefined) return undefined;
	return {
		title: item.label,
		body: group.label,
		confirm: item.action,
		cancel,
		tone: item.destructive === true ? 'danger' : 'accent'
	};
}

/** A custom network's delete control, titled with the network's name. */
export function removeNetworkQuestion(
	networks: { rows: NetworkRowModel[]; removeSheet: ConfirmSheetModel },
	id: string
): ConfirmSheetModel | undefined {
	const row = networks.rows.find((entry) => entry.id === id);
	return row === undefined ? undefined : { ...networks.removeSheet, title: row.name };
}
