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
import { signInRoute } from '$lib/core/kernels';
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
	 * Spec 075: the Trusted Signer page this key lives behind, when it does —
	 * omitted by the core when it does not (`skip_serializing_if`). The core
	 * knows it only because [`deviceKeys`] passes it in from the account record:
	 * the registry stores no origin.
	 *
	 * A row with one has `method === 'trusted_signer'`, which OUTRANKS the
	 * authenticator's report: a page runs its ceremony in a browser, so it
	 * always answers `platform`, and that names the one side of the page this
	 * wallet cannot reach.
	 */
	signer_origin?: string | null;
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
 * `signer_origin` travels with each key (spec 075): the walk cannot learn it
 * anywhere else — the registry contract stores no origin — so a key behind a
 * Trusted Signer page is drawn as a passkey on some device unless this list says
 * otherwise. A legacy record predates the field and has none.
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
			transports: key.transports,
			signer_origin: key.signer_origin ?? null
		}));
	}
	return [
		{
			credential_id: account.id,
			public_key_hex: account.public_key_hex,
			name: account.name,
			transports: '',
			signer_origin: null
		}
	];
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
		const signIn = signInRoute(account)?.credential_id ?? '';
		const done = await runWalk<KeysDone>((answers) =>
			walletKeysStep(account.address, json, answers, signIn)
		);
		if (done !== null) return { source: done.source, chainId: done.chain_id, keys: done.keys };
		// The walk gave up mid-way (round cap): ask it what the device alone says.
		const alone = JSON.parse(walletKeysStep('', json, '[]', signIn)) as KeysDone;
		return { source: 'device', chainId: null, keys: alone.keys };
	} catch {
		return fallback;
	}
}
