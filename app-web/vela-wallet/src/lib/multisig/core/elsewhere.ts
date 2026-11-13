/**
 * r13-1: where the shown multisig lives when none of its networks is in the
 * space in front — the core's `network_space::lives_elsewhere`, the rule the
 * native homes drew on their own (r10-4, r11-3). The web home over such an
 * account says where it lives with the switch, puts the other space's
 * heading where the figure goes, and claims nothing of the space in front:
 * no "Live · listening for payments", no "No activity yet", no "Deposit
 * your first asset".
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { getAllNetworksSync, isHiddenNetwork } from '$lib/services/networks';

/** The core's `Elsewhere`. */
export interface Elsewhere {
	/** Its networks of the other space, in its order. */
	chain_ids: number[];
	/** The space it lives in (the switch's target). */
	space: 'mainnet' | 'testnet';
	/** `home.multisig.add.usedOn` ("Used on {{networks}}"). */
	line_key: string;
	/** `networkSpace.headingTestnet` / `.headingMainnet`. */
	heading_key: string;
	/** `networkSpace.toTestnet` / `.toMainnet`. */
	switch_key: string;
}

/**
 * The core's word for an account used on `chainIds` with `front` in front:
 * the person's lists (a network they took off is in neither space), split by
 * space. `null`: nothing to say — or the core is not loaded yet.
 */
export function livesElsewhere(
	chainIds: readonly number[],
	front: 'mainnet' | 'testnet'
): Elsewhere | null {
	if (chainIds.length === 0) return null;
	const listed = getAllNetworksSync().filter((n) => !isHiddenNetwork(n.chainId));
	const testnet = front === 'testnet';
	const here = listed.filter((n) => n.isTestnet === testnet).map((n) => n.chainId);
	const other = listed.filter((n) => n.isTestnet !== testnet).map((n) => n.chainId);
	try {
		return JSON.parse(
			wasm.networkLivesElsewhere(JSON.stringify({ chain_ids: chainIds, here, other, front }))
		) as Elsewhere | null;
	} catch {
		return null;
	}
}
