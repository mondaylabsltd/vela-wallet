/**
 * The Safe Genesis Registry service (`safe-genesis-registry`, genesis-cf) in
 * use: where a multisig's genesis is recorded and the proposal mailbox owners
 * share. Every request, its retries, its outage back-off and what an answer
 * means are the core's machines' (`multisig_shell::Service`); the shell only
 * names the service the person set (Settings → Service Endpoints).
 */
import { DEFAULT_GENESIS_SERVICE_URL, genesisService } from './service-state.svelte';

export { DEFAULT_GENESIS_SERVICE_URL };

/** The service in use: the person's own (Settings → Service Endpoints), else the default. */
export function genesisServiceURL(): string {
	return genesisService.url;
}
