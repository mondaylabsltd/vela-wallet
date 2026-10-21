/**
 * The shell's half of an intent's rules (spec 103): what the shared check in
 * the core cannot know because it holds no state — whether what an action
 * names exists, whether it is already so, and whether the change it asked
 * for has been SEEN. Pure functions over a {@link World}.
 */
import type { ContactsView } from '$lib/core/generated/ContactsView';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { IntentAction } from '$lib/core/generated/IntentAction';
import type { IntentPreference } from '$lib/core/generated/IntentPreference';
import type { IntentReason } from '$lib/core/generated/IntentReason';
import type { IntentReasonCode } from '$lib/core/generated/IntentReasonCode';
import type { NetView } from '$lib/core/generated/NetView';
import type { SessionView } from '$lib/session/generated/SessionView';
import type { World } from './world';

export type Examined =
	| { kind: 'go' }
	/** Already true: nothing to do, and nothing to approve. */
	| { kind: 'already' }
	| { kind: 'refused'; reason: IntentReason };

/** What was true when the call began, to tell its change from others'. */
export type Baseline = {
	/** Groups already carrying the name a new group is to have. */
	sameNamedGroups: number;
	/** The session's sign-out sheet was up when the call began. */
	sheetUp: boolean;
};

export type Verdict =
	{ kind: 'held' } | { kind: 'not_yet' } | { kind: 'refused'; reason: IntentReason };

const refused = (code: IntentReasonCode, detail: string): Examined => ({
	kind: 'refused',
	reason: { code, detail }
});

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

function account(session: SessionView, address: string) {
	return session.accounts.find((row) => same(row.account.address, address));
}

function contact(book: ContactsView, address: string) {
	return book.contacts.find((entry) => same(entry.address, address));
}

function group(book: ContactsView, id: string) {
	return book.groups.find((entry) => entry.id === id);
}

function readySession(world: World): SessionView | null {
	return world.session && !world.session.loading ? world.session : null;
}

function readBook(world: World): ContactsView | null {
	return world.contacts?.loaded ? world.contacts : null;
}

const notReady = (what: string) => refused('not_ready', `${what} is not loaded yet`);

function network(networks: NetView, chainId: number) {
	return networks.networks.find((row) => row.chain_id === chainId);
}

/** What one network's card shows for `field` now (its draft while open). */
function networkField(networks: NetView, chainId: number, field: 'rpc' | 'explorer') {
	const row = network(networks, chainId);
	return row === undefined ? undefined : field === 'rpc' ? row.rpc_url : row.explorer_url;
}

/** The records a feed row's id names: its own, or every one of a batch by its operation's hash. */
function recordsNamed(feed: FeedView, id: string): number {
	return feed.transactions.filter((tx) => tx.id === id || same(tx.user_op_hash, id)).length;
}

function signedIn(world: World): Examined {
	const session = readySession(world);
	if (!session) return notReady('the session');
	return session.has_wallet ? { kind: 'go' } : refused('not_ready', 'no wallet is signed in');
}

export function preferenceHolds(world: World, preference: IntentPreference): boolean {
	const prefs = world.preferences;
	switch (preference.kind) {
		case 'currency':
			return world.currency?.code === preference.value;
		case 'fee_speed':
			return world.feeTier?.committed === true && world.feeTier.tier === preference.value;
		case 'theme':
			return prefs.theme === preference.value;
		case 'language':
			return prefs.language === preference.value;
		case 'text_scale':
			return prefs.textScale === preference.value;
		case 'number_format':
			return prefs.numberFormat === preference.value;
		case 'date_format':
			return prefs.dateFormat === preference.value;
		case 'time_format':
			return prefs.timeFormat === preference.value;
		case 'analytics':
			return prefs.analytics === preference.value;
	}
}

