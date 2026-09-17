/**
 * The `window.vela` dev/e2e console — WEB (spec 025 T117; the fault-harness
 * doctrine: every failure-state UX must be reachable without breaking real
 * infrastructure).
 *
 * Installed only when `localStorage['vela.dev.console'] === '1'` (dev builds
 * set it themselves below; e2e seeds it via addInitScript; a production
 * visitor never has it). Everything here is diagnostics or SELF-harm on the
 * local session — no capability a page script could not already exercise
 * against its own storage.
 */

import { installFaultConsole } from './fault-injection';
import { getFailedRpcChains, getRateLimitedChains, poolRpcCall } from './rpc-pool';

const GATE_KEY = 'vela.dev.console';

export function maybeInstallDevConsole(): void {
	if (typeof window === 'undefined') return;
	try {
		if (!import.meta.env.DEV && window.localStorage.getItem(GATE_KEY) !== '1') return;
	} catch {
		return;
	}

	installFaultConsole();
	// The parallel space's own verbs, behind the same gate and one more dynamic
	// import: `vela.parallel.enter()` is how a test (or a developer) swaps in the
	// fixture wallet without visiting its page. The fixture keys ride along with
	// THIS import, never with a product chunk.
	void import('$lib/dev/parallel-space').then((m) => m.installParallelConsole());
	// The signing requester: the seam a real transport plugs into (027). Behind
	// the same gate, and it registers itself with the resident so the core can
	// answer the request it delivered — never a shared reference.
	void Promise.all([
		import('$lib/dev/test-requester'),
		import('$lib/signing/core/sign-resident.svelte')
	]).then(([requester, resident]) => {
		const transportId = resident.signRequest.registerTransport({
			sendResponse: (id, result, error) => requester.bindRequesterResponse(id, result, error)
		});
		requester.bindRequester((request) => {
			resident.signRequest.syncNetworks();
			resident.signRequest.dispatch({
				type: 'request_arrived',
				id: request.id,
				method: request.method,
				params_json: JSON.stringify(request.params),
				origin: request.origin,
				transport_id: transportId,
				dedicated_transport: true,
				per_request_chain: null,
				dapp: null,
				granted_address: null,
				requested_address: null,
				request_ts_ms: null,
				now_ms: Date.now()
			});
		});
		requester.installRequesterConsole();
	});
	const vela = (window as unknown as { vela?: Record<string, unknown> }).vela ?? {};
	Object.assign(vela, {
		/**
		 * The Ethereum backup (spec 062), before any screen draws it:
		 * `vela.backup.check()` → where the founding record stands;
		 * `vela.backup.start()` → the same, and the signing sheet when there is
		 * something to sign (run it on the wallet route, which hosts the sheet).
		 */
		backup: {
			check: async () => {
				const [{ session }, backup, service] = await Promise.all([
					import('$lib/session/core/session.svelte'),
					import('$lib/backup/ethereum-backup'),
					import('$lib/services/registry-backup')
				]);
				const account = session.view.accounts[session.view.active_index]?.account;
				if (!account) return null;
				return service.checkEthereumBackup(account.address, backup.foundingKeyOf(account));
			},
			start: async () => {
				const [{ session }, backup] = await Promise.all([
					import('$lib/session/core/session.svelte'),
					import('$lib/backup/ethereum-backup')
				]);
				const account = session.view.accounts[session.view.active_index]?.account;
				return account ? backup.startEthereumBackup(account) : null;
			}
		},
		/** Drive one pool-routed read — the harness's entry into the router. */
		poolCall: (method: string, params: unknown[], chainId: number) =>
			poolRpcCall(method, params, chainId),
		rpcState: () => ({
			failed: [...getFailedRpcChains()],
			rateLimited: [...getRateLimitedChains()]
		})
	});
	(window as unknown as { vela: Record<string, unknown> }).vela = vela;
}
