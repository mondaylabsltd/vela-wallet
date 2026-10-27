/**
 * Doing an intent (spec 103): the calls the UI itself makes — the same
 * resident, the same event, the same service — so a person's tap and an
 * agent's request reach the rule that owns them by one road.
 *
 * Resolves with why it was refused, or `null` once every call was made. Seeing
 * the change land is the caller's (`rules.ts` → `verdict`).
 */
import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { analyticsConsent } from '$lib/analytics/consent.svelte';
import { networkSpace } from '$lib/network-space/network-space.svelte';
import { agentSettings } from '$lib/agent/settings.svelte';
import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
import type { IntentAction } from '$lib/core/generated/IntentAction';
import type { IntentActor } from '$lib/core/generated/IntentActor';
import type { IntentPreference } from '$lib/core/generated/IntentPreference';
import type { IntentScreen } from '$lib/core/generated/IntentScreen';
import { revokeAll } from '$lib/dapp/connections';
import { packagedHref } from '$lib/extension/page-url';
import { localeOfPath, SUPPORTED_LOCALES, type Locale } from '$lib/i18n/locales';
import { clearAllCaches, clearStorageItem, type StorageItemId } from '$lib/services/device-storage';
import { eraseDeviceData } from '$lib/services/erase-device';
import {
	preferences,
	type DateFormatKey,
	type NumberFormatKey,
	type TextScaleLevel,
	type ThemeChoice,
	type TimeFormatKey
} from '$lib/services/preferences.svelte';
import { session } from '$lib/session/core/session.svelte';
import { currency } from '$lib/settings/core/currency.svelte';
import { feeTierPreference } from '$lib/settings/core/fee-tier.svelte';
import type { IntentOperation } from '$lib/core/generated/IntentOperation';
import type { IntentReason } from '$lib/core/generated/IntentReason';
import { screenDesk } from '$lib/flows/screen-desk.svelte';
import { balance } from '$lib/wallet/core/balance.svelte';
import { feed } from '$lib/wallet/core/feed.svelte';
import * as networks from './networks';
import { addToken } from './tokens';
import { transfers } from './transfers.svelte';
import type { World } from './world';

/**
 * What doing it came to: `null` (done — the change is for the caller to see),
 * why it was refused (in words, or with a code), or the operation it started.
 */
export type Performed =
	| null
	| string
	| IntentReason
	/** `closed`: the finished transfer whose receipt this draft closed. */
	| { operation: IntentOperation; closed: string | null };

const started = (result: IntentOperation | IntentReason): Performed =>
	'code' in result ? result : { operation: result, closed: transfers.closedFor(result.id) };

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/** The locale the page is in now. */
function currentLocale(): Locale {
	return localeOfPath(location.pathname) ?? 'en';
}

/** The session indexes its wallets its own way: read from it, by address. */
function indexOf(world: World, address: string): number | null {
	return world.session?.accounts.find((row) => same(row.account.address, address))?.index ?? null;
}

/** The route a screen is drawn on. */
function routeOf(
	screen: IntentScreen
): '/[locale]/wallet' | '/[locale]/contacts' | '/[locale]/settings' {
	switch (screen.type) {
		case 'home':
		case 'asset':
		case 'activity':
		case 'receive':
			return '/[locale]/wallet';
		case 'contacts':
		case 'contact':
		case 'group':
			return '/[locale]/contacts';
		case 'settings':
			return '/[locale]/settings';
	}
}

/**
 * Go to the screen's page, then open the screen there as its own row would —
 * the page's presenter (`screenDesk`) — unless the page itself is the screen.
 */
export async function show(screen: IntentScreen): Promise<string | null> {
	await goto(resolve(routeOf(screen), { locale: currentLocale() }));
	const whole =
		screen.type === 'home' ||
		screen.type === 'contacts' ||
		(screen.type === 'settings' && screen.page === null);
	return whole ? null : screenDesk.present(screen);
}

/** The language is part of the URL on the web: the page goes to its own locale. */
async function chooseLanguage(value: string): Promise<void> {
	preferences.setLanguage(value);
	const here = currentLocale();
	const target = value === 'auto' ? here : value;
	if (target === here || !SUPPORTED_LOCALES.includes(target as Locale)) return;
	const path = location.pathname.replace(/^\/[^/.]+/, `/${target}`);
	// eslint-disable-next-line svelte/no-navigation-without-resolve -- this very page, in another locale
	await goto(`${path}${location.search}`);
}

async function setPreference(preference: IntentPreference): Promise<void> {
	switch (preference.kind) {
		case 'theme':
			preferences.setTheme(preference.value as ThemeChoice);
			return;
		case 'language':
			await chooseLanguage(preference.value);
			return;
		case 'text_scale':
			preferences.setTextScale(preference.value as TextScaleLevel);
			return;
		case 'number_format':
			preferences.setNumberFormat(preference.value as NumberFormatKey);
			return;
		case 'date_format':
			preferences.setDateFormat(preference.value as DateFormatKey);
			return;
		case 'time_format':
			preferences.setTimeFormat(preference.value as TimeFormatKey);
			return;
		case 'currency':
			currency.choose(preference.value);
			return;
		case 'fee_speed':
			if (preference.value !== 'rapid') feeTierPreference.choose(preference.value);
			return;
		case 'analytics':
			analyticsConsent.set(preference.value);
			return;
	}
}

