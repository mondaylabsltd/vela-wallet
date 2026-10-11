/**
 * Which passkeys control a wallet — WEB transport for `vela_core::wallet_keys`
 * (spec 062).
 *
 * The walk reads the registry CONTRACT (Gnosis, then the Ethereum backup) for
 * the wallet's founding set, names each key's vault from its attestation, and
 * falls back to what this device's account record remembers when no chain
 * answers. Everything that decides anything is the core's; this file carries
 * the `eth_call`s (`core-walk.ts`) and nothing else.
 *
 * Not cached: it is asked when a person opens one page, and the founding set of
 * a wallet that was just created is exactly the thing a cache would get wrong.
 */
import { loadCore, walletKeysStep } from '$lib/core/client';
import { signingPlan } from '$lib/core/kernels';
import type { Account } from '$lib/core/generated/Account';
import type { CreateKeyRow } from '$lib/onboarding/generated/CreateKeyRow';
import { runWalk } from './core-walk';

/**
 * `wallet_keys::WalletKeyRow` — `CreateKeyRow`'s fields, plus the key itself.
 *
 * Minus the two the create flow alone carries: `kind` (this view's `method` is
 * already the authenticator's report, not a tap — `wallet_keys::method_of`) and
 * `synced_known`, which here is simply `synced !== null`.
 */
export interface WalletKeyRow extends Omit<CreateKeyRow, 'synced' | 'kind' | 'synced_known'> {
	/** `null` when only the device answered: nobody can vouch for a badge. */
	synced: boolean | null;
	public_key_hex: string;
	/** base64url, as authenticators and the registry explorer print it; empty from the device. */
	credential_id: string;
	/** The registry's 20-byte attestation summary, `0x`-hex; empty from the device. */
	attestation_hex: string;
	/** The authenticator verified the person at registration; `null` = nobody can vouch. */
	user_verified: boolean | null;
	/**
	 * The key this device signs with — the one the account signed in with
	 * (founder, 2026-09-26). Exactly one row, or none for a record from before
	 * the sign-in key.
	 */
	signs_here: boolean;
}

export interface WalletKeys {
	/**
	 * `device` = no chain answered. `not_registered` = one did, and holds no
	 * record for this address: the same rows, but nothing was unreachable.
	 */
	source: 'registry' | 'device' | 'not_registered';
	/** The chain whose registry answered; `null` otherwise. */
	chainId: number | null;
	keys: WalletKeyRow[];
	/**
	 * A chain is still being asked (`deviceOnlyKeys`): the rows are this
	 * device's own record — how many keys, in founding order, their names, the
	 * one signed in with — and the answer will fill them in where they stand.
	 * Everything else on such a row is nobody's word yet: where a key lives is
	 * read here off the record's transports alone, and a registry that says
	 * "security key" a moment later would contradict it. A shell draws those
	 * fields as room, not as words.
	 */
	asking?: true;
}

interface KeysDone {
	type: 'done';
	source: 'registry' | 'device' | 'not_registered';
	chain_id: number | null;
	keys: WalletKeyRow[];
}

/**
 * The account record's key list in founding order; a legacy record is a list of
 * one.
 *
 * No page origin travels any more (spec 102): a row is captioned by where its
 * key LIVES — this device, a phone, a security key — never by a page it was
 * minted on, because the same key signs in Vela and on a page alike.
 *
 * `credential_id` is how the walk finds the row of the key this device signs
 * with; a legacy record's one credential is its `id`.
 */
export function deviceKeys(account: Pick<Account, 'id' | 'name' | 'public_key_hex' | 'keys'>) {
	if (account.keys.length > 0) {
		return account.keys.map((key) => ({
			credential_id: key.credential_id,
			public_key_hex: key.public_key_hex,
			name: key.name,
			transports: key.transports
		}));
	}
	return [
		{
			credential_id: account.id,
			public_key_hex: account.public_key_hex,
			name: account.name,
			transports: ''
		}
	];
}

/** The credential of the account's sign-in route; empty when it has none. */
const signInCredential = (account: Account) => signingPlan(account)?.key?.credential_id ?? '';

/**
 * The walk run with no address: it asks no chain, and its answer is what the
 * device's record alone says — the core's own rows, the sign-in key marked.
 */
function deviceAnswer(deviceJson: string, signIn: string): WalletKeyRow[] {
	return (JSON.parse(walletKeysStep('', deviceJson, '[]', signIn)) as KeysDone).keys;
}

/**
 * What stands in the keys block from its first frame, before any chain has
 * been asked (device walk, item 13.1): the device's own record. It already
 * says how many keys the wallet has and what each is called, so the rows, the
 * count and everything under them are where they will be when the registry
 * answers — Settings used to draw one grey row and drop by the height of the
 * rest (three keys: 202 px on a phone).
 *
 * Synchronous, so it is there with the account: the core is the module the
 * session already loaded. `null` when it is not (nothing can be read yet), or
 * for a record with no key to show.
 */
export function deviceOnlyKeys(account: Account): WalletKeys | null {
	try {
		const keys = deviceAnswer(JSON.stringify(deviceKeys(account)), signInCredential(account));
		return keys.length === 0 ? null : { source: 'device', chainId: null, keys, asking: true };
	} catch {
		return null;
	}
}

/**
 * The keys that control `account`, the one it signed in with marked. Never
 * throws; the worst answer is the device's own.
 */
export async function readWalletKeys(account: Account): Promise<WalletKeys> {
	const device = deviceKeys(account);
	const fallback: WalletKeys = { source: 'device', chainId: null, keys: [] };
	try {
		await loadCore();
		const json = JSON.stringify(device);
		const signIn = signInCredential(account);
		const done = await runWalk<KeysDone>((answers) =>
			walletKeysStep(account.address, json, answers, signIn)
		);
		if (done !== null) return { source: done.source, chainId: done.chain_id, keys: done.keys };
		// The walk gave up mid-way (round cap): ask it what the device alone says.
		return { source: 'device', chainId: null, keys: deviceAnswer(json, signIn) };
	} catch {
		return fallback;
	}
}
