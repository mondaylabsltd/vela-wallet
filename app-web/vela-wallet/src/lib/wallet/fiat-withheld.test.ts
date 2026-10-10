/**
 * No fiat figure before the display currency commits — on every surface
 * (PR 3 notes 9 and 27; the core's rule in `display_currency.rs`).
 *
 * `CurrencyView.committed == false` means the view is the USD/1 placeholder,
 * not the person's currency. The home total waited for it since the last
 * round; the rule now has no surface it skips, and the web makes every fiat
 * figure in one place (`moneyParts`) that answers "withheld" first.
 *
 * The twelve surfaces are the core's (`FIAT_SURFACES`). Each is replayed from
 * a REAL balance and feed (the shared privacy fixture's shown side: priced
 * holdings, an unreachable network last seen holding something, the switcher's
 * cached totals, a priced transfer, a split and a dApp row) through the web's
 * own builders, withheld and committed, and held to one assertion
 * (`expectWithheld`): no figure in any money, the pending mark where the
 * figure will be, and the same lines as once it lands.
 *
 * Send's and the signing sheet's cases live beside their builders
 * (`flows/live-send.test.ts`, `signing/live.test.ts`); the last test here
 * checks that all twelve are covered. The rooms themselves — that a figure
 * landing moves nothing on screen — are measured in a browser
 * (`BalanceDisplay`, `FeeRow`, `AccountsSheetBody` `.svelte.test.ts`) and end
 * to end (`e2e/currency-withheld.e2e.ts`).
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { SessionAccountRow } from '$lib/core/generated/SessionAccountRow';
import {
	resolveContactsMessages,
	resolveSettingsMessages,
	resolveWalletFlowMessages,
	resolveWalletMessages
} from '$lib/i18n/engine.server';
import { liveContactDetail } from '$lib/contacts/live';
import { buildFlowState } from '$lib/flows/fixtures';
import { withLiveFlow } from '$lib/flows/live';
import {
	buildDesktopState as buildSettingsDesktopState,
	buildMobileState as buildSettingsMobileState
} from '$lib/settings/fixtures';
import {
	liveAccountsSheet,
	liveBalanceDetail,
	liveUnreachable,
	pickRescueMessages,
	withLiveAccounts,
	withLiveAccountsDesktop,
	type LiveAccountsInput
} from '$lib/settings/live';
import { buildDesktopState, buildMobileState } from './fixtures';
import {
	liveActivityGroups,
	liveAssetDetail,
	liveAssetRow,
	liveBalance,
	MONEY_PENDING,
	withLiveWallet,
	withLiveWalletDesktop
} from './live';
import { liveTxDetail } from './live-detail';
import {
	COMMITTED,
	expectWithheld,
	FIAT_FIGURE,
	FIAT_SURFACES,
	NOT_READ,
	ON_ITS_WAY,
	shapeOf
} from './testing/fiat-withheld';

const FIXTURE = JSON.parse(
	readFileSync('../../rust/crates/vela-core/tests/fixtures/privacy-hidden.json', 'utf8')
) as { shown: { balance: BalanceView; feed: FeedView } };
const { balance, feed } = FIXTURE.shown;

const m = resolveWalletMessages('en');
const fm = resolveWalletFlowMessages('en');
const cm = resolveContactsMessages('en');
const sm = resolveSettingsMessages('en');
const rescue = pickRescueMessages(sm);
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const NOW = 1_700_000_000_000;

const items: FeedItem[] = feed.rows.flatMap((row) => (row.type === 'item' ? [row.item] : []));
const flowInputs = (currency: CurrencyView) => ({
	balance,
	currency,
	m,
	emptyCopy: undefined,
	feed,
	fm
});

/** The two accounts the switcher lists: the fixture's own and its neighbour. */
const sessionRows: SessionAccountRow[] = [
	balance.address ?? '',
	'0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
].map((address, index) => ({
	index,
	account: {
		id: `acct-${index}`,
		name: index === 0 ? 'Main' : 'Spare',
		address,
		public_key_hex: '04',
		created_at_iso: '2026-01-01T00:00:00.000Z',
		keys: [],
		signing_domain: 'getvela.app',
		signing_venue: { type: 'in_vela' as const }
	}
}));
const accounts = (currency: CurrencyView): LiveAccountsInput => ({
	rows: sessionRows,
	activeIndex: 0,
	balances: new Map(balance.switcher.balances.map((row) => [row.address.toLowerCase(), row.usd])),
	hidden: false,
	currency,
	identicon
});

