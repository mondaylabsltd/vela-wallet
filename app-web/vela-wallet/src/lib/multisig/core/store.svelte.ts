/**
 * The multisig accounts this browser has added, and which one (if any) is the
 * account the wallet is showing.
 *
 * Kept under keys of their own, never in `vela.accounts`: an older build reads
 * that list with a parser that requires a passkey, and its migration re-derives
 * every address from the keys — a multisig row there would be dropped or
 * rewritten (docs/multisig-vault-design.md §3.1). The Vela accounts stay the
 * session core's; a multisig is an overlay on top of it: when one is active,
 * the wallet shows its address, and the session's own active account is
 * untouched underneath.
 *
 * What is stored is what does not change — the address and the genesis that
 * makes it. Owners and threshold are each chain's and are read from it; the
 * genesis's are kept as the multisig's founding members, for display before
 * a chain answers.
 */
import { genesisAddress, genesisMembers, sameAddress, type Genesis } from './kernel';

export const MULTISIG_ACCOUNTS_KEY = 'vela.multisigAccounts';
export const ACTIVE_MULTISIG_KEY = 'vela.activeMultisig';

export interface MultisigAccount {
	address: string;
	genesis: Genesis;
	/** The founding members (the genesis's), in their order. */
	owners: string[];
	threshold: number;
	/** Whether the registry is known to hold the genesis (else it is retried). */
	recorded: boolean;
	/** The networks it is used on: where its proposals are looked for. */
	chainIds: number[];
	addedMs: number;
}

function read(): MultisigAccount[] {
	try {
		const raw = localStorage.getItem(MULTISIG_ACCOUNTS_KEY);
		if (!raw) return [];
		const rows = JSON.parse(raw) as unknown;
		if (!Array.isArray(rows)) return [];
		return rows
			.filter(
				(r): r is MultisigAccount =>
					!!r &&
					typeof r === 'object' &&
					typeof (r as MultisigAccount).address === 'string' &&
					!!(r as MultisigAccount).genesis
			)
			.map((r) => ({ ...r, chainIds: Array.isArray(r.chainIds) ? r.chainIds : [] }));
	} catch {
		return [];
	}
}

function write(rows: MultisigAccount[]): void {
	try {
		localStorage.setItem(MULTISIG_ACCOUNTS_KEY, JSON.stringify(rows));
	} catch {
		// best effort: the in-memory list stands
	}
}

function readActive(): string | null {
	try {
		return localStorage.getItem(ACTIVE_MULTISIG_KEY) || null;
	} catch {
		return null;
	}
}

function writeActive(address: string | null): void {
	try {
		if (address) localStorage.setItem(ACTIVE_MULTISIG_KEY, address);
		else localStorage.removeItem(ACTIVE_MULTISIG_KEY);
	} catch {
		// best effort
	}
}

class MultisigStore {
	accounts = $state<MultisigAccount[]>([]);
	#active = $state<string | null>(null);
	#booted = false;

	/** The active multisig, or `null` when the wallet shows a Vela account. */
	get active(): MultisigAccount | null {
		const address = this.#active;
		return address ? (this.accounts.find((a) => sameAddress(a.address, address)) ?? null) : null;
	}

	boot(): void {
		if (this.#booted || typeof window === 'undefined') return;
		this.#booted = true;
		this.accounts = read();
		this.#active = readActive();
		// Another tab added, removed or switched: follow it.
		window.addEventListener('storage', (event) => {
			if (event.key === MULTISIG_ACCOUNTS_KEY) this.accounts = read();
			if (event.key === ACTIVE_MULTISIG_KEY) this.#active = readActive();
		});
	}

	find(address: string): MultisigAccount | undefined {
		return this.accounts.find((a) => sameAddress(a.address, address));
	}

	/**
	 * Add a multisig by its genesis. The genesis must derive the address —
	 * the core re-derives it — so a wrong one never becomes an account.
	 */
	add(address: string, genesis: Genesis, recorded: boolean, chainIds: number[] = []): MultisigAccount {
		if (!sameAddress(genesisAddress(genesis), address)) {
			throw new Error('the genesis does not make this address');
		}
		const existing = this.find(address);
		if (existing) {
			if (recorded && !existing.recorded) this.markRecorded(address);
			for (const chainId of chainIds) this.useChain(address, chainId);
			return this.find(address) ?? existing;
		}
		const { owners, threshold } = genesisMembers(genesis);
		const row: MultisigAccount = {
			address,
			genesis,
			owners,
			threshold,
			recorded,
			chainIds: [...new Set(chainIds)],
			addedMs: Date.now()
		};
		this.accounts = [...this.accounts, row];
		write(this.accounts);
		return row;
	}

	/** Note that the multisig is used on `chainId` (its proposals are looked for there). */
	useChain(address: string, chainId: number): void {
		const row = this.find(address);
		if (!row || row.chainIds.includes(chainId)) return;
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, chainIds: [...a.chainIds, chainId] } : a
		);
		write(this.accounts);
	}

	markRecorded(address: string): void {
		this.accounts = this.accounts.map((a) => (sameAddress(a.address, address) ? { ...a, recorded: true } : a));
		write(this.accounts);
	}

	remove(address: string): void {
		this.accounts = this.accounts.filter((a) => !sameAddress(a.address, address));
		write(this.accounts);
		if (sameAddress(this.#active, address)) this.activate(null);
	}

	/** Show this multisig (or, `null`, the session's own Vela account). */
	activate(address: string | null): void {
		this.#active = address;
		writeActive(address);
	}
}

export const multisig = new MultisigStore();
