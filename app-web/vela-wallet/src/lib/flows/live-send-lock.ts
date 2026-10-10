/**
 * A payment request the wallet cannot take up as it is — the send core's
 * `lock_error` (PR 3 final notes F6 and F27).
 *
 * A scanned code or a pay link can name a network this wallet does not have,
 * or a token it cannot describe. The core stops the send there
 * (`SendStage::LockError`) and offers one way on for a network: add it
 * (`add_network_tapped` → the `add_network` operation → `add_network_msg`
 * when it could not be done). The web drew none of it: the stage fell through
 * to the asset picker, with no word about the request that had been scanned,
 * and `add_network_tapped` had no caller.
 *
 * Which stop it is, whether an add is running and what came of it are the
 * core's; this only words them, from the corpus. Pure.
 */
import type { SendView } from '$lib/core/generated/SendView';
import { fill } from '$lib/wallet/messages';
import type { WalletFlowMessages } from './messages';

export interface SendLockModel {
	title: string;
	body: string;
	/**
	 * "Add this network" — a network stop only (a token nobody can describe
	 * has nothing to add). `busy` while the core's add is running: the button
	 * says so and takes no second press.
	 */
	add?: { label: string; chainId: number; busy: boolean };
	/** Why the add did not happen, under the button. */
	failed?: string;
}

type LockFacts = Pick<SendView, 'lock_error' | 'adding_network' | 'add_network_msg'>;

/** The stop as drawn, or `undefined` when the send is not stopped on one. */
export function liveSendLock(view: LockFacts, m: WalletFlowMessages): SendLockModel | undefined {
	const lock = view.lock_error;
	if (!lock) return undefined;
	if (lock.type === 'token') {
		return { title: m['send.lock.tokenTitle'], body: m['send.lock.tokenBody'] };
	}
	const said = view.add_network_msg;
	const failed =
		said === null || view.adding_network
			? undefined
			: said.type === 'net_not_found'
				? m['send.lock.netNotFound']
				: said.type === 'net_not_compatible'
					? m['send.lock.netNotCompatible']
					: m['send.lock.netAddError'];
	return {
		title: m['send.lock.netTitle'],
		body: fill(m['send.lock.netBody'], { chainId: lock.chain_id }),
		add: { label: m['send.lock.addNetwork'], chainId: lock.chain_id, busy: view.adding_network },
		...(failed === undefined ? {} : { failed })
	};
}