describe('no fiat figure before the display currency commits — every surface the core names', () => {
	it('home_total: the skeleton, the currency on its way named — never the placeholder’s dollars', () => {
		expectWithheld('home_total', (currency) => liveBalance(balance, currency, m), {
			sameShape: false
		});
		// The hero's stand-in is its skeleton, whatever the balance already knows.
		expect(balance.display_total_usd).toBe(951.79321);
		expect(liveBalance(balance, NOT_READ, m)).toMatchObject({ state: 'loading' });
		expect(liveBalance(balance, NOT_READ, m).currency).toBeUndefined();
		expect(liveBalance(balance, ON_ITS_WAY, m)).toMatchObject({
			state: 'loading',
			currency: 'CNY'
		});
		expect(liveBalance(balance, NOT_READ, m).integer).toBeUndefined();
		// Committed: the figure, once, in the right money (951.79321 × 7.2).
		expect(liveBalance(balance, COMMITTED, m)).toMatchObject({
			state: 'normal',
			currency: 'CNY',
			integer: '¥6,852',
			decimals: '91'
		});
		// The whole home, both layouts: not one figure anywhere on it.
		for (const view of [NOT_READ, ON_ITS_WAY]) {
			const inputs = { balance, currency: view, m, feed };
			const phone = withLiveWallet(buildMobileState('h1', m, identicon), inputs);
			const wide = withLiveWalletDesktop(buildDesktopState('d1', m, identicon), inputs);
			expect(JSON.stringify([phone.balance, phone.assetRows, phone.activityGroups])).not.toMatch(
				FIAT_FIGURE
			);
			expect(JSON.stringify([wide.balance, wide.assetRows, wide.activityGroups])).not.toMatch(
				FIAT_FIGURE
			);
		}
	});

	it('holdings: each row’s worth is the pending mark, its token amount drawn as always', () => {
		const rows = (currency: CurrencyView) =>
			balance.tokens.map((token) => liveAssetRow(token, currency, m, false));
		expectWithheld('holdings', rows);
		const withheld = rows(ON_ITS_WAY);
		expect(withheld.map((row) => row.fiat)).toEqual([
			{ kind: 'value', text: MONEY_PENDING },
			{ kind: 'value', text: MONEY_PENDING }
		]);
		// A token amount is not fiat: it does not wait.
		expect(withheld.map((row) => row.balance)).toEqual(['418.25', '376.5432']);
		expect(rows(COMMITTED).map((row) => row.fiat)).toEqual([
			{ kind: 'value', text: '¥3,011.40' },
			{ kind: 'value', text: '¥2,711.11' }
		]);
	});

	it('account_switcher: every account’s total, and their sum', () => {
		expectWithheld('account_switcher', (currency) =>
			liveAccountsSheet(accounts(currency), sm.accounts)
		);
		const sheet = liveAccountsSheet(accounts(ON_ITS_WAY), sm.accounts);
		expect(sheet.rows.map((row) => row.amount)).toEqual([MONEY_PENDING, MONEY_PENDING]);
		expect(sheet.summary).toContain(MONEY_PENDING);
	});

	it('settings_total: Settings’ own accounts page, phone and wide', () => {
		expectWithheld('settings_total', (currency) => {
			const phone = withLiveAccounts(
				buildSettingsMobileState('st2', sm, identicon),
				accounts(currency),
				sm
			);
			const wide = withLiveAccountsDesktop(
				buildSettingsDesktopState('dst1', sm, identicon),
				accounts(currency),
				sm
			);
			return [phone.accountsSheet.summary, phone.accountsSheet.rows, wide.account];
		});
	});

	it('token_detail: its worth and its price, on the page and in the wide panel', () => {
		for (const token of balance.tokens) {
			expectWithheld('token_detail', (currency) => {
				const page = withLiveFlow(buildFlowState('t2', fm, identicon), {
					...flowInputs(currency),
					selectedToken: token
				}).sheet;
				const panel = liveAssetDetail(
					token,
					{ balance, currency, m, feed },
					buildDesktopState('d1', m, identicon).panels.assetDetail
				);
				return [page, panel.token, panel.facts];
			});
		}
		const sheet = withLiveFlow(buildFlowState('t2', fm, identicon), {
			...flowInputs(ON_ITS_WAY),
			selectedToken: balance.tokens[0]
		}).sheet;
		if (sheet?.kind !== 'token-detail') throw new Error('t2 raises the token detail');
		// The worth is withheld; the balance beside it is a token amount.
		expect(sheet.model.fiat).toBe(MONEY_PENDING);
		expect(sheet.model.balance).toBe('418.25 xDAI');
		// "1 xDAI = …": the price is a fiat figure too.
		expect(JSON.stringify(sheet.model.facts)).toContain(MONEY_PENDING);
	});

	it('assets: the Assets page’s rows', () => {
		expectWithheld('assets', (currency) => {
			const state = withLiveFlow(buildFlowState('t1', fm, identicon), flowInputs(currency));
			return state.base.kind === 'assets' ? state.base.model.rows : null;
		});
	});

	it('balance_detail: the sheet’s totals per network, and the unreachable list’s "last seen"', () => {
		expectWithheld('balance_detail', (currency) => [
			liveBalanceDetail(balance, currency, rescue, m.balance.unpriced),
			liveUnreachable(balance, currency, rescue)
		]);
		// BNB Chain was last seen holding $157: said as "Last seen …" until
		// the currency is the person's.
		expect(balance.unreachable_networks[0]).toMatchObject({ chain_id: 56, last_seen_usd: 157 });
		expect(liveUnreachable(balance, ON_ITS_WAY, rescue).rows[0].line).toBe(
			`Last seen ${MONEY_PENDING}`
		);
		expect(liveUnreachable(balance, COMMITTED, rescue).rows[0].line).toBe('Last seen ¥1,130.40');
		const sheet = liveBalanceDetail(balance, ON_ITS_WAY, rescue, m.balance.unpriced);
		expect(sheet.summary).toContain(MONEY_PENDING);
		expect(sheet.done.map((row) => row.amount)).toEqual([MONEY_PENDING, MONEY_PENDING]);
	});

	it('activity_row: the web’s rows carry a token amount and no fiat — nothing to withhold, nothing drawn', () => {
		const contacts: ContactsView = {
			loaded: true,
			contacts: [
				{
					address: '0xdddddddddddddddddddddddddddddddddddddddd',
					name: 'Bea',
					resolved_name: null,
					resolved_source: null,
					kind: 'unknown',
					favorite: false,
					note: null,
					tx_count: 1,
					last_used_ms: 0,
					first_seen_ms: 0,
					source: 'manual'
				}
			],
			sections: [{ letter: 'B', addresses: ['0xdddddddddddddddddddddddddddddddddddddddd'] }],
			groups: [],
			last_import: null,
			import_failure: null,
			import_failure_key: null,
			export: null,
			recipient: null
		};
		expectWithheld(
			'activity_row',
			(currency) => {
				const history = withLiveFlow(buildFlowState('a1', fm, identicon), flowInputs(currency));
				const contact = liveContactDetail(contacts.contacts[0]!, contacts, cm, identicon, {
					feed,
					contactAddress: contacts.contacts[0]!.address,
					rowMessages: { activity: m.activity },
					allActivity: true,
					now: NOW
				});
				return [
					liveActivityGroups(feed.home_rows, m, false),
					history.base.kind === 'history' ? history.base.model.groups : null,
					contact.rows
				];
			},
			{ figure: false }
		);
	});

	it('activity_detail: a transfer’s worth and a dApp row’s, under the amount', () => {
		const detail = (item: FeedItem, currency: CurrencyView) =>
			liveTxDetail(item, { m: fm, wm: m, currency, hidden: false, identicon, now: NOW });
		// Every priced row: its "≈" line is there before and after.
		const priced = items.filter((item) => item.priced);
		expect(priced.map((item) => item.id)).toEqual(
			expect.arrayContaining(['received', 'sent', 'swap'])
		);
		expectWithheld('activity_detail', (currency) => priced.map((item) => detail(item, currency)));
		const received = items.find((item) => item.id === 'received')!;
		expect(detail(received, ON_ITS_WAY).fiat).toBe(`≈ ${MONEY_PENDING}`);
		expect(detail(received, ON_ITS_WAY).amount).toBe('+289.5 USDT');
		expect(detail(received, COMMITTED).fiat).toBe('≈ ¥2,084.40');
		// A row the core has no price for has no "≈" line either way — never "≈ …".
		const unpriced = items.filter((item) => !item.priced);
		expect(unpriced.length).toBeGreaterThan(0);
		for (const item of unpriced) {
			expect(detail(item, ON_ITS_WAY).fiat).toBe('');
			expect(shapeOf(detail(item, ON_ITS_WAY))).toEqual(shapeOf(detail(item, COMMITTED)));
		}
	});
});

