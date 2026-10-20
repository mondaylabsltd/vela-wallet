/**
 * Every capability the person has is an intent (spec 103) — kept true by
 * this ledger. It names everything the UI does to the wallet's machines: each
 * event a screen dispatches, each resident method a screen calls, and each
 * event of Settings' own vocabularies. A new one fails here until it is
 * placed:
 *
 * - `intent` — it commits that catalogued action (the agent can do it too);
 * - `step`   — one step of that action's own sequence;
 * - `view`   — what is shown or how, not what the wallet holds;
 * - `machine`— a machine's input (a tick, a fee, a boot), not the person's;
 * - `none`   — not a capability, and why;
 * - `later`  — a capability not yet offered to an agent, and why: the gaps,
 *              in one place, never silent.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, intentCatalog } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';

type Place =
	| { intent: string }
	| { step: string }
	| { view: string }
	| { machine: string }
	| { none: string }
	| { later: string };

const FILE = 'a file the person picks or saves on this device';
const DAPP = "a dApp's own request, answered by the person to that dApp";
const SHOWN = 'how the screen shows it';

/** `<resident>.dispatch({ type })` in the screens, by the resident the screen names. */
const DISPATCHED: Record<string, Place> = {
	// The send form: drafting is the intents' prepare, confirming their submit.
	'sendSession.select_token': { step: 'prepare_transfer' },
	'sendSession.change_token': { step: 'prepare_transfer' },
	'sendSession.set_recipient': { step: 'prepare_transfer' },
	'sendSession.picked_address': { step: 'prepare_transfer' },
	'sendSession.set_amount': { step: 'prepare_transfer' },
	'sendSession.tap_max': { step: 'prepare_transfer' },
	'sendSession.continue': { step: 'prepare_transfer' },
	'sendSession.enter_split_mode': { step: 'prepare_batch_transfer' },
	'sendSession.recipients_changed': { step: 'prepare_batch_transfer' },
	'sendSession.append_split_recipients': { step: 'prepare_batch_transfer' },
	'sendSession.slide_confirm': { intent: 'submit_transfer' },
	'sendSession.back': { step: 'cancel_transfer' },
	'sendSession.done': { view: 'the receipt closed' },
	'sendSession.choose_fee_token': { later: 'paying the fee in another token, per transfer' },
	'sendSession.toggle_multi_token': { later: 'several tokens to one address (sweep)' },
	'sendSession.set_multi_network': { later: 'several tokens to one address (sweep)' },
	'sendSession.confirm_multi_selection': { later: 'several tokens to one address (sweep)' },
	'sendSession.toggle_all_multi_tokens': { later: 'several tokens to one address (sweep)' },
	'sendSession.display_changed': { view: SHOWN },
	'sendSession.toggle_fiat_input': { view: 'typing the amount in fiat or in the token' },
	'sendSession.open_contact_picker': { view: 'a picker; the recipient is the step' },
	'sendSession.close_contact_picker': { view: 'a picker; the recipient is the step' },
	'sendSession.open_scanner': { none: 'the camera, which an agent has no use for' },
	'sendSession.close_scanner': { none: 'the camera, which an agent has no use for' },
	'sendSession.scan_resolved': { none: 'the camera, which an agent has no use for' },
	'sendSession.add_network_tapped': { view: 'goes to add a network (add_network)' },
	'sendSession.retry_after_bootstrap': { view: 'the review’s own retry' },
	'sendSession.retry_after_error': { view: 'the review’s own retry' },
	'sendSession.retry_relay_unreachable': { view: 'the review’s own retry' },
	'sendSession.dismiss_relay_unreachable': { view: 'a notice dismissed' },
	'sendSession.dismiss_treasury_sheet': { view: 'a notice dismissed' },
	'sendSession.fee_updated': { machine: 'the fee quote' },
	'sendSession.holdings_updated': { machine: 'the balances' },
	'sendSession.in_flight_ops': { machine: 'the chain tracker' },
	'sendSession.receipt_update': { machine: 'the chain tracker' },
	// Pasting a list into the split form: an agent gives the list itself.
	'batchSession.set_raw_text': { step: 'prepare_batch_transfer' },
	'batchSession.set_unit': { step: 'prepare_batch_transfer' },
	'batchSession.set_fiat_code': { step: 'prepare_batch_transfer' },
	'batchSession.edit_rate': { step: 'prepare_batch_transfer' },
	'batchSession.reset_rate_to_auto': { step: 'prepare_batch_transfer' },
	'batchSession.pick_file_requested': { none: FILE },
	'batchSession.save_template_requested': { none: FILE },
	'paySession.include_network_changed': { view: 'what the receive code carries' },
	// Adding a token: the sheet's own session.
	'manageTokens.address_input': { step: 'add_token' },
	'manageTokens.detect_requested': { step: 'add_token' },
	'manageTokens.save_requested': { intent: 'add_token' },
	// The network ledger.
	'networkAdmin.wizard_reset': { step: 'add_network' },
	'networkAdmin.search_input': { step: 'add_network' },
	'networkAdmin.chain_selected': { step: 'add_network' },
	'networkAdmin.custom_rpc_edited': { step: 'add_network' },
	'networkAdmin.delete_confirmed': { intent: 'remove_network' },
	'networkAdmin.override_expanded': { step: 'set_network_endpoint' },
	'networkAdmin.override_field_edited': { step: 'set_network_endpoint' },
	// The person's own field editing: the network core cleans and saves what
	// was typed. The intents' input check is an agent's (a blank service URL,
	// a key with spaces), so these stay the core's events.
	'networkAdmin.override_blurred': { intent: 'set_network_endpoint' },
	'networkAdmin.providers_opened': { step: 'set_provider_key' },
	'networkAdmin.provider_key_edited': { step: 'set_provider_key' },
	'networkAdmin.provider_key_blurred': { intent: 'set_provider_key' },
	'networkAdmin.provider_test_requested': {
		later: 'testing a provider key; its answer is only drawn in Settings'
	},
	'networkAdmin.endpoints_opened': { step: 'set_service_endpoint' },
	'networkAdmin.endpoint_edited': { step: 'set_service_endpoint' },
	'networkAdmin.endpoint_blurred': { intent: 'set_service_endpoint' },
	'networkAdmin.dapp_add_requested': { none: DAPP },
	'networkAdmin.dapp_add_approved': { none: DAPP },
	'networkAdmin.dapp_add_declined': { none: DAPP },
	'networkAdmin.dapp_add_cancelled': { none: DAPP },
	'networkAdmin.dapp_add_retried': { none: DAPP },
	// The address book.
	'contacts.inspect_recipient': { step: 'show' },
	'contacts.import_file': { none: FILE },
	'contacts.export_requested': { none: FILE },
	'contacts.export_taken': { none: FILE },
	'contacts.import_acknowledged': { none: FILE },
	'contactsBook.history_changed': { machine: 'the send history' },
	// A dApp's signing request: the person answers it, the passkey signs.
	'signRequest.approve_tapped': { later: "answering a dApp's signing request" },
	'signRequest.dismiss_tapped': { later: "answering a dApp's signing request" },
	'signRequest.retry_tapped': { view: 'the request’s own retry' },
	'signRequest.request_arrived': { machine: 'a dApp’s request arriving' },
	'signRequest.sim_started': { machine: 'the simulation' },
	'signRequest.sim_settled': { machine: 'the simulation' },
	'signRequest.transport_dropped': { machine: 'the dApp’s connection' },
	'login.sign_in': { later: 'signing in or creating a wallet: the passkey’s own ceremony' }
};

