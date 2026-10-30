/**
 * A dApp's request to a multisig, carried between the request's surface
 * (`DappRequestHost`) and the proposals machine, which holds it
 * (`dapp_request` → `view.dapp[rid]`; docs/multisig-vault-design.md §6).
 *
 * The machine decides everything — refused (a message, dapp-7), read
 * (`dapp_rpc::multisig_calls`) and drafted with its review, proposed on the
 * person's word, held for its execution, told "proposed" before the
 * request's limit, answered once whichever way it is left. This only:
 *
 * - draws it: the multisig's sheet opens on the held request and follows it
 *   (`multisigUi.held`), kept as last seen once it is answered — a sheet on
 *   an executed transaction, or one whose site was told, stays;
 * - answers the site with the machine's answer, exactly once, then tells
 *   the machine (`dapp_settled`); an error the site was not told of takes
 *   the sheet down with it;
 * - passes on the person's way out (`leave` → `dapp_leave`).
 */
import type { MultisigDappAnswer } from '../generated/MultisigDappAnswer';
import type { MultisigDappError } from '../generated/MultisigDappError';
import type { MultisigHeldRequest } from '../generated/MultisigHeldRequest';
import type { MultisigProposalsView } from '../generated/MultisigProposalsView';
import type { MultisigAccount } from '../core/store.svelte';
import { multisigUi } from '../core/ui.svelte';
import { proposalsHost } from './proposals-host.svelte';

/** What the surface hands over with the request. */
export interface DappHandOff {
	rid: string;
	/** The multisig the site was granted (shown or not). */
	account: MultisigAccount;
	method: string;
	params: unknown;
	/** The site, as the browser names it. */
	origin: string;
	/** The chain the site is on (`0`: none — the machine answers that). */
	chainId: number;
	/** How long from the claim before the passkey the request can still be answered. */
	limitMs?: number;
	/** RB5: is the request still live? Asked before the passkey. */
	claim(): Promise<boolean>;
	/** dapp-7: a method a multisig cannot do — said first; answered on {@link DappHold.leave}. */
	refused(refusal: MultisigDappError): void;
	/** Answer the site (called once, with the request as the machine last held it). */
	answer(answer: MultisigDappAnswer, held: MultisigHeldRequest): void | Promise<void>;
}

/** A request held by the machine, from the surface's side. */
export interface DappHold {
	/** The person left it (declined, closed, read the refusal) or its page did: answered as it stands. */
	leave(): void;
	/**
	 * The surface is going away now: the machine's answer for leaving it,
	 * at once (`null`: already answered, or never held).
	 */
	closing(): MultisigDappAnswer | null;
	/** Stop following it (nothing answered). */
	stop(): void;
}

/** The machine's host, as this reads it (a test's stand-in). */
export interface DappHost {
	boot(): Promise<void>;
	readonly view: MultisigProposalsView | null;
	send(event: { type: string; [key: string]: unknown }): void;
	listen(onview: (view: MultisigProposalsView) => void): () => void;
}

const host: DappHost = {
	boot: () => proposalsHost.boot(),
	get view() {
		return proposalsHost.view;
	},
	send: (event) => proposalsHost.send(event as never),
	listen: (onview) => proposalsHost.machine?.listen(onview) ?? (() => {})
};

/** Hand `h` to the machine: drawn, held and answered by it. */
export async function holdDappRequest(h: DappHandOff, machine: DappHost = host): Promise<DappHold> {
	await machine.boot();
	let done = false;
	let shown = false;
	let said = false;
	const ask = () =>
		machine.send({
			type: 'dapp_request',
			id: h.rid,
			account: $state.snapshot(h.account),
			method: h.method,
			params: h.params,
			origin: h.origin,
			chain_id: Math.max(0, h.chainId),
			limit_ms: h.limitMs ?? null
		});
	const settle = (held: MultisigHeldRequest, answer: MultisigDappAnswer) => {
		done = true;
		stop();
		// An executed transaction's sheet, or one whose site was told, stays on it.
		if (answer.error && !held.told) multisigUi.closeDapp(h.rid);
		void h.answer(answer, held);
		// Told after this view is drawn: the machine forgets it.
		queueMicrotask(() => machine.send({ type: 'dapp_settled', id: h.rid }));
	};
	const on = (view: MultisigProposalsView) => {
		const held = view.dapp?.[h.rid];
		if (done || !held) return;
		if (held.stage === 'refused' && !held.answer) {
			if (!said && held.refusal) {
				said = true;
				h.refused(held.refusal);
			}
			return;
		}
		if (!shown && !held.answer) {
			shown = true;
			multisigUi.open({
				kind: 'dapp',
				request: { rid: h.rid, held, claim: h.claim, retry: ask }
			});
		} else multisigUi.held(h.rid, held);
		if (held.answer) settle(held, held.answer);
	};
	const stop = machine.listen(on);
	ask();
	if (machine.view) on(machine.view);
	return {
		leave() {
			if (!done) machine.send({ type: 'dapp_leave', id: h.rid });
		},
		closing() {
			if (done) return null;
			machine.send({ type: 'dapp_leave', id: h.rid });
			return machine.view?.dapp?.[h.rid]?.answer ?? null;
		},
		stop
	};
}
