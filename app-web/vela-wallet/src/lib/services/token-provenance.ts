/**
 * Which token a row names, said where a name alone could lie (r4a-2).
 *
 * A token's symbol is whatever its contract answers: a stranger's contract
 * can call itself "USDC". The core rules who vouches for a name
 * (`token_provenance`) — the chain's own coin, a token the wallet's registry
 * lists on that chain, or nobody — and whether an unvouched token's symbol
 * reads as a vouched one there. Every surface that names a token draws the
 * same note from it (the send picker, form and confirm, a multisig's waiting
 * rows and its sheet): an unverified token shows its contract, a lookalike says
 * it is not the real one. A vouched token draws nothing.
 */
import { checksumAddress, tokenProvenance } from '$lib/core/client';
import { explorerAddressURL, nativeSymbol } from '$lib/services/networks';
import { fillTemplate } from '$lib/i18n/fill';
import { shortenAddress } from '$lib/wallet/identity';

/** The core's `TokenProvenance`. */
export interface TokenProvenance {
	trust: 'native' | 'vouched' | 'unverified';
	/** For an unverified token: the vouched symbol its own reads as. */
	imitates: string | null;
}

/** The words the note is drawn with (`home.multisig.token`). */
export interface ProvenanceWords {
	unverified: string;
	/** `{{symbol}}`: the real token's. */
	lookalike: string;
}

/** What a surface draws beside an unvouched token. */
export interface ProvenanceNote {
	/** "Unverified token", or "Not the real USDC — …". */
	text: string;
	/** `danger` for a lookalike; `caution` for any other unverified token. */
	tone: 'caution' | 'danger';
	/** The contract, whole (what a copy writes) and short (what a row shows). */
	contract: string;
	short: string;
	explorerUrl: string;
}

/** The core's ruling on `token` (`null`: the native coin) named `symbol` on `chainId`. */
export function provenanceOf(
	chainId: number,
	token: string | null | undefined,
	symbol: string
): TokenProvenance {
	try {
		return JSON.parse(
			tokenProvenance(chainId, token ?? '', symbol, nativeSymbol(chainId))
		) as TokenProvenance;
	} catch {
		// The core not loaded: a token is not vouched for by a failure.
		return token ? { trust: 'unverified', imitates: null } : { trust: 'native', imitates: null };
	}
}

/** The note an unvouched token wears, or `null` for one the wallet vouches for. */
export function provenanceNote(
	chainId: number,
	token: string | null | undefined,
	symbol: string,
	words: ProvenanceWords
): ProvenanceNote | null {
	if (!token) return null;
	const ruled = provenanceOf(chainId, token, symbol);
	if (ruled.trust !== 'unverified') return null;
	// Checksummed, as every other address the wallet shows (a token's record
	// keeps its contract lowercased).
	let contract = token;
	try {
		contract = checksumAddress(token);
	} catch {
		// not an address the core reads: shown as it came
	}
	return {
		text:
			ruled.imitates !== null
				? fillTemplate(words.lookalike, { symbol: ruled.imitates })
				: words.unverified,
		tone: ruled.imitates !== null ? 'danger' : 'caution',
		contract,
		short: shortenAddress(contract),
		explorerUrl: explorerAddressURL(chainId, contract)
	};
}