/** Resident methods the screens call. */
const CALLED: Record<string, Place> = {
	'session.signOut': { intent: 'sign_out' },
	'session.confirmSignOut': { step: 'sign_out' },
	'session.dismissSignOut': { step: 'sign_out' },
	'session.chooseVenue': {
		none: 'where this account signs is the signing boundary itself (spec 102)'
	},
	'session.resetLocalCopy': {
		later: 'signing in or creating a wallet: the passkey’s own ceremony'
	},
	'session.boot': { machine: 'boot' },
	'session.start': { machine: 'boot' },
	'session.settled': { machine: 'a read' },
	'session.accountEstablished': { machine: 'the account the pages were given' },
	'balance.refresh': { intent: 'refresh_balances' },
	'balance.reread': { intent: 'refresh_balances' },
	'balance.boot': { machine: 'boot' },
	'balance.setAccount': { machine: 'the account' },
	'balance.settledFor': { machine: 'a read' },
	'balance.openSwitcher': { view: 'the account switcher shown' },
	'balance.closeSwitcher': { view: 'the account switcher shown' },
	'balance.unreachableListOpened': { view: 'a list shown' },
	'balance.unreachableListClosed': { view: 'a list shown' },
	'balance.fixChainResolved': { view: 'a network’s fix shown' },
	'balance.focused': { machine: 'the tab’s focus' },
	'balance.backgrounded': { machine: 'the tab’s focus' },
	// `balance.startsWith` / `balance.slice` are a string named `balance`.
	'balance.startsWith': { machine: 'not the resident' },
	'balance.slice': { machine: 'not the resident' },
	'feed.focusTick': { step: 'refresh_balances' },
	'feed.liveTick': { machine: 'a tick' },
	'feed.boot': { machine: 'boot' },
	'feed.setAccount': { machine: 'the account' },
	'feed.privacyChanged': { step: 'set_balance_privacy' },
	'feed.chainFilter': { view: 'the network filter' },
	'feed.contactFilter': { view: 'a contact’s page shown' },
	'chainFilter.select': { view: 'the network filter' },
	'contactsBook.setAccount': { machine: 'the account' },
	'contactsBook.dispatch': { machine: 'its events are placed above' },
	'networkAdmin.dispatch': { machine: 'its events are placed above' },
	'networkAdmin.boot': { machine: 'boot' },
	'signRequest.dispatch': { machine: 'its events are placed above' },
	'signRequest.boot': { machine: 'boot' },
	'signRequest.syncNetworks': { machine: 'the networks' },
	'signRequest.syncAccounts': { machine: 'the accounts' },
	'signRequest.registerTransport': { machine: 'the dApp’s connection' },
	'currency.boot': { machine: 'boot' },
	'preferences.boot': { machine: 'boot' },
	'feeTierPreference.boot': { machine: 'boot' },
	'agentSettings.boot': { machine: 'boot' },
	'analyticsConsent.boot': { machine: 'boot' },
	'identiconViewer.open': { view: 'an avatar enlarged' },
	'identiconViewer.close': { view: 'an avatar enlarged' },
	'reportSend.send': { later: 'sending a feedback report' },
	'reportSend.surface': { view: 'the report form shown' },
	'reportSend.done': { view: 'the report form closed' }
};