/** Does what the action names exist, and is it already so? */
export function examine(action: IntentAction, world: World): Examined {
	switch (action.type) {
		case 'show': {
			const screen = action.screen;
			if (screen.type === 'asset') {
				if (!world.balance) return notReady('the balances');
				const held = world.balance.tokens.some(
					(token) =>
						!token.spam &&
						token.chain_id === screen.chain_id &&
						(token.token_address ?? '').toLowerCase() === (screen.token ?? '').toLowerCase()
				);
				return held ? { kind: 'go' } : refused('not_found', 'the wallet does not hold that token');
			}
			if (screen.type === 'activity') {
				if (!world.feed) return notReady('the activity');
				return world.feed.transactions.some((tx) => tx.id === screen.id)
					? { kind: 'go' }
					: refused('not_found', 'no activity record has that id');
			}
			if (screen.type !== 'contact' && screen.type !== 'group') return { kind: 'go' };
			const book = readBook(world);
			if (!book) return notReady('the address book');
			if (screen.type === 'contact' && !contact(book, screen.address))
				return refused('not_found', 'no contact has that address');
			if (screen.type === 'group' && !group(book, screen.id))
				return refused('not_found', 'no group has that id');
			return { kind: 'go' };
		}
		case 'switch_account':
		case 'remove_wallet': {
			const session = readySession(world);
			if (!session) return notReady('the session');
			if (!account(session, action.address))
				return refused('not_found', 'no wallet on this device has that address');
			if (action.type === 'switch_account' && same(session.address, action.address))
				return { kind: 'already' };
			return { kind: 'go' };
		}
		case 'sign_out': {
			const session = readySession(world);
			if (!session) return notReady('the session');
			return session.has_wallet ? { kind: 'go' } : { kind: 'already' };
		}
		case 'set_preference':
			return preferenceHolds(world, action.preference) ? { kind: 'already' } : { kind: 'go' };
		case 'save_contact': {
			const book = readBook(world);
			if (!book) return notReady('the address book');
			const existing = contact(book, action.address);
			return existing?.name === action.name.trim() ? { kind: 'already' } : { kind: 'go' };
		}
		case 'delete_contact': {
			const book = readBook(world);
			if (!book) return notReady('the address book');
			return contact(book, action.address)
				? { kind: 'go' }
				: refused('not_found', 'no contact has that address');
		}
		case 'save_group': {
			const book = readBook(world);
			if (!book) return notReady('the address book');
			if (action.id === null) return { kind: 'go' };
			const existing = group(book, action.id);
			if (!existing) return refused('not_found', 'no group has that id');
			return existing.name === action.name.trim() ? { kind: 'already' } : { kind: 'go' };
		}
		case 'delete_group': {
			const book = readBook(world);
			if (!book) return notReady('the address book');
			return group(book, action.id) ? { kind: 'go' } : refused('not_found', 'no group has that id');
		}
		case 'set_group_members': {
			const book = readBook(world);
			if (!book) return notReady('the address book');
			if (!group(book, action.id)) return refused('not_found', 'no group has that id');
			const stranger = action.members.find((member) => !contact(book, member));
			return stranger === undefined
				? { kind: 'go' }
				: refused('not_found', `${stranger} is not a saved contact`);
		}
		case 'set_contact_groups': {
			const book = readBook(world);
			if (!book) return notReady('the address book');
			if (!contact(book, action.address))
				return refused('not_found', 'no contact has that address');
			const unknown = action.group_ids.find((id) => !group(book, id));
			return unknown === undefined
				? { kind: 'go' }
				: refused('not_found', `no group has the id ${unknown}`);
		}
		case 'erase_device':
		case 'clear_storage_item':
		case 'clear_caches':
		case 'configure_agent':
			return { kind: 'go' };
		case 'prepare_transfer':
		case 'prepare_batch_transfer':
		case 'add_token':
		case 'refresh_balances':
			return signedIn(world);
		case 'submit_transfer':
		case 'cancel_transfer': {
			const operation = world.operations.find((entry) => entry.id === action.operation);
			if (!operation) return refused('not_found', 'no transfer has that operation id');
			if (action.type === 'cancel_transfer' && operation.state === 'cancelled')
				return { kind: 'already' };
			// A review that failed before anything was sent can still be left.
			if (
				action.type === 'cancel_transfer' &&
				operation.state === 'failed' &&
				operation.user_op_hash === null
			)
				return { kind: 'go' };
			return operation.state === 'awaiting_confirmation'
				? { kind: 'go' }
				: refused('refused', `that transfer is ${operation.state}, not on its review`);
		}
		case 'delete_activity':
			if (!world.feed) return notReady('the activity');
			return recordsNamed(world.feed, action.id) > 0
				? { kind: 'go' }
				: refused('not_found', 'no activity record has that id');
		case 'set_balance_privacy':
			if (!world.balance) return notReady('the balances');
			return world.balance.hidden === action.hidden ? { kind: 'already' } : { kind: 'go' };
		case 'set_contact_favorite': {
			const book = readBook(world);
			if (!book) return notReady('the address book');
			const existing = contact(book, action.address);
			if (!existing) return refused('not_found', 'no contact has that address');
			return existing.favorite === action.favorite ? { kind: 'already' } : { kind: 'go' };
		}
		case 'add_network':
			if (!world.networks) return notReady('the network list');
			return network(world.networks, action.chain_id) ? { kind: 'already' } : { kind: 'go' };
		case 'set_network_endpoint': {
			if (!world.networks) return notReady('the network list');
			const now = networkField(world.networks, action.chain_id, action.field);
			if (now === undefined)
				return refused('not_found', 'the wallet has no network with that chain id');
			return now === action.url.trim() ? { kind: 'already' } : { kind: 'go' };
		}
		case 'remove_network': {
			if (!world.networks) return notReady('the network list');
			const row = network(world.networks, action.chain_id);
			if (!row) return refused('not_found', 'the wallet has no network with that chain id');
			return row.is_custom
				? { kind: 'go' }
				: refused('refused', 'a network the wallet ships with cannot be removed');
		}
		case 'set_provider_key':
		case 'set_service_endpoint':
		case 'reset_service_endpoints':
			return world.networks ? { kind: 'go' } : notReady('the network settings');
	}
}

