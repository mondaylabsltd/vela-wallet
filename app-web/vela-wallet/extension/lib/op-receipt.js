/**
 * Receipt reads for the operation hashes the extension handed out (spec 082
 * RF3, W12) — pure rules for `background.js`.
 *
 * An on-chain dApp request is sometimes answered with its ERC-4337 OPERATION
 * hash rather than a transaction hash: when the receipt was late (079's
 * still-confirming contract) or the relay's reply was lost (082 RA2, "may have
 * been sent"). A dApp then polls `eth_getTransactionReceipt(<that hash>)`,
 * which no node knows, and before 082 it polled `null` forever.
 *
 * The in-app browsers already translate (`dapp-submit.ts`'s receipt case,
 * `dapp_browser.rs`): ask the bundler for the operation's receipt, and forward
 * the page's own method for the REAL transaction hash — the authentic node
 * answer, never a synthesised one. This is the same rule for the worker: the
 * panel's answer carries `opHash: {chainId}`, the worker keeps
 * `vela.ext.op.<hash>` for 24 h, and a receipt read for a recorded hash is
 * translated on the chain the operation was sent on. A hash it never handed
 * out is forwarded untouched.
 */

/** `vela.ext.op.<lower-case hash>` → `{ chainId, at }`, in storage.local. */
export const OP_PREFIX = 'vela.ext.op.';
/** How long a handed-out operation hash is translated. */
export const OP_TTL_MS = 24 * 60 * 60 * 1000;
/** The page's receipt reads that are translated. */
export const RECEIPT_METHODS = new Set(['eth_getTransactionReceipt', 'eth_getTransactionByHash']);

const HASH32 = /^0x[0-9a-fA-F]{64}$/;

/** A 32-byte hex hash. */
export function isHash32(value) {
	return typeof value === 'string' && HASH32.test(value);
}

/** The storage key for `hash`. */
export function opKey(hash) {
	return OP_PREFIX + hash.toLowerCase();
}

/**
 * What to write when an answer went out: `{ key, value }` for an answer that
 * is an operation hash with a chain, else `null`.
 */
export function opRecord(result, opHash, now) {
	if (!isHash32(result)) return null;
	const chainId = opHash && typeof opHash === 'object' ? Number(opHash.chainId) : NaN;
	if (!Number.isInteger(chainId) || chainId <= 0) return null;
	return { key: opKey(result), value: { chainId, at: now } };
}

/**
 * The hash a read asks about, when it is one this rule may translate: a
 * receipt method whose first param is a 32-byte hash. `null` otherwise.
 */
export function receiptLookupHash(method, params) {
	if (!RECEIPT_METHODS.has(method)) return null;
	const first = Array.isArray(params) ? params[0] : undefined;
	return isHash32(first) ? first : null;
}

/** The stored entry, when it is still inside its 24 hours; else `null`. */
export function liveOpEntry(entry, now) {
	if (!entry || typeof entry !== 'object') return null;
	const chainId = Number(entry.chainId);
	if (!Number.isInteger(chainId) || chainId <= 0) return null;
	if (typeof entry.at !== 'number' || now - entry.at >= OP_TTL_MS) return null;
	return { chainId, at: entry.at };
}

/**
 * The real transaction hash in an `eth_getUserOperationReceipt` result, or
 * `null` while the operation has not landed (the page is then answered `null`
 * and keeps polling, as it would for an unmined transaction).
 */
export function realTxHash(userOpReceipt) {
	const hash =
		userOpReceipt && typeof userOpReceipt === 'object'
			? userOpReceipt.receipt?.transactionHash
			: undefined;
	return isHash32(hash) ? hash : null;
}

/** The stored keys whose 24 hours are over (pruned at worker start). */
export function expiredOpKeys(all, now) {
	if (!all || typeof all !== 'object') return [];
	return Object.keys(all).filter(
		(key) => key.startsWith(OP_PREFIX) && liveOpEntry(all[key], now) === null
	);
}
