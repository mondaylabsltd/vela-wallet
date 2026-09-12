/**
 * The sidebar's network filter — the phone app's `selectedChainId`
 * (`useHomeController.ts:241`): one chain, or `null` for every network.
 *
 * Shell render state, not a preference: the phone app keeps it in component
 * state and forgets it on relaunch, and so does this. It lives in a module
 * rather than on the wallet page because the desktop draws the same sidebar
 * on three routes — a network chosen on 设置 has to be the one the wallet then
 * shows, and a person coming back from 通讯录 expects the list they left.
 */
class ChainFilter {
	chainId = $state<number | null>(null);

	select(chainId: number | null): void {
		this.chainId = chainId;
	}
}

export const chainFilter = new ChainFilter();