describe('the list of surfaces is the core’s, and each has a case', () => {
	it('names exactly the surfaces `display_currency.rs` names, in its order', () => {
		const source = readFileSync('../../rust/crates/vela-core/src/app/display_currency.rs', 'utf8');
		const block = /pub const FIAT_SURFACES: \[&str; (\d+)\] = \[([^\]]+)\];/.exec(source);
		expect(block, 'FIAT_SURFACES is declared in the core').not.toBeNull();
		const named = [...block![2].matchAll(/"([a-z_]+)"/g)].map((match) => match[1]);
		expect(named).toHaveLength(Number(block![1]));
		expect(named).toEqual([...FIAT_SURFACES]);
	});

	it('every surface is held to the rule by a test that replays it', () => {
		const tests = [
			'src/lib/wallet/fiat-withheld.test.ts',
			'src/lib/flows/live-send.test.ts',
			'src/lib/signing/live.test.ts'
		].map((file) => readFileSync(file, 'utf8'));
		for (const surface of FIAT_SURFACES) {
			const call = new RegExp(`expectWithheld\\(\\s*'${surface}'`);
			const covered = tests.some((source) => call.test(source));
			expect(covered, `${surface} has no expectWithheld case`).toBe(true);
		}
	});

	it('money is formatted in one place: nothing else pairs a currency glyph with digits', () => {
		// `currencyGlyph` is what turns a code into "¥"; outside the formatter
		// it is only the picker's row glyph (a symbol beside a currency's
		// name, no figure).
		const users = [
			'src/lib/wallet/live.ts',
			'src/lib/wallet/live-detail.ts',
			'src/lib/flows/live.ts',
			'src/lib/flows/live-send.ts',
			'src/lib/flows/fee-line.ts',
			'src/lib/signing/live.ts',
			'src/lib/settings/live.ts'
		].map((file) => [file, readFileSync(file, 'utf8')] as const);
		const calls = users.flatMap(([file, source]) =>
			[...source.matchAll(/currencyGlyph\(/g)].map(() => file)
		);
		expect(calls.sort()).toEqual(['src/lib/settings/live.ts', 'src/lib/wallet/live.ts']);
		// …and the formatter itself answers "withheld" before anything else.
		const formatter = users[0][1];
		const body = formatter.slice(formatter.indexOf('export function moneyParts('));
		expect(body.indexOf('if (!currency.committed) return null;')).toBeGreaterThan(0);
		expect(body.indexOf('if (!currency.committed) return null;')).toBeLessThan(
			body.indexOf('currencyGlyph(')
		);
	});
});
