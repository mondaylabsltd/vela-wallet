/**
 * The link that asks another owner to approve: `…/wallet#ms=<chainId>.<multisig>.<safeTxHash>`.
 *
 * It names the transaction, nothing more. Whoever opens it reads the
 * transaction from the mailbox, re-hashes it and checks it is the one named —
 * and adds the multisig by its address first if this device does not have it.
 * Kept after `#`, so it never reaches a server; any chat app or a QR code can
 * carry it.
 */
export interface ProposalLink {
	chainId: number;
	multisig: string;
	safeTxHash: string;
}

const PREFIX = 'ms=';

export function proposalLink(origin: string, locale: string, link: ProposalLink): string {
	return `${origin}/${locale}/wallet#${PREFIX}${link.chainId}.${link.multisig}.${link.safeTxHash}`;
}

/** The link in a location hash, or `null`. */
export function parseProposalLink(hash: string): ProposalLink | null {
	const body = hash.startsWith('#') ? hash.slice(1) : hash;
	if (!body.startsWith(PREFIX)) return null;
	const [chain, multisig, safeTxHash] = body.slice(PREFIX.length).split('.');
	if (!/^[1-9]\d{0,15}$/.test(chain ?? '')) return null;
	const chainId = Number(chain);
	if (!Number.isSafeInteger(chainId)) return null;
	if (!/^0x[0-9a-fA-F]{40}$/.test(multisig ?? '')) return null;
	if (!/^0x[0-9a-fA-F]{64}$/.test(safeTxHash ?? '')) return null;
	return { chainId, multisig, safeTxHash };
}

const ADD_PREFIX = 'msa=';

/** The link that adds a multisig by its address: `…/wallet#msa=<multisig>`. */
export function addLink(origin: string, locale: string, multisig: string): string {
	return `${origin}/${locale}/wallet#${ADD_PREFIX}${multisig}`;
}

/** The multisig an add link names, or `null`. */
export function parseAddLink(hash: string): string | null {
	const body = hash.startsWith('#') ? hash.slice(1) : hash;
	if (!body.startsWith(ADD_PREFIX)) return null;
	const address = body.slice(ADD_PREFIX.length);
	return /^0x[0-9a-fA-F]{40}$/.test(address) ? address : null;
}
