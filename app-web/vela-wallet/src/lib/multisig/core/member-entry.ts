/**
 * A member typed as a name (`alice.eth`, `bob.base.eth`): resolved FORWARD
 * to the address it points to now, by the core's walk (`resolvedNameStep`,
 * the same one that checks a reverse record), over this wallet's own RPCs.
 * The address is always shown beside the name before it is used: the name
 * is how the person found the member, the address is the member.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { runWalk } from '$lib/services/core-walk';
import { networkForChainId } from '$lib/services/networks';

/** Where a name is looked up: Basenames on Base, everything else ENS on Ethereum. */
const SERVICES = [
	{ suffix: '.base.eth', chainId: 8453, registry: '0xb94704422c2a1e396835a571837aa5ae53285a95' },
	{ suffix: '', chainId: 1, registry: '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e' }
];

export type NameAnswer =
	| { state: 'resolved'; address: string; name: string }
	/** The name has no address (or is no name anyone resolves). */
	| { state: 'not_found' }
	/** Nobody answered, or this wallet has no network to ask on. */
	| { state: 'unavailable' };

/** Whether `typed` reads as a name to resolve rather than an address. */
export function looksLikeName(typed: string): boolean {
	const value = typed.trim();
	return !value.startsWith('0x') && /^[^\s.]+(\.[^\s.]+)+$/u.test(value);
}

interface ResolveDone {
	type: 'done';
	state: 'resolved' | 'not_found' | 'unavailable';
	address: string | null;
	name: string | null;
}

export async function resolveTypedName(typed: string): Promise<NameAnswer> {
	const name = typed.trim();
	const service = SERVICES.find((s) => name.toLowerCase().endsWith(s.suffix));
	if (!service || networkForChainId(service.chainId) === null) return { state: 'unavailable' };
	const step = (wasm as unknown as Record<string, unknown>).resolvedNameStep as
		((chainId: number, registry: string, name: string, answers: string) => string) | undefined;
	if (typeof step !== 'function') return { state: 'unavailable' };
	try {
		const done = await runWalk<ResolveDone>((answers) =>
			step(service.chainId, service.registry, name, answers)
		);
		if (!done) return { state: 'unavailable' };
		if (done.state === 'resolved' && done.address && done.name) {
			return { state: 'resolved', address: done.address, name: done.name };
		}
		return { state: done.state === 'resolved' ? 'not_found' : done.state };
	} catch {
		return { state: 'unavailable' };
	}
}
