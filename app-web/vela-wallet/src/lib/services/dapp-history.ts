// Ported from src/services/dapp-history.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * dApp signing-history records — pure helpers that turn an approved dApp request
 * into a LocalTransaction for the Connections panel.
 *
 * Kept free of React / native deps so the "every approved method is recorded"
 * guarantee can be unit-tested directly. The set of methods handled here MUST
 * stay in sync with isSigningMethod() in hooks/use-dapp-signing.ts — anything
 * that gets approved but isn't recorded silently vanishes from history.
 *
 * Spec 093: what the record says about the request is the core's — its
 * summary, its cut of the request and its `result` (empty for a signature) —
 * stored here verbatim. This file only projects the record's own fields.
 */
import type { DappSummary } from '$lib/core/generated/DappSummary';
import type { TrustSimJudgment } from '$lib/core/generated/TrustSimJudgment';
import { nativeSymbol } from './networks';
import { MAX_SIGNED_CONTENT, type LocalTransaction } from './transactions-model';
import type { StoredAssetSim } from './sim/tx-simulation';

/** Cap stored payloads so a huge typed-data blob can't bloat history. */
function cap(s: string | undefined): string | undefined {
	if (!s) return undefined;
	return s.length > MAX_SIGNED_CONTENT ? `${s.slice(0, MAX_SIGNED_CONTENT)}…` : s;
}

/**
 * The stored request back into its params (spec 093). The core cut it
 * (`SignRecord.stored_request`, ≤ 8 KB, JSON shape kept), so this only parses:
 * anything that does not read as a JSON array is stored as `[]` — no request
 * kept rather than half a one.
 */
export function storedParams(storedRequest: string): unknown[] {
	try {
		const parsed: unknown = JSON.parse(storedRequest);
		return Array.isArray(parsed) ? parsed : [];
	} catch {
		return [];
	}
}

/** Decode a hex message to readable text; keep hex if it decoded to binary. */
export function decodeSignMessage(raw: unknown): string | undefined {
	if (typeof raw !== 'string') return undefined;
	const clean = raw.startsWith('0x') ? raw.slice(2) : raw;
	if (clean.length === 0 || clean.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(clean)) {
		return raw; // already plain text (some dApps don't hex-encode)
	}
	try {
		const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map((b) => parseInt(b, 16)));
		const decoded = new TextDecoder().decode(bytes);
		// Keep hex if it decoded to binary (control chars / invalid UTF-8); else show text.
		// eslint-disable-next-line no-control-regex -- strips control characters on purpose
		return /[\x00-\x08\x0E-\x1F�]/.test(decoded) ? raw : decoded;
	} catch {
		return raw;
	}
}

/** Pull the human-meaningful signed payload out of a dApp request. */
export function extractSignedContent(
	method: string,
	params: unknown[] | undefined
): string | undefined {
	if (!params) return undefined;
	// personal_sign → [message, address]; eth_sign → [address, data] (reversed).
	if (method === 'personal_sign') return cap(decodeSignMessage(params[0]));
	if (method === 'eth_sign') return cap(decodeSignMessage(params[1]));
	if (method.includes('signTypedData')) {
		const rawData = params[1] ?? params[0];
		try {
			const obj = typeof rawData === 'string' ? JSON.parse(rawData) : rawData;
			return cap(JSON.stringify(obj, null, 2));
		} catch {
			return cap(typeof rawData === 'string' ? rawData : undefined);
		}
	}
	if (method === 'eth_sendTransaction') {
		const tx = params[0] as Record<string, string> | undefined;
		return tx?.data && tx.data !== '0x' ? cap(tx.data) : undefined;
	}
	if (method === 'wallet_sendCalls') {
		try {
			return cap(JSON.stringify(params[0], null, 2));
		} catch {
			return undefined;
		}
	}
	return undefined;
}

export interface SigningRecordInput {
	method: string;
	/** The FULL final params — the recipient/value projection and `signedContent` read them. */
	params: unknown[] | undefined;
	/**
	 * The request as the record keeps it — the core's `stored_request` (spec
	 * 093), never a cut of this shell's own — and whether it is shorter than
	 * the request was (`request_truncated`).
	 */
	storedRequest: string;
	requestTruncated: boolean;
	/** The core's summary of the request (`SignRecord.summary`), stored verbatim. */
	summary?: DappSummary | null;
	/**
	 * The sheet's simulation judgments as approved (`SignRecord.balance_changes`),
	 * stored verbatim (083 F1).
	 */
	balanceChanges?: TrustSimJudgment[] | null;
	/**
	 * The core's `result`: the tx hash a transaction settled under, `""` while
	 * pending — and always `""` for a signature, which the disk never keeps.
	 */
	result: unknown;
	from: string;
	chainId: number;
	dappOrigin: string;
	/**
	 * The origin the request arrived from (the core's `dapp_url`), kept beside
	 * `dappOrigin` — which holds the dApp's own name when it gave one — so
	 * Activity names the site from an address the dApp did not choose (083 H2).
	 */
	dappUrl?: string;
	/** Millisecond timestamp; drives both the unique id and the display time. */
	nowMs: number;
	/**
	 * Lifecycle status. Defaults to 'confirmed'. A tx is first recorded 'pending'
	 * the moment it's submitted (so closing the sheet can't lose it), then patched
	 * to 'confirmed'/'failed' once the on-chain receipt resolves.
	 */
	status?: 'pending' | 'confirmed' | 'failed';
	/** UserOp hash, kept on the pending record so receipt polling can resume. */
	userOpHash?: string;
	/** Sign-time asset-change simulation (predicted balance changes), JSON-safe. */
	assetChanges?: StoredAssetSim;
	/**
	 * ERC-7730 / best-effort clear-signing intent (e.g. "Swap", "Approve", "Permit"),
	 * captured at approve time. Persisted so the Connections list + detail view show a
	 * meaningful operation label instead of the generic "Contract interaction". The
	 * signing sheet re-derives intent live at replay time; this is the recorded copy.
	 */
	intent?: string;
	/**
	 * The submit's reply was lost and `userOpHash` is the local hash (spec 082
	 * RA3). Stored with the row so a reload follows it as may-have-been-sent.
	 */
	maybeSent?: boolean;
	/** The head read before the first submit POST (ruling 8); absent = unknown. */
	submitBlock?: number | null;
}

