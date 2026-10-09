/**
 * Parallel space — the developer test environment.
 *
 * Ported from src/services/dev/parallel-space.ts @ f9bcb278. The parallel
 * space is the real Vela app with exactly ONE difference: passkey signing is
 * served by a fixed keyset (`passkey-fixture.ts`) instead of a real
 * authenticator. Chains, relay, backend, storage, transports and UI are all
 * the real thing, which is what makes a test here a test of what ships.
 *
 * Boundary (real space ⇄ parallel space):
 *   - Passkey / WebAuthn: real credential → fixed fixture keyset (the ONLY change)
 *   - Wallet accounts: the person's → fixture Safes (swapped; the real cache is
 *     backed up on entry and restored on exit)
 *   - Everything else: unchanged
 *
 * Web deltas from the Expo module (spec 026 D19): accounts live in
 * localStorage through the onboarding storage (the same `vela.accounts` key
 * and the generated `Account` shape); contacts live in the IndexedDB KV; the
 * active flag is a rune in `parallel-flag.svelte.ts` rather than a `globalThis`
 * property (Metro's double-bundle hazard has no counterpart here); and the
 * gate is the RUNTIME dev gate, because the web e2e runs the production
 * artifact (D18).
 */
import type { Account } from '$lib/core/generated/Account';
import { loadCore } from '$lib/core/client';
import { setPasskeyOverride, relyingPartyId } from '$lib/onboarding/core/passkey';
import { STORAGE_KEYS } from '$lib/onboarding/core/storage';
import { getItem, setItem } from '$lib/services/storage';
import { PARALLEL_FLAG_KEY, setParallelActive } from './parallel-flag.svelte';
import {
	buildMockAssertion,
	fixtureAccount,
	fixtureAccounts,
	fixtureAddresses,
	fixtureMultiAddress,
	nextFixtureRegistration,
	resetFixtureRegistrationCursor,
	setPreferredMockSigner
} from './passkey-fixture';

const K_ACCOUNTS = STORAGE_KEYS.accounts;
const K_ACTIVE_INDEX = STORAGE_KEYS.activeAccountIndex;
const K_FLAG = PARALLEL_FLAG_KEY;
const K_BACKUP = 'vela.parallelSpace.realWalletBackup';
/** The address book lives in the KV (024), not localStorage. */
const K_CONTACTS = 'vela.contacts';

const FIXTURE_CREATED_AT = '2025-01-01T00:00:00.000Z';

/**
 * A fixture address-book entry so "send to a saved contact" resolves a name.
 * Seeded on enter/boot, removed by EXACT address on exit — the person's own
 * contacts are never touched.
 */
const FIXTURE_CONTACT = {
	address: '0x1234567890abcdef1234567890abcdef12345678',
	name: 'Alice Chen',
	kind: 'eoa' as const
};

// ---------------------------------------------------------------------------
// Fixture accounts as wallet records
// ---------------------------------------------------------------------------

/**
 * The fixture wallets as the web persists them: three single-key accounts and
 * the multi-key golden Safe founded on all three keys (the one the live sweep
 * spends from). `keys[0]` mirrors the scalar fields, as the stored shape
 * requires.
 */
export function fixtureStoredAccounts(): Account[] {
	const accounts = fixtureAccounts();
	// Spec 102: what the core writes for an account made in the app — keys on
	// `getvela.app`, reviewed and signed in Vela.
	const inVela = { signing_domain: 'getvela.app', signing_venue: { type: 'in_vela' as const } };
	const single = accounts.map((a): Account => ({
		id: a.id,
		name: a.name,
		address: a.address,
		public_key_hex: a.publicKeyHex,
		created_at_iso: FIXTURE_CREATED_AT,
		keys: [
			{
				credential_id: a.id,
				public_key_hex: a.publicKeyHex,
				name: a.name,
				transports: 'internal'
			}
		],
		...inVela
	}));
	const primary = fixtureAccount();
	const multi: Account = {
		id: primary.id,
		name: 'Parallel Multi',
		address: fixtureMultiAddress(),
		public_key_hex: primary.publicKeyHex,
		created_at_iso: FIXTURE_CREATED_AT,
		keys: accounts.map((a) => ({
			credential_id: a.id,
			public_key_hex: a.publicKeyHex,
			name: a.name,
			transports: 'internal'
		})),
		...inVela
	};
	return [...single, multi];
}

// ---------------------------------------------------------------------------
// The signer swap
// ---------------------------------------------------------------------------

/** Wire the fixed-key signer into the passkey module. Idempotent; badge on. */
export function installFixtureSigner(): void {
	resetFixtureRegistrationCursor();
	// Sign with the rpId the app actually reports (localhost in dev, the real
	// domain in production): a real authenticator hashes the CURRENT rpId, and
	// the registry verifies exactly that hash in every possession proof.
	const rpId = relyingPartyId();
	setPasskeyOverride({
		sign: async (challengeHex, credentialIds) =>
			buildMockAssertion(challengeHex, { credentialId: credentialIds, rpId })
	});
	setParallelActive(true);
}

/** Remove the fixed-key signer; badge off. */
export function uninstallFixtureSigner(): void {
	setPasskeyOverride(null);
	setParallelActive(false);
}

// ---------------------------------------------------------------------------
// Enter / exit / boot
// ---------------------------------------------------------------------------

function readLocal(key: string): string | null {
	try {
		return localStorage.getItem(key);
	} catch {
		return null;
	}
}

function writeLocal(key: string, value: string): void {
	try {
		localStorage.setItem(key, value);
	} catch {
		/* storage denied — the caller sees the flag never set */
	}
}

function removeLocal(key: string): void {
	try {
		localStorage.removeItem(key);
	} catch {
		/* nothing to undo */
	}
}

