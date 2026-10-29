/**
 * An ENS name typed where a member's address goes (r4D-5): read on Ethereum
 * through this wallet's own RPC — the registry's resolver for the name, then
 * that resolver's `addr` — and shown as the address it gives, prominently,
 * for the person to add. The name is a label only: the member is the
 * address, checked and checksummed like a typed one, and nothing about who
 * may act ever rests on the name.
 *
 * Plain ASCII names only (`alice.eth`, `team.alice.eth`): a name with other
 * characters needs ENSIP-15 normalisation, which this does not do — it says
 * so rather than resolve a lookalike.
 */
import { checksumAddress } from '$lib/core/kernels';
import { namehash } from '$lib/services/ens';
import { poolRpcCall } from '$lib/services/rpc-pool';

/** The ENS registry, the same address on Ethereum since 2020. */
const ENS_REGISTRY = '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e';
const RESOLVER = '0x0178b8bf'; // resolver(bytes32)
const ADDR = '0x3b3b57de'; // addr(bytes32)
const ETHEREUM = 1;

/** Whether `text` is meant as an ENS name (ends in `.eth`), readable here or not. */
export function looksLikeEns(text: string): boolean {
	return /^[^\s/]+\.eth$/i.test(text.trim());
}

/** A name this resolves: letters, digits and hyphens, dot-separated, ending in `.eth`. */
function plainName(text: string): boolean {
	return /^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+eth$/.test(text);
}

export type EnsAnswer =
	| { kind: 'found'; name: string; address: string }
	/** No resolver, or no address set for it. */
	| { kind: 'none'; name: string }
	/** Ethereum did not answer: nothing is said of the name. */
	| { kind: 'unavailable'; name: string }
	/** Characters this does not normalise: the person enters the address. */
	| { kind: 'unsupported'; name: string };

function word(result: unknown): string | null {
	return typeof result === 'string' && /^0x[0-9a-fA-F]{64}$/.test(result)
		? `0x${result.slice(-40)}`
		: null;
}

const ZERO = `0x${'0'.repeat(40)}`;

export async function resolveEns(typed: string): Promise<EnsAnswer> {
	const name = typed.trim().toLowerCase();
	if (!plainName(name)) return { kind: 'unsupported', name };
	try {
		const node = namehash(name).slice(2);
		const resolver = word(
			(
				await poolRpcCall(
					'eth_call',
					[{ to: ENS_REGISTRY, data: `${RESOLVER}${node}` }, 'latest'],
					ETHEREUM
				)
			).result
		);
		if (resolver === null) return { kind: 'unavailable', name };
		if (resolver === ZERO) return { kind: 'none', name };
		const read = await poolRpcCall(
			'eth_call',
			[{ to: resolver, data: `${ADDR}${node}` }, 'latest'],
			ETHEREUM
		);
		// A resolver that reverts on `addr` holds no address for it.
		if (read.error)
			return /revert/i.test(String(read.error.message ?? ''))
				? { kind: 'none', name }
				: { kind: 'unavailable', name };
		const address = word(read.result);
		if (address === null) return { kind: 'unavailable', name };
		if (address === ZERO) return { kind: 'none', name };
		return { kind: 'found', name, address: checksumAddress(address) };
	} catch {
		return { kind: 'unavailable', name };
	}
}