/** Settings' own vocabularies, which its route turns into the calls above. */
const SETTINGS: Record<string, Place> = {
	'net.open-add': { step: 'add_network' },
	'net.search': { step: 'add_network' },
	'net.pick-suggestion': { step: 'add_network' },
	'net.custom-rpc': { step: 'add_network' },
	'net.recheck': { step: 'add_network' },
	'net.confirm-add': { intent: 'add_network' },
	'net.delete-network': { intent: 'remove_network' },
	'net.select-network': { view: 'one network’s page shown' },
	'net.detail-field': { step: 'set_network_endpoint' },
	'net.detail-blur': { intent: 'set_network_endpoint' },
	'net.providers-open': { step: 'set_provider_key' },
	'net.provider-key': { step: 'set_provider_key' },
	'net.provider-blur': { intent: 'set_provider_key' },
	'net.provider-test': { later: 'testing a provider key; its answer is only drawn in Settings' },
	'net.endpoints-open': { step: 'set_service_endpoint' },
	'net.endpoint': { step: 'set_service_endpoint' },
	'net.endpoint-blur': { intent: 'set_service_endpoint' },
	'net.endpoints-reset': { intent: 'reset_service_endpoints' },
	'pref.theme': { intent: 'set_preference' },
	'pref.language': { intent: 'set_preference' },
	'pref.text-scale': { intent: 'set_preference' },
	'pref.number-format': { intent: 'set_preference' },
	'pref.date-format': { intent: 'set_preference' },
	'pref.time-format': { intent: 'set_preference' },
	'pref.currency': { intent: 'set_preference' },
	'pref.fee-speed': { intent: 'set_preference' },
	'pref.analytics': { intent: 'set_preference' },
	'pref.erase': { intent: 'erase_device' },
	'pref.agent': { intent: 'configure_agent' },
	'pref.agent-url': { intent: 'configure_agent' },
	'pref.agent-token': { intent: 'configure_agent' },
	'pref.agent-grant': { intent: 'set_agent_grant' },
	'pref.signing-venue': {
		none: 'where this account signs is the signing boundary itself (spec 102)'
	}
};

