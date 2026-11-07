/**
 * The request window's message manifest (spec 027 T322).
 *
 * Client-safe: names keys and shapes only — resolution happens in
 * `engine.server.ts` at build time, exactly like `wallet/messages.ts` and
 * `explore/messages.ts`.
 *
 * Every key here already exists in the corpus. The words a person reads when a
 * site asks for their address were written for the in-app browser's connect
 * sheet (spec 022), and they say the right thing wherever the request came
 * from — which is the point of keeping copy in one corpus rather than one per
 * surface.
 */
import type { NetHintKey } from '$lib/settings/messages';

export interface RequestMessages {
	/** Template — 'Connect to {{host}}'. */
	title: string;
	body: string;
	connect: string;
	cancel: string;
	preparing: string;
	/**
	 * The request window with nobody signed in (spec 094 S5): the panel goes to
	 * the welcome, the window cannot (it is the request), so it says this and
	 * opens the welcome in a tab.
	 */
	noWallet: string;
	createWallet: string;
	haveWallet: string;
	/** dapp-8: the window's title while a signature waits for somebody to sign in. */
	signInTitle: string;
	/**
	 * dapp-7: a multisig asked for a message signature — what it cannot give
	 * yet, and why (template — '{{host}}'), said before the site is told no;
	 * and the line a connect for a multisig adds.
	 */
	multisigNoMessagesTitle: string;
	multisigNoMessagesBody: string;
	multisigConnectNote: string;
	/**
	 * §31: a site asked a multisig to act for a Safe it owns (the core's
	 * refusal, `data.reason: "parent_safe"`) — said in the signing sheet's own
	 * words: its refusal title, and the sentence `data.key` names (templates —
	 * '{{function}}', '{{safe}}').
	 */
	parentSafeTitle: string;
	parentSafeBlocked: string;
	parentSafeUnchecked: string;
	/**
	 * r10 follow-up (template — '{{network}}'): a network-space switch moved
	 * the site since its last request; the consent says so once.
	 */
	spaceMoved: string;
	gotIt: string;
	/**
	 * Spec 096 F11: the consent names what a Connect shares — the account and
	 * the network — under the labels the in-app browsers' consent uses
	 * (`explore.account`, `explore.network`).
	 */
	accountLabel: string;
	networkLabel: string;
	/** Spec 100: a page asks to add a network — Settings' words wherever they fit. */
	addNetwork: AddNetworkMessages;
}

/**
 * Spec 100: the add-network sheet's words. Three are new to the corpus
 * (`connect.browser.addLead` / `addFromSite`, `componentsUi.browserStatus.
 * reason.badRpc`); every other one is Settings' add-network wizard's own.
 */
export interface AddNetworkMessages {
	title: string;
	/** Template — '{{host}} asks to add a network'. */
	lead: string;
	fromSite: string;
	name: string;
	chainIdLabel: string;
	nativeToken: string;
	rpcUrl: string;
	explorer: string;
	/** Template — 'Chain {{chainId}}', while the name is not known yet. */
	chainName: string;
	checking: string;
	compatible: string;
	incompatible: string;
	/**
	 * The line under a refusal, by corpus key — the core says which
	 * (`NetCompatibility.hint_key`): no P-256 verifier, or missing contracts.
	 */
	hints: Record<NetHintKey, string>;
	singleKeyOnly: string;
	/** Owner decision 2026-10-29: a Vela network out of the list — the pill, and what Add does. */
	velaNetwork: string;
	velaNetworkHint: string;
	unableToVerify: string;
	/** dapp-13: the catalog knows the chain but lists no RPC Vela uses — the RPC is the site's. */
	rpcFromSite: string;
	/** Why it could not verify: no RPC Vela may use. */
	whyNoRpc: string;
	/** Template — why it could not verify: '{{host}}' did not answer. */
	whyUnreachable: string;
	/** The door to adding it unverified. */
	addUnverified: string;
	/** Template — what adding it unverified risks ('{{host}}', '{{chainId}}'). */
	unverifiedWarning: string;
	/** The explicit confirmation. */
	addUnverifiedConfirm: string;
	/** Template — '… (chain {{actual}}, expected {{expected}}).' */
	wrongRpc: string;
	noRpc: string;
	checksTitle: string;
	checkSigner: string;
	add: string;
	retry: string;
	setupTool: string;
	cancel: string;
	done: string;
}