/** Keep the connection in step with what was just configured. */
async function syncAgent(): Promise<void> {
	const { agentBridge } = await import('$lib/agent/bridge.svelte');
	agentBridge.sync();
}

export async function perform(
	action: IntentAction,
	world: World,
	nowMs: number,
	actor: IntentActor = 'person'
): Promise<Performed> {
	switch (action.type) {
		case 'show':
			return show(action.screen);
		case 'switch_account':
		case 'remove_wallet': {
			const index = indexOf(world, action.address);
			if (index === null) return 'no wallet on this device has that address';
			if (action.type === 'switch_account') session.switchAccount(index);
			else session.removeAccount(index);
			return null;
		}
		case 'sign_out':
			session.signOut();
			return null;
		case 'set_preference':
			await setPreference(action.preference);
			return null;
		case 'save_contact':
			contactsBook.dispatch({
				type: 'save',
				input: {
					address: action.address.trim(),
					name: action.name.trim(),
					note: null,
					favorite: null,
					kind: null,
					resolved_name: null,
					resolved_source: null
				},
				now_ms: nowMs
			});
			return null;
		case 'delete_contact':
			contactsBook.dispatch({ type: 'delete', address: action.address, now_ms: nowMs });
			return null;
		case 'save_group':
			contactsBook.dispatch({
				type: 'group_save',
				input: { id: action.id, name: action.name.trim(), color: null, members: null }
			});
			return null;
		case 'delete_group':
			contactsBook.dispatch({ type: 'group_delete', id: action.id });
			return null;
		case 'set_group_members':
			contactsBook.dispatch({ type: 'set_group_members', id: action.id, members: action.members });
			return null;
		case 'set_contact_groups':
			contactsBook.dispatch({
				type: 'set_contact_groups',
				address: action.address,
				group_ids: action.group_ids
			});
			return null;
		case 'erase_device':
			// The sweep VERIFIES before it resolves: a rejection means data is
			// still here, and the person stays where they can see it said.
			try {
				await eraseDeviceData();
			} catch (error) {
				return error instanceof Error ? error.message : 'the erase did not finish';
			}
			// Nothing of this wallet is left: a full reload is the first run.
			location.assign(packagedHref(resolve('/[locale]', { locale: currentLocale() })));
			return null;
		case 'clear_storage_item':
			if (action.item === 'dapps') {
				await revokeAll();
				return null;
			}
			await clearStorageItem(action.item as Exclude<StorageItemId, 'dapps'>);
			// The address book is resident: it reads the cleared stores again,
			// or it would show — and re-save — what was just removed.
			if (action.item === 'contacts') await contactsBook.reload();
			return null;
		case 'clear_caches':
			await clearAllCaches();
			return null;
		case 'configure_agent':
			agentSettings.set({
				...(action.enabled !== null && { enabled: action.enabled }),
				...(action.bridge_url !== null && { url: action.bridge_url.trim() }),
				...(action.client_token !== null && { token: action.client_token.trim() })
			});
			await syncAgent();
			return null;
		case 'prepare_transfer':
			return started(await transfers.prepare(action, actor));
		case 'submit_transfer':
			return started(transfers.submit(action.operation, actor, action.review ?? null));
		case 'cancel_transfer':
			return transfers.cancel(action.operation);
		case 'prepare_batch_transfer':
			return started(await transfers.prepareBatch(action, actor));
		case 'add_token':
			return addToken(action.chain_id, action.address);
		case 'delete_activity':
			// History's "Delete record": the feed tombstones it and drops the row.
			feed.deleteRecord(action.id);
			return null;
		case 'set_balance_privacy':
			// The eye is a toggle: pressed only when it is not already so.
			if (balance.view.hidden !== action.hidden) balance.togglePrivacy();
			return null;
		case 'set_network_space':
			// The device's space (spec 105): every list follows it.
			networkSpace.set(action.space);
			return null;
		case 'refresh_balances':
			// The home's refresh: balances read again, the feed looks again.
			balance.refresh(true, true);
			feed.focusTick();
			return null;
		case 'set_contact_favorite':
			contactsBook.dispatch({ type: 'toggle_favorite', address: action.address, now_ms: nowMs });
			return null;
		case 'add_network':
			return networks.addNetwork(action.chain_id, action.rpc_url);
		case 'set_network_endpoint':
			return networks.setNetworkEndpoint(action.chain_id, action.field, action.url);
		case 'remove_network':
			await networks.removeNetwork(action.chain_id);
			return null;
		case 'set_provider_key':
			await networks.setProviderKey(action.provider, action.key);
			return null;
		case 'set_service_endpoint':
			await networks.setServiceEndpoint(action.field, action.url);
			return null;
		case 'reset_service_endpoints':
			await networks.resetServiceEndpoints();
			return null;
	}
}