export function baselineOf(action: IntentAction, world: World): Baseline {
	const name = action.type === 'save_group' && action.id === null ? action.name.trim() : null;
	return {
		sameNamedGroups:
			name === null ? 0 : (world.contacts?.groups.filter((g) => g.name === name).length ?? 0),
		sheetUp: world.session?.sign_out !== null && world.session?.sign_out !== undefined
	};
}

/**
 * Has the change held? `baseline` is updated as the call goes on — the
 * sign-out sheet coming up is part of the call, not its end.
 */
export function verdict(action: IntentAction, world: World, baseline: Baseline): Verdict {
	const held = (yes: boolean): Verdict => (yes ? { kind: 'held' } : { kind: 'not_yet' });
	const book = readBook(world);
	switch (action.type) {
		// Shown, cleared, erased, configured: the shell's own word is the change.
		case 'show':
		case 'erase_device':
		case 'clear_storage_item':
		case 'clear_caches':
		case 'configure_agent':
			return { kind: 'held' };
		case 'switch_account':
			return held(world.session !== null && same(world.session.address, action.address));
		case 'remove_wallet':
			return held(world.session !== null && !account(world.session, action.address));
		case 'sign_out': {
			const session = world.session;
			if (!session) return { kind: 'not_yet' };
			if (session.sign_out) baseline.sheetUp = true;
			if (!session.has_wallet || session.allowed_route === 'onboarding') return { kind: 'held' };
			if (baseline.sheetUp && !session.sign_out)
				return {
					kind: 'refused',
					reason: { code: 'denied', detail: 'the person kept the wallet signed in' }
				};
			return { kind: 'not_yet' };
		}
		case 'set_preference':
			return held(preferenceHolds(world, action.preference));
		case 'save_contact':
			return held(book !== null && contact(book, action.address)?.name === action.name.trim());
		case 'delete_contact':
			return held(book !== null && !contact(book, action.address));
		case 'save_group': {
			if (!book) return { kind: 'not_yet' };
			const name = action.name.trim();
			if (action.id !== null) return held(group(book, action.id)?.name === name);
			return held(book.groups.filter((g) => g.name === name).length > baseline.sameNamedGroups);
		}
		case 'delete_group':
			return held(book !== null && !group(book, action.id));
		case 'set_group_members': {
			const members = book ? group(book, action.id)?.members : undefined;
			return held(
				members !== undefined &&
					members.length === action.members.length &&
					action.members.every((wanted) => members.some((have) => same(have.address, wanted)))
			);
		}
		case 'set_contact_groups':
			return held(
				book !== null &&
					book.groups.every(
						(entry) =>
							action.group_ids.includes(entry.id) ===
							entry.members.some((member) => same(member.address, action.address))
					)
			);
		// Drafting and confirming answer with their operation (`accepted`);
		// what happens next is the operation's, followed by its own state.
		case 'prepare_transfer':
		case 'submit_transfer':
			return { kind: 'held' };
		case 'cancel_transfer':
			return held(
				world.operations.find((entry) => entry.id === action.operation)?.state === 'cancelled'
			);
		case 'prepare_batch_transfer':
			return { kind: 'held' };
		// Found and stored is the token core's own view, read before it answered.
		case 'add_token':
		case 'refresh_balances':
			return { kind: 'held' };
		case 'delete_activity':
			return held(world.feed !== null && recordsNamed(world.feed, action.id) === 0);
		case 'set_balance_privacy':
			return held(world.balance?.hidden === action.hidden);
		case 'set_contact_favorite':
			return held(book !== null && contact(book, action.address)?.favorite === action.favorite);
		case 'add_network': {
			const networks = world.networks;
			if (!networks) return { kind: 'not_yet' };
			if (networks.wizard.error)
				return {
					kind: 'refused',
					reason: { code: 'refused', detail: `add network: ${networks.wizard.error.type}` }
				};
			return held(network(networks, action.chain_id)?.is_custom === true);
		}
		case 'set_network_endpoint': {
			const networks = world.networks;
			const row = networks ? network(networks, action.chain_id) : undefined;
			if (!networks || !row) return { kind: 'not_yet' };
			// The save gate: an RPC answering for another chain is not written.
			if (action.field === 'rpc' && row.rpc_chain_mismatch)
				return {
					kind: 'refused',
					reason: { code: 'refused', detail: 'that RPC answers for another chain' }
				};
			if (action.field === 'rpc' && row.rpc_save_deferred) return { kind: 'not_yet' };
			return held(networkField(networks, action.chain_id, action.field) === action.url.trim());
		}
		case 'remove_network':
			return held(
				world.networks !== null && network(world.networks, action.chain_id)?.is_custom !== true
			);
		case 'set_provider_key': {
			const row = world.networks?.providers.find((entry) => entry.provider === action.provider);
			const key = action.key.trim();
			return held(row !== undefined && row.key === key && row.has_key === (key !== ''));
		}
		case 'set_service_endpoint':
			return held(
				world.networks?.endpoints.find((entry) => entry.field === action.field)?.value ===
					action.url.trim()
			);
		case 'reset_service_endpoints':
			return held(
				world.networks !== null &&
					world.networks.endpoints.every(
						(entry) => entry.value === '' || entry.value === entry.default_value
					)
			);
	}
}
