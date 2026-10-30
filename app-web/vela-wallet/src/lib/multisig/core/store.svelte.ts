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
import type { Finding } from './refusal';
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
	/**
	 * Whether the genesis service can hand the genesis to another device — the
	 * registry holds it, or the service keeps it until the record lands.
	 */
	shared?: boolean;
	/** The networks it is used on: where its proposals are looked for. */
	chainIds: number[];
	/**
	 * The person's own name for it on this device — an imported Safe's name
	 * from the Safe{Wallet} address book. Never part of the Safe; a name in
	 * its genesis comes first.
	 */
	label?: string;
	/**
	 * What this device knows of its members (r4D-5), by lowercase address: a
	 * name and a kind, typed by its creator or carried by an invite. Labels
	 * only — who may act is the genesis's and each chain's, never this.
	 */
	members?: Record<string, MemberInfo>;
	/**
	 * r4D-8: created on this device, its first steps (invite, fund, what the
	 * first transfer costs) not dismissed yet.
	 */
	firstRun?: boolean;
	addedMs: number;
	/**
	 * What else can act for it, as the chain showed when the person added it
	 * anyway (§3.5: a module, a guard, a handler or singleton that is not
	 * Safe's) — kept to be shown wherever it is, never hidden. The re-check
	 * (`recheck.ts`) keeps each network's current: a risk found there since,
	 * or its genesis refused on a network it is not deployed on (read-only
	 * there).
	 */
	findings?: Finding[];
}

/** A member's label and kind, as one device knows it (never authority). */
export interface MemberInfo {
	name?: string;
	/**
	 * A Vela account (its key a passkey), an outside wallet, or (r7) a smart
	 * contract account that approves from its own app.
	 */
	kind?: 'passkey' | 'wallet' | 'contract';
	/**
	 * r5reg-16: `name` is what the person typed on this device (Settings →
	 * Add member), not a link's label — shown for that member over any other.
	 */
	typed?: true;
}

/**
 * Whether another device can find it by its address: the service hands its
 * genesis out (`shared`), or the registry holds it (a row written before
 * `shared` existed says only `recorded`).
 */
export function isShared(account: Pick<MultisigAccount, 'shared' | 'recorded'>): boolean {
	return account.shared === true || account.recorded;
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
	add(
		address: string,
		genesis: Genesis,
		recorded: boolean,
		chainIds: number[] = []
	): MultisigAccount {
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

	/** This device's own name for it (shown when its genesis has none). */
	setLabel(address: string, label: string | null): void {
		const clean = label?.trim() || undefined;
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, label: clean } : a
		);
		write(this.accounts);
	}

	/**
	 * Keep what is known of its members (r4D-5). What this device already
	 * holds stays: a field is only filled where it is empty — the person's own
	 * names over a link's. `known` is keyed by address, any case. `typed`: the
	 * person typed these just now (Settings → Add member, r5reg-16) — their
	 * word replaces what was kept.
	 */
	nameMembers(
		address: string,
		known: Record<string, MemberInfo>,
		{ typed = false }: { typed?: boolean } = {}
	): void {
		const row = this.find(address);
		if (!row) return;
		const members = { ...(row.members ?? {}) };
		let changed = false;
		for (const [member, info] of Object.entries(known)) {
			const key = member.toLowerCase();
			const was = members[key] ?? {};
			const given = info.name?.trim() || undefined;
			const name = typed ? (given ?? was.name) : (was.name ?? given);
			const kind = typed ? (info.kind ?? was.kind) : (was.kind ?? info.kind);
			const mine = (typed && !!given) || was.typed === true;
			if (name === was.name && kind === was.kind && mine === (was.typed === true)) continue;
			members[key] = {
				...(name ? { name } : {}),
				...(kind ? { kind } : {}),
				...(mine && name ? { typed: true as const } : {})
			};
			changed = true;
		}
		if (!changed) return;
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, members } : a
		);
		write(this.accounts);
	}

	/** r4D-8: show (or, `false`, stop showing) its first steps on the home. */
	setFirstRun(address: string, on: boolean): void {
		const row = this.find(address);
		if (!row || !!row.firstRun === on) return;
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, firstRun: on || undefined } : a
		);
		write(this.accounts);
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
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, recorded: true, shared: true } : a
		);
		write(this.accounts);
	}

	/**
	 * The service answered it holds no record of it (one in an earlier
	 * registry, which v4's service no longer reads): not shared, said on
	 * screen, and asked again — never left looking shared.
	 */
	markUnshared(address: string): void {
		const row = this.find(address);
		if (!row || (!row.recorded && !row.shared)) return;
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, recorded: false, shared: false } : a
		);
		write(this.accounts);
	}

	/**
	 * The name its record holds now (the multisig renamed itself): the
	 * genesis keeps every field that makes the address, only its label moves.
	 */
	setRecordedName(address: string, name: string): void {
		const row = this.find(address);
		if (!row || row.genesis.name === name) return;
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, genesis: { ...a.genesis, name } } : a
		);
		write(this.accounts);
	}

	/** What else can act for it, accepted by the person when it was added (shown, never hidden). */
	setFindings(address: string, findings: Finding[]): void {
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, findings } : a
		);
		write(this.accounts);
	}

	/**
	 * What the chain says now about `chainId` (the re-check, §3.5): that
	 * network's findings are replaced by `found`, every other network's kept.
	 * Written only when something changed.
	 */
	setFindingsOn(address: string, chainId: number, found: Finding[]): void {
		const row = this.find(address);
		if (!row) return;
		const kept = (row.findings ?? []).filter((f) => f.chainId !== chainId);
		const next = [...kept, ...found];
		if (JSON.stringify(next) === JSON.stringify(row.findings ?? [])) return;
		this.setFindings(address, next);
	}

	markShared(address: string): void {
		const row = this.find(address);
		if (!row || row.shared) return;
		this.accounts = this.accounts.map((a) =>
			sameAddress(a.address, address) ? { ...a, shared: true } : a
		);
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
