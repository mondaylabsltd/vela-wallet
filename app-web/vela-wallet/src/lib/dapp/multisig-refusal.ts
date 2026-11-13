/**
 * What the request surface says when the proposals machine refuses a site's
 * request to a multisig (`HeldStage::Refused`, `refusal`), before the site is
 * told — in the words the refusal is for:
 *
 * - §31 (`data.reason === "parent_safe"`): the site asked this multisig to
 *   act for a Safe it owns — the signing sheet's own refusal title, and the
 *   sentence the core names in `data.key` (`componentsUi.signing.
 *   parentSafeBlocked`, or `…Unchecked` when ownership could not be read),
 *   with the call and the Safe;
 * - §35 (`data.reason === "self_call"`, SEC-PARENT-2): the site asked this
 *   multisig to call itself and change who controls it — the same title, the
 *   sentence `componentsUi.signing.multisigSelfCallBlocked` with the call;
 * - anything else: dapp-7's "a multisig can't sign messages yet".
 *
 * The core decides which; this only reads its answer.
 */
import { checksumAddress } from '$lib/core/kernels';
import { shortenAddress } from '$lib/wallet/identity';
import type { RequestMessages } from './messages';

export interface MultisigRefusalWords {
	title: string;
	body: string;
}

const UNCHECKED_KEY = 'componentsUi.signing.parentSafeUnchecked';

/**
 * r18-23: the Safe in its EIP-55 checksummed form, shortened (the core's
 * `checksumAddress`) — the signing sheet's own, never the lowercase the
 * refusal's data carries.
 */
function safeShort(address: string): string {
	try {
		return shortenAddress(checksumAddress(address));
	} catch {
		return shortenAddress(address);
	}
}

export function multisigRefusalWords(
	error: { data?: unknown } | null | undefined,
	host: string,
	m: Pick<
		RequestMessages,
		| 'multisigNoMessagesTitle'
		| 'multisigNoMessagesBody'
		| 'parentSafeTitle'
		| 'parentSafeBlocked'
		| 'parentSafeUnchecked'
		| 'multisigSelfCallBlocked'
	>
): MultisigRefusalWords {
	const data = (error?.data ?? null) as {
		reason?: unknown;
		key?: unknown;
		function?: unknown;
		safe?: unknown;
	} | null;
	if (data?.reason === 'parent_safe') {
		const sentence = data.key === UNCHECKED_KEY ? m.parentSafeUnchecked : m.parentSafeBlocked;
		return {
			title: m.parentSafeTitle,
			body: sentence
				.replaceAll('{{function}}', typeof data.function === 'string' ? data.function : '')
				.replaceAll('{{safe}}', typeof data.safe === 'string' ? safeShort(data.safe) : '')
		};
	}
	if (data?.reason === 'self_call') {
		return {
			title: m.parentSafeTitle,
			body: m.multisigSelfCallBlocked.replaceAll(
				'{{function}}',
				typeof data.function === 'string' ? data.function : ''
			)
		};
	}
	return {
		title: m.multisigNoMessagesTitle,
		body: m.multisigNoMessagesBody.replace('{{host}}', host)
	};
}
