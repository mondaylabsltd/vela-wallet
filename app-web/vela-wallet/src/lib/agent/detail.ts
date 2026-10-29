/**
 * What an agent's request names, in the person's words (spec 103): the
 * approval sheet says which contact, network, setting or record it changes —
 * by the names the wallet shows, never a raw id ("analytics: false",
 * "rates", "chain 59144").
 *
 * The core decides what a request must show (`intent_rules::approval_detail_in`:
 * the wallet a switch leaves and the one it goes to, a site's networks); the
 * words are the locale's, resolved on the server (`AgentMessages`).
 */
import type { IntentAction } from '$lib/core/generated/IntentAction';
import type { IntentPreference } from '$lib/core/generated/IntentPreference';
import { fill } from '$lib/wallet/messages';
import type { AgentMessages } from './messages';

export type DetailContext = {
	messages: AgentMessages;
	/** The page's locale, for names the browser knows (languages, currencies). */
	locale: string;
	contacts: { address: string; name?: string | null }[];
	groups: { id: string; name: string }[];
	/** The networks the wallet knows, and the one an add is checking. */
	networks: { chain_id: number; display_name: string }[];
	checking: { chain_id: number; name: string } | null;
	/** The activity, for a record's amount and counterparty. */
	records: {
		id: string;
		user_op_hash: string;
		value: string;
		symbol: string;
		to: string;
		to_name: string | null;
	}[];
	/** The space words ("Mainnet", "Testnet"), once read. */
	space: { mainnet: string; testnet: string } | null;
	/** The core's detail, for what it says better from the wallet (accounts, sites). */
	core: (action: IntentAction) => string | null;
};

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/** `0x066e…d704`. */
function short(address: string): string {
	const a = address.trim();
	return a.length <= 12 ? a : `${a.slice(0, 6)}…${a.slice(-4)}`;
}

/** The number and date formats, shown as Settings shows them: by example. */
const EXAMPLES: Record<string, string> = {
	'number_format.comma_dot': '1,234.56',
	'number_format.dot_comma': '1.234,56',
	'number_format.space_comma': '1 234,56',
	'date_format.ymd_slash': '2026/12/31',
	'date_format.mdy_slash': '12/31/2026',
	'date_format.dmy_slash': '31/12/2026',
	'date_format.dmy_dot': '31.12.2026',
	'date_format.iso': '2026-12-31'
};

/** The browser's name for `code` of `type`, or `undefined`. */
function displayName(
	type: 'language' | 'currency',
	code: string,
	locale: string
): string | undefined {
	try {
		const name = new Intl.DisplayNames([locale], { type }).of(code);
		return name === undefined || name === code ? undefined : name;
	} catch {
		return undefined;
	}
}

/** The six text sizes' factors (`prefs::TEXT_SCALE_LEVELS`), said as a percentage. */
const TEXT_SCALES: Record<string, number> = {
	compact: 0.82,
	small: 0.91,
	standard: 1,
	comfortable: 1.1,
	large: 1.22,
	xlarge: 1.35
};

function preferenceValue(preference: IntentPreference, ctx: DetailContext): string {
	const { messages: m, locale } = ctx;
	const key = `${preference.kind}.${String(preference.value)}`;
	const said = m.values[key] ?? EXAMPLES[key];
	if (said !== undefined) return said;
	switch (preference.kind) {
		case 'analytics':
			return preference.value ? m.on : m.off;
		case 'language':
			// A language by its own name, as the language picker lists it.
			return displayName('language', preference.value, preference.value) ?? preference.value;
		case 'currency': {
			const name = displayName('currency', preference.value, locale);
			return name === undefined ? preference.value : `${name} (${preference.value})`;
		}
		case 'text_scale': {
			const factor = TEXT_SCALES[preference.value];
			if (factor === undefined) return preference.value;
			try {
				return new Intl.NumberFormat(locale, { style: 'percent' }).format(factor);
			} catch {
				return `${Math.round(factor * 100)}%`;
			}
		}
		default:
			return String(preference.value);
	}
}

function network(chainId: number, ctx: DetailContext): string {
	const known =
		ctx.networks.find((row) => row.chain_id === chainId)?.display_name.trim() ||
		(ctx.checking?.chain_id === chainId ? ctx.checking.name.trim() : '');
	return known || fill(ctx.messages.chain, { id: chainId });
}

/** Where a URL goes: the request's own address, whole — it is what is approved. */
const to = (url: string | null | undefined) => (url && url.trim() ? ` → ${url.trim()}` : '');

/** What `action` names, in the person's words, or `null` when its name says it all. */
export function approvalDetail(action: IntentAction, ctx: DetailContext): string | null {
	const m = ctx.messages;
	switch (action.type) {
		case 'delete_contact': {
			const known = ctx.contacts.find((c) => same(c.address, action.address));
			return known?.name || short(action.address);
		}
		case 'delete_group':
			return ctx.groups.find((g) => g.id === action.id)?.name ?? action.id;
		case 'clear_storage_item':
			return m.storage[action.item] ?? action.item;
		case 'set_preference':
			return fill(m.pair, {
				label: m.preferences[action.preference.kind] ?? action.preference.kind,
				value: preferenceValue(action.preference, ctx)
			});
		case 'set_network_space': {
			if (!ctx.space) return null;
			return action.space === 'testnet' ? ctx.space.testnet : ctx.space.mainnet;
		}
		case 'set_balance_privacy':
			return action.hidden ? m.hideBalance : m.showBalance;
		case 'add_network':
			return `${network(action.chain_id, ctx)}${to(action.rpc_url)}`;
		case 'remove_network':
			return network(action.chain_id, ctx);
		case 'set_network_endpoint':
			return `${network(action.chain_id, ctx)} · ${m.fields[action.field] ?? action.field}${to(action.url)}`;
		case 'set_only_my_node':
			// "Gnosis (100) · Only use my node: Off" — either way is the person's.
			return `${network(action.chain_id, ctx)} · ${fill(m.pair, {
				label: m.onlyMine,
				value: action.on ? m.on : m.off
			})}`;
		case 'set_service_endpoint':
			return `${m.services[action.field] ?? action.field}${to(action.url)}`;

		case 'delete_activity': {
			const record = ctx.records.find((r) => r.id === action.id || same(r.user_op_hash, action.id));
			if (!record) return short(action.id);
			const amount = `${record.value} ${record.symbol}`.trim();
			const who = record.to_name?.trim() || (record.to ? short(record.to) : '');
			return who ? `${amount} · ${who}` : amount;
		}
		default:
			// The core's, read against the wallet: the wallet a switch leaves and
			// the one it goes to, a wallet to remove by its name (spec 107).
			return ctx.core(action);
	}
}

/** The sheet's sentence: the action, what it names, and the hint — in the locale's punctuation. */
export function approvalMessage(name: string, detail: string | null, m: AgentMessages): string {
	return detail === null || detail === ''
		? fill(m.approveAction, { action: name, hint: m.approveHint })
		: fill(m.approveLine, { action: name, detail, hint: m.approveHint });
}
