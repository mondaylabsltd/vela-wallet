/**
 * Who can act for a multisig already on this device, checked again (§3.5).
 * A multisig added before the counterfactual rule existed was never vetted,
 * and a deployed one can gain a module, a guard or a handler later — so
 * once a session, and on each network it comes to be used on, every stored
 * multisig is judged as adding one is: deployed there, the chain's state
 * (`deployedOn`); not deployed there, the core's counterfactual rule
 * (`counterfactualOn`). What is found replaces that network's findings on
 * the account — a risk marks it high risk, a refused genesis makes it
 * read-only there — and is shown wherever it is drawn. Never silent: a
 * network that does not answer is not counted as checked, and is asked
 * again.
 */
import { networkForChainId } from '$lib/services/networks';
import { judgeOn } from './judge';
import { hasCode } from './chain';
import { sameAddress } from './kernel';
import type { Finding } from './refusal';
import { multisig, type MultisigAccount } from './store.svelte';

/** This session's verdicts, by `address:chainId` (lowercase). */
const verdicts = new Map<string, Finding[]>();
/** Checks under way, so two callers read the chain once. */
const pending = new Map<string, Promise<Finding[] | null>>();

function keyOf(address: string, chainId: number): string {
	return `${address.toLowerCase()}:${chainId}`;
}

/**
 * What the chain says about `account` on `chainId` now, as findings: `[]`
 * — only its members act for it (or its genesis is Verified there);
 * `null` — the chain did not answer, nothing can be said.
 */
async function judge(account: MultisigAccount, chainId: number): Promise<Finding[] | null> {
	let deployed: boolean;
	try {
		deployed = await hasCode(chainId, account.address);
	} catch {
		return null;
	}
	return judgeOn(chainId, account.address, account.genesis, deployed);
}

/**
 * `account` on `chainId`, checked once this session (a check under way is
 * shared). The verdict is written to the stored account, if it is one.
 * `null`: the chain did not answer — not remembered, asked again next time.
 */
export function checkOn(account: MultisigAccount, chainId: number): Promise<Finding[] | null> {
	const key = keyOf(account.address, chainId);
	const known = verdicts.get(key);
	if (known) return Promise.resolve(known);
	const running = pending.get(key);
	if (running) return running;
	const run = judge(account, chainId)
		.then((found) => {
			if (found === null) return null;
			verdicts.set(key, found);
			if (multisig.find(account.address)) multisig.setFindingsOn(account.address, chainId, found);
			return found;
		})
		.finally(() => pending.delete(key));
	pending.set(key, run);
	return run;
}

/**
 * Every multisig on this device, on each of its networks this device has,
 * that this session has not checked yet — one multisig at a time, so a
 * device with many does not spend the RPCs' budget at once.
 */
export async function recheckStored(): Promise<void> {
	for (const account of [...multisig.accounts]) {
		const due = account.chainIds.filter(
			(id) => networkForChainId(id) !== null && !verdicts.has(keyOf(account.address, id))
		);
		await Promise.all(due.map((id) => checkOn(account, id)));
	}
}

/** Whether `address` has been checked on `chainId` this session. */
export function checkedOn(address: string, chainId: number): boolean {
	return verdicts.has(keyOf(address, chainId));
}

/** A new session (tests): nothing checked. */
export function forgetChecks(): void {
	verdicts.clear();
	pending.clear();
}

/** The findings of `account` on `chainId`: the stored row's, else this session's. */
export function findingsOn(
	account: Pick<MultisigAccount, 'address' | 'findings'>,
	chainId: number
): Finding[] {
	const stored = multisig.accounts.find((a) => sameAddress(a.address, account.address));
	const all = stored?.findings ?? account.findings ?? verdicts.get(keyOf(account.address, chainId));
	return (all ?? []).filter((f) => f.chainId === chainId);
}