/**
 * The native value a transaction request moves, as hex wei: `value` of an
 * `eth_sendTransaction`, the SUM over every leg of a `wallet_sendCalls` batch
 * (its `params[0]` is `{calls}`, not a transaction — reading `.value` there
 * recorded every batch as moving nothing). Hex or decimal legs, as the submit
 * path accepts them; anything unreadable counts as zero.
 */
function requestedValueHex(method: string, first: Record<string, unknown> | undefined): string {
	if (method !== 'wallet_sendCalls') {
		return typeof first?.value === 'string' ? first.value : '0x0';
	}
	const calls = Array.isArray(first?.calls) ? (first.calls as Record<string, unknown>[]) : [];
	let total = 0n;
	for (const call of calls) {
		const value = call?.value;
		if (typeof value !== 'string' || value === '' || value === '0x') continue;
		try {
			total += BigInt(value);
		} catch {
			/* unreadable — the submit refuses it; the row counts nothing */
		}
	}
	return '0x' + total.toString(16);
}

/**
 * Build the history record for an approved request. Returns a record for ANY
 * method (the fallback is a message signature) so an approved request is never
 * dropped. Transactions (`eth_sendTransaction`, `wallet_sendCalls`) carry the
 * recipient/value/hash; everything else is a signature.
 */
export function buildSigningRecord(input: SigningRecordInput): LocalTransaction {
	const {
		method,
		params,
		storedRequest,
		requestTruncated,
		summary,
		balanceChanges,
		result,
		from,
		chainId,
		dappOrigin,
		dappUrl,
		nowMs,
		status = 'confirmed',
		userOpHash = '',
		assetChanges,
		intent,
		maybeSent = false,
		submitBlock = null
	} = input;
	const now = Math.floor(nowMs / 1000);
	const signedContent = extractSignedContent(method, params);
	const signedRequest = { method, params: storedParams(storedRequest) };
	const base = {
		userOpHash,
		// The core's answer verbatim: a transaction's hash, and nothing for a
		// signature — the signature itself is never written down (spec 093).
		txHash: typeof result === 'string' ? result : '',
		from,
		chainId,
		timestamp: now,
		status,
		dappOrigin,
		...(dappUrl ? { dappUrl } : {}),
		signedContent,
		signedRequest,
		requestTruncated,
		...(summary ? { dappSummary: summary } : {}),
		...(balanceChanges && balanceChanges.length > 0 ? { balanceChanges } : {}),
		assetChanges,
		intent,
		...(maybeSent ? { maybeSent: true } : {}),
		...(submitBlock != null ? { submitBlock } : {})
	};

	if (method === 'eth_sendTransaction' || method === 'wallet_sendCalls') {
		const first = (Array.isArray(params) ? params[0] : undefined) as
			Record<string, unknown> | undefined;
		// A batch names its first leg's recipient, as its sheet does (RC7).
		const leg =
			method === 'wallet_sendCalls' && Array.isArray(first?.calls)
				? (first.calls[0] as Record<string, unknown> | undefined)
				: first;
		return {
			...base,
			id: `dapp-${nowMs}-tx`,
			to: typeof leg?.to === 'string' ? leg.to : '',
			value: requestedValueHex(method, first),
			symbol: nativeSymbol(chainId),
			decimals: 18,
			type: 'dapp_tx'
		};
	}
	if (method.includes('signTypedData')) {
		return {
			...base,
			id: `dapp-${nowMs}-typed`,
			to: '',
			value: '0',
			symbol: '',
			decimals: 0,
			type: 'sign_typed_data'
		};
	}
	// personal_sign, eth_sign, and any other approved signing method.
	return {
		...base,
		id: `dapp-${nowMs}-msg`,
		to: '',
		value: '0',
		symbol: '',
		decimals: 0,
		type: 'sign_message'
	};
}

/**
 * Build the "Connected to <app>" audit record for a dApp session grant. Unlike a
 * signing record this carries no signature/tx — it just marks that a connection to
 * `dappOrigin` was authorized at `nowMs`, so every surface has a tappable session
 * trail (the in-app browser previously left only a silent grant). Written once per
 * user-approved connection (the consent moment is naturally deduped), never on an
 * auto-reconnect. Reuses the single transactionHistory store.
 */
export function buildConnectionRecord(input: {
	from: string;
	chainId: number;
	dappOrigin: string;
	nowMs: number;
}): LocalTransaction {
	const { from, chainId, dappOrigin, nowMs } = input;
	return {
		id: `dapp-${nowMs}-connect`,
		userOpHash: '',
		txHash: '',
		from,
		to: '',
		value: '0',
		symbol: '',
		decimals: 0,
		chainId,
		timestamp: Math.floor(nowMs / 1000),
		status: 'confirmed',
		type: 'connect',
		dappOrigin
	};
}