async function seedFixtureContact(): Promise<void> {
	try {
		const raw = await getItem(K_CONTACTS);
		const list = raw ? (JSON.parse(raw) as { address?: string }[]) : [];
		if (!Array.isArray(list)) return;
		if (list.some((c) => c?.address === FIXTURE_CONTACT.address)) return;
		list.push(FIXTURE_CONTACT);
		await setItem(K_CONTACTS, JSON.stringify(list));
	} catch {
		/* storage unavailable */
	}
}

async function removeFixtureContact(): Promise<void> {
	try {
		const raw = await getItem(K_CONTACTS);
		if (!raw) return;
		const list = JSON.parse(raw) as { address?: string }[];
		if (!Array.isArray(list)) return;
		const next = list.filter((c) => c?.address !== FIXTURE_CONTACT.address);
		if (next.length !== list.length) await setItem(K_CONTACTS, JSON.stringify(next));
	} catch {
		/* storage unavailable */
	}
}

/**
 * Enter: install the fixed-key signer and swap the fixture wallet into
 * storage. The real wallet cache is backed up on FIRST entry (re-entering must
 * not overwrite the backup with fixture data) and restored on exit; the true
 * keys live in the device passkey, never here, so the swap is safe and fully
 * reversible.
 */
export async function enterParallelSpace(): Promise<void> {
	// Deriving a fixture Safe is a core call, so the core comes first.
	await loadCore();
	installFixtureSigner();

	if (readLocal(K_FLAG) !== '1') {
		writeLocal(
			K_BACKUP,
			JSON.stringify({ accounts: readLocal(K_ACCOUNTS), idx: readLocal(K_ACTIVE_INDEX) })
		);
	}

	writeLocal(K_ACCOUNTS, JSON.stringify(fixtureStoredAccounts()));
	writeLocal(K_ACTIVE_INDEX, '0');
	writeLocal(K_FLAG, '1');
	await seedFixtureContact();
}

/** Exit: remove the signer, restore the real wallet cache, drop the fixture contact. */
export async function exitParallelSpace(): Promise<void> {
	uninstallFixtureSigner();

	const rawBackup = readLocal(K_BACKUP);
	let restored = false;
	if (rawBackup) {
		try {
			const { accounts, idx } = JSON.parse(rawBackup) as {
				accounts: string | null;
				idx: string | null;
			};
			if (accounts != null) writeLocal(K_ACCOUNTS, accounts);
			else removeLocal(K_ACCOUNTS);
			if (idx != null) writeLocal(K_ACTIVE_INDEX, idx);
			else removeLocal(K_ACTIVE_INDEX);
			restored = true;
		} catch {
			/* a corrupt backup falls through to the clear below */
		}
	}
	if (!restored) {
		removeLocal(K_ACCOUNTS);
		removeLocal(K_ACTIVE_INDEX);
	}

	removeLocal(K_BACKUP);
	removeLocal(K_FLAG);
	await removeFixtureContact();
}

/**
 * On boot: if the flag is set, re-install the fixed-key signer so a reload
 * inside the parallel space stays in it (the fixtures are already in storage).
 * Called unconditionally by the root layout — skipping the re-arm would boot
 * the fixture wallet UNMARKED, as if it were the real one. No-op in the real
 * space.
 */
export async function applyParallelSpaceOnBoot(): Promise<void> {
	if (readLocal(K_FLAG) !== '1') return;
	await loadCore();
	installFixtureSigner();
	await seedFixtureContact();
}

/**
 * Whether the space is on, read from storage. The light twin
 * (`parallel-flag.svelte.ts::parallelFlagSet`) is what product modules import,
 * so asking this question never pulls the fixture keys into their chunk.
 */
export function parallelSpaceActive(): boolean {
	return readLocal(K_FLAG) === '1';
}

// ---------------------------------------------------------------------------
// Console (vela.parallel.*)
// ---------------------------------------------------------------------------

export function installParallelConsole(): void {
	if (typeof window === 'undefined') return;
	const summary = () => ({
		active: parallelSpaceActive(),
		accounts: fixtureAccounts().map((a) => ({ name: a.name, id: a.id, address: a.address })),
		multi: fixtureMultiAddress()
	});
	const api = {
		async enter() {
			await enterParallelSpace();
			console.log('[vela] parallel space ON — reload to load the fixture wallet');
			return summary();
		},
		async exit() {
			await exitParallelSpace();
			console.log('[vela] parallel space OFF — reload to restore the real wallet');
			return summary();
		},
		status: () => summary(),
		addresses: () => fixtureAddresses(),
		/**
		 * Prefer fixture N whenever a multi-key allow-list offers it — the
		 * mock's stand-in for the provider's key picker. `signWith(null)`
		 * restores the first-allowed default.
		 */
		signWith(index: number | null) {
			setPreferredMockSigner(index);
			return summary();
		},
		/** Mint the next fixture credential (multi-key onboarding rehearsal). */
		nextCredential: () => nextFixtureRegistration().id,
		help() {
			console.log(
				'[vela.parallel] the real app, with only the passkey faked\n' +
					'  vela.parallel.enter()      seed the fixture wallet + fixed-key signer\n' +
					'  vela.parallel.exit()       restore the real wallet + remove the signer\n' +
					'  vela.parallel.status()     active state + fixture accounts\n' +
					'  vela.parallel.addresses()  fixture Safe addresses (fund these on-chain)\n' +
					'  vela.parallel.signWith(n)  prefer fixture n when an allow-list offers it'
			);
		}
	};
	const g = window as unknown as { vela?: Record<string, unknown> };
	g.vela = Object.assign(g.vela ?? {}, { parallel: api });
}