/**
 * What the person's own taps ask through the intents (spec 103 D3): the same
 * way in as an agent's, by the same action. A screen asking for one not here
 * fails, as does one here no screen asks for any more.
 */
const PERSON = [
	'add_network',
	'clear_caches',
	'clear_storage_item',
	'configure_agent',
	'delete_activity',
	'delete_contact',
	'delete_group',
	'erase_device',
	'refresh_balances',
	'remove_network',
	'remove_wallet',
	'reset_service_endpoints',
	'save_contact',
	'save_group',
	'set_agent_grant',
	'set_balance_privacy',
	'set_contact_groups',
	'set_group_members',
	'set_preference',
	'sign_out',
	'submit_transfer',
	'switch_account'
];

const ROOT = join(__dirname, '../..');
const RESIDENTS =
	'session|balance|feed|contactsBook|networkAdmin|chainFilter|preferences|currency|feeTierPreference|analyticsConsent|agentSettings|identiconViewer|reportSend|signRequest';

function screens(dir: string, found: string[] = []): string[] {
	for (const name of readdirSync(dir)) {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) screens(path, found);
		else if (name.endsWith('.svelte')) found.push(path);
	}
	return found;
}

function scan(pattern: RegExp, key: (match: RegExpExecArray) => string): Map<string, string> {
	const seen = new Map<string, string>();
	for (const file of screens(ROOT)) {
		const source = readFileSync(file, 'utf8');
		for (const match of source.matchAll(pattern)) {
			const name = key(match as RegExpExecArray);
			if (!seen.has(name)) seen.set(name, relative(ROOT, file));
		}
	}
	return seen;
}

function kinds(file: string, prefix: string): string[] {
	const source = readFileSync(join(ROOT, 'lib/settings', file), 'utf8');
	return [...new Set([...source.matchAll(/kind: '([a-z-]+)'/g)].map((m) => `${prefix}.${m[1]}`))];
}

let catalog: Set<string>;
beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	catalog = new Set(
		(JSON.parse(intentCatalog()) as { action: string }[]).map((entry) => entry.action)
	);
});

function check(ledger: Record<string, Place>, found: Iterable<string>, where = new Map()) {
	const unplaced = [...found].filter((name) => !(name in ledger));
	expect(
		unplaced.map((name) => `${name} (${where.get(name) ?? 'settings'})`),
		'place each in this ledger'
	).toEqual([]);
}

describe('every capability the person has is an intent (spec 103)', () => {
	it('places every event a screen dispatches', () => {
		const found = scan(
			/\b([A-Za-z]+)\??\.dispatch\(\{\s*type: '([a-z_]+)'/g,
			(m) => `${m[1]}.${m[2]}`
		);
		check(DISPATCHED, found.keys(), found);
		const stale = Object.keys(DISPATCHED).filter((name) => !found.has(name));
		expect(stale, 'no longer dispatched: drop them').toEqual([]);
	});

	it('places every resident method a screen calls', () => {
		const found = scan(
			new RegExp(`\\b(${RESIDENTS})\\.([a-z][A-Za-z]+)\\(`, 'g'),
			(m) => `${m[1]}.${m[2]}`
		);
		check(CALLED, found.keys(), found);
	});

	it('places every event of Settings’ own vocabularies', () => {
		check(SETTINGS, [...kinds('net-events.ts', 'net'), ...kinds('pref-events.ts', 'pref')]);
	});

	it('lists what the person’s taps ask through the intents', () => {
		const found = scan(/intents\.invoke\(\s*'person',\s*\{\s*type: '([a-z_]+)'/g, (m) => m[1]);
		expect([...found.keys()].sort()).toEqual(PERSON);
		expect(PERSON.filter((action) => !catalog.has(action))).toEqual([]);
	});

	it('names only actions the catalog has', () => {
		const named = [DISPATCHED, CALLED, SETTINGS].flatMap((ledger) =>
			Object.values(ledger).flatMap((place) =>
				'intent' in place ? [place.intent] : 'step' in place ? [place.step] : []
			)
		);
		expect(named.filter((action) => !catalog.has(action))).toEqual([]);
	});
});
