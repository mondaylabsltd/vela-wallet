/**
 * The send's stop on a request it cannot take up (PR 3 final notes F6/F27),
 * through the REAL send core: a locked request for a network this wallet
 * does not have stops the send, "Add this network" asks the shell to add it,
 * and what came of it is said — in the corpus's words, in every language.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { SendCore } from '$lib/core/client';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendOperation } from '$lib/core/generated/SendOperation';
import type { SendShellResult } from '$lib/core/generated/SendShellResult';
import type { SendView } from '$lib/core/generated/SendView';
import { rawResolve, resolveWalletFlowMessages } from '$lib/i18n/engine.server';
import { SUPPORTED_LOCALES } from '$lib/i18n/locales';
import { liveSendLock } from './live-send-lock';

const m = resolveWalletFlowMessages('en');
const ACCOUNT = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
const UNKNOWN_CHAIN = 59144;

type Out = { view: SendView; effects: { id: number; operation: SendOperation }[] };

/** A send opened on a locked request for `UNKNOWN_CHAIN`, its boot answered: Ethereum is all the wallet has. */
function lockedSend() {
	const core = new SendCore();
	const pending: Out['effects'] = [];
	let view!: SendView;
	const take = (out: Out) => {
		view = out.view;
		pending.push(...out.effects);
	};
	const dispatch = (event: SendEvent) =>
		take(JSON.parse(core.dispatch(JSON.stringify(event))) as Out);
	const resolve = (type: SendOperation['type'], result: SendShellResult) => {
		const at = pending.findIndex((effect) => effect.operation.type === type);
		if (at < 0) throw new Error(`the core asked for no ${type}`);
		const [effect] = pending.splice(at, 1);
		take(JSON.parse(core.resolve_effect(BigInt(effect.id), JSON.stringify(result))) as Out);
		return effect.operation;
	};
	dispatch({
		type: 'open',
		account: { id: 'cred-1', address: ACCOUNT, name: 'Wallet' },
		params: {
			preselected_symbol: null,
			preselected_network: null,
			prefilled_recipient: '0x' + 'ab'.repeat(20),
			prefilled_chain_id: String(UNKNOWN_CHAIN),
			prefilled_token_address: null,
			prefilled_amount_base: null,
			locked: true,
			preselected_multi: null
		},
		display: { code: 'USD', rate: 1, fiat_decimals: 2 }
	});
	resolve('fetch_tokens', {
		type: 'tokens_loaded',
		tokens: [],
		chains: [{ chain_id: 1, network: 'ethereum', native_symbol: 'ETH' }]
	});
	return { view: () => view, dispatch, resolve, pending, free: () => core.free() };
}

describe('a request for a network the wallet does not have', () => {
	it('stops the send and says so, with the one way on', () => {
		const send = lockedSend();
		expect(send.view().stage).toBe('lock_error');
		expect(send.view().lock_error).toEqual({ type: 'network', chain_id: UNKNOWN_CHAIN });
		expect(liveSendLock(send.view(), m)).toEqual({
			title: 'Network not supported',
			body: "This payment request is on a network Vela doesn't support yet (chain 59144).",
			add: { label: 'Add this network', chainId: UNKNOWN_CHAIN, busy: false }
		});
		send.free();
	});

	it('"Add this network" asks the shell for exactly that chain, and is busy until it answers', () => {
		const send = lockedSend();
		send.dispatch({ type: 'add_network_tapped', chain_id: UNKNOWN_CHAIN });
		expect(send.pending.map((effect) => effect.operation)).toContainEqual({
			type: 'add_network',
			chain_id: UNKNOWN_CHAIN
		});
		expect(send.view().adding_network).toBe(true);
		const busy = liveSendLock(send.view(), m);
		expect(busy?.add).toEqual({ label: 'Add this network', chainId: UNKNOWN_CHAIN, busy: true });
		expect(busy?.failed).toBeUndefined();
		send.free();
	});

	it.each([
		['not_found', "We couldn't find that network."],
		['not_compatible', "That network isn't compatible with Vela smart accounts yet."],
		['error', "Couldn't add the network. Please try again."]
	] as const)(
		'an add that answers %s says so under the button, and may be tried again',
		(type, line) => {
			const send = lockedSend();
			send.dispatch({ type: 'add_network_tapped', chain_id: UNKNOWN_CHAIN });
			send.resolve('add_network', {
				type: 'network_added',
				outcome: type === 'not_compatible' ? { type, detail: null } : { type }
			} as SendShellResult);
			const model = liveSendLock(send.view(), m);
			expect(model?.failed).toBe(line);
			// Still the stop, and the button is live again.
			expect(send.view().stage).toBe('lock_error');
			expect(model?.add?.busy).toBe(false);
			send.free();
		}
	);

	it('an add that worked reads the wallet again: the stop is the core’s to lift', () => {
		const send = lockedSend();
		send.dispatch({ type: 'add_network_tapped', chain_id: UNKNOWN_CHAIN });
		send.resolve('add_network', { type: 'network_added', outcome: { type: 'added' } });
		// The core boots again — it asks for the tokens, with the new chain's.
		expect(send.pending.some((effect) => effect.operation.type === 'fetch_tokens')).toBe(true);
		expect(liveSendLock(send.view(), m)?.failed).toBeUndefined();
		send.free();
	});
});

describe('the stop’s words', () => {
	it('a token nobody can describe: said, with nothing to add', () => {
		expect(
			liveSendLock(
				{ lock_error: { type: 'token' }, adding_network: false, add_network_msg: null },
				m
			)
		).toEqual({
			title: 'Unknown token',
			body: "We couldn't recognize the token in this payment request on this network."
		});
	});

	it('no stop, no sheet', () => {
		expect(
			liveSendLock({ lock_error: null, adding_network: false, add_network_msg: null }, m)
		).toBeUndefined();
	});

	it.each(SUPPORTED_LOCALES)('are the corpus’s in %s', (locale) => {
		const words = resolveWalletFlowMessages(locale);
		const model = liveSendLock(
			{
				lock_error: { type: 'network', chain_id: 59144 },
				adding_network: false,
				add_network_msg: { type: 'net_add_error' }
			},
			words
		)!;
		expect(model.title).toBe(rawResolve(locale, 'send.lock.netTitle'));
		expect(model.body).toContain('59144');
		expect(model.body).not.toContain('{{');
		expect(model.add?.label).toBe(rawResolve(locale, 'send.lock.addNetwork'));
		expect(model.failed).toBe(rawResolve(locale, 'send.lock.netAddError'));
	});
});
