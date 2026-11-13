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
import type { BatchView } from '$lib/core/generated/BatchView';
import type { ClearSignField } from '$lib/core/generated/ClearSignField';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import type { SignView } from '$lib/core/generated/SignView';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { IDLE_FEE_VIEW } from '$lib/flows/core/fee-quote.svelte';
import { liveBatchImport } from '$lib/flows/live-batch';
import {
	liveSendConfirm,
	liveSendForm,
	liveSendPick,
	type SendLiveInputs
} from '$lib/flows/live-send';
import type { NetworkSpace } from '$lib/network-space/network-space.svelte';
import { buildSigningModel, type SigningLiveInputs } from '$lib/signing/live';
import { INITIAL_CLEAR_VIEW, INITIAL_GUARD_VIEW } from '$lib/signing/core/sheet.svelte';
import { INITIAL_SIGN_VIEW } from '$lib/signing/core/sign-resident.svelte';
import { SEND_BASE } from './testing/send-view';
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

const flowState = (id: 'sd1' | 'sd2' | 'sd3') => buildFlowState(id, fm, identicon).base;
const sendPickModel = () => {
	const base = flowState('sd1');
	if (base.kind !== 'send-pick') throw new Error('sd1 is the coin list');
	return base.model;
};
const sendFormModel = () => {
	const base = flowState('sd2');
	if (base.kind !== 'send-form') throw new Error('sd2 is the form');
	return base.model;
};
const sendConfirmModel = () => {
	const base = flowState('sd3');
	if (base.kind !== 'send-confirm') throw new Error('sd3 is the review');
	return base.model;
};

// Spec 105: the testnet space has nothing of real value — no total, and no
// worth on a holding's row, whatever the coins' pegs say.
describe('the testnet space', () => {
	const NO_VALUE = 'Test assets · no real value';

	it('says so in the total’s place, and gives no row a worth', () => {
		const total = liveBalance(balance, COMMITTED, m, undefined, NO_VALUE);
		expect(total).toMatchObject({ state: 'no-value', noValueText: NO_VALUE });
		expect(total.integer).toBeUndefined();
		expect(total.currency).toBeUndefined();
		const priced = balance.tokens.find((t) => t.price_usd !== null)!;
		expect(liveAssetRow(priced, COMMITTED, m, false).fiat.kind).toBe('value');
		expect(liveAssetRow(priced, COMMITTED, m, false, true).fiat).toEqual({ kind: 'none' });
	});

	it('hidden stays hidden', () => {
		expect(liveBalance({ ...balance, hidden: true }, COMMITTED, m, undefined, NO_VALUE).state).toBe(
			'hidden'
		);
	});

	/**
	 * r18-9 (spec 105, `network_space::shows_fiat`): the testnet space shows
	 * NO fiat anywhere. Every money surface's model, built in the testnet
	 * space with the person's currency committed (CNY at 7.2 — a figure
	 * would be drawn in the mainnet space, checked first), names no currency:
	 * the home, the assets and a token's page and panel, an activity's
	 * detail, the balance sheets, Send (coins, form, review, a sweep, the
	 * speeds), the dApp signing sheet (an amount, the fee, the fee's speeds)
	 * and the batch importer — with a stablecoin priced at 1 and a relay fee
	 * coin the relay published a `usd_price` for.
	 */
	it('r18-9: every money surface in the testnet space names no currency — a stablecoin at 1, a relay fee priced', () => {
		// A currency's glyph anywhere, or its ISO code beside a figure ("USD Coin",
		// a contract's name, is not money).
		const CODES = '(?:USD|CNY|EUR|JPY|VND|KRW|HKD|GBP|IDR)';
		const NO_MONEY = new RegExp(`[$¥€£₫₩₹₽₺₴₱฿]|\\b${CODES}\\b\\s?\\d|\\d\\s?${CODES}\\b`);
		const stable = balance.tokens.find((t) => t.symbol === 'USDC')!;
		expect(stable.price_usd).toBe(1);
		const usdc: SendToken = {
			network: 'eth-mainnet',
			chain_id: 1,
			symbol: 'USDC',
			balance: '376.54321',
			decimals: 6,
			token_address: stable.token_address,
			price_usd: 1,
			logo_urls: [],
			spam: false
		};
		const relayFee = {
			symbol: 'USDC',
			contract: stable.token_address,
			decimals: 6,
			balance: '376543210',
			recipient: '0x1',
			usd_balance: '376.54',
			usd_price: '1',
			amount: '210000',
			insufficient: false,
			selected: true,
			spent_by_operation: false,
			short_with_amount: false,
			short: null
		};
		const quote = {
			chain_id: 1,
			total_wei: '2100000000000000',
			max_fee_per_gas: '1',
			network_fee_per_gas: '1',
			relayer_fee_per_gas: '0',
			bundler_gas_price: '1',
			in_band_gas_basis: '1',
			effective_gas_price: null,
			max_gas_price: null,
			total_gas: '1',
			deployed: true,
			tier: 'standard' as const,
			quoted: true,
			// A relay fee paid in the stablecoin, its price published (`usd_price`).
			fee_asset: {
				type: 'erc20' as const,
				token: stable.token_address!,
				decimals: 6,
				amount: '210000',
				symbol: 'USDC'
			},
			fee_recipient: '0x1'
		} satisfies FeeEstimateView;
		const fee: FeeView = {
			...IDLE_FEE_VIEW,
			fee: quote,
			options: [relayFee],
			confirm_fee_ready: true
		};
		const speed = {
			view: {
				tier: 'standard',
				picked: false,
				free: false,
				open: true,
				free_note: false,
				single: false,
				gas_price_line: null,
				options: [
					{ tier: 'fast', fee: quote, measuring: false, gas_price: null, selected: false },
					{ tier: 'standard', fee: quote, measuring: false, gas_price: null, selected: true }
				]
			} as unknown as FeeSpeedView,
			feeOptions: () => [relayFee]
		};
		const recipient = '0x' + 'ab'.repeat(20);
		const sendView = (over: Partial<SendView>): SendView =>
			({
				...SEND_BASE,
				tokens: [usdc],
				selected_token: usdc,
				recipient,
				amount: '25',
				token_amount: '25',
				confirm_amount: '25',
				fee: quote,
				fee_coin: null,
				...over
			}) as SendView;
		const sendInputs = (space: NetworkSpace, over: Partial<SendView> = {}): SendLiveInputs => ({
			send: sendView(over),
			fee,
			m: fm,
			currency: COMMITTED,
			space,
			identity: { name: 'Main', address: balance.address ?? '', identiconSvg: '<svg/>' },
			identicon,
			speed
		});
		const sm2 = resolveSigningMessages('en');
		const amountField = {
			label: 'Amount',
			value: '25 USDC',
			format: 'amount',
			token_address: stable.token_address,
			warning: false,
			unverified: false,
			role: 'send_amount',
			detail: false,
			expired: false,
			address: null,
			usd_value: 25,
			label_term: null,
			value_term: null
		} as ClearSignField;
		const signInputs = (space: NetworkSpace): SigningLiveInputs => ({
			sign: {
				...INITIAL_SIGN_VIEW,
				surface: 'sheet',
				confirm_gate_open: true,
				request: {
					id: 'req-1',
					method: 'eth_sendTransaction',
					kind: 'transaction',
					params_json: '[{"to":"0xdead","data":"0x","value":"0x0"}]',
					origin: 'https://app.example',
					dapp: null,
					chain_id: 1,
					signer_address: balance.address ?? '',
					first_party: false
				}
			} as SignView,
			clear: {
				...INITIAL_CLEAR_VIEW,
				resolved: true,
				surface: 'clear_sign',
				result: {
					intent: 'Send USDC',
					intent_term: null,
					contract_name: 'USD Coin',
					owner: null,
					fields: [amountField],
					risk: 'normal',
					contract_address: stable.token_address,
					verified: true,
					provenance: 'built_in',
					sign_type: 'transaction',
					partial: false,
					best_effort: false,
					to_own_token: false,
					terms_off_chain: false,
					reading: {
						source: 'vela',
						descriptor_owner: null,
						descriptor_path: null,
						contract_named_by: null,
						contradictions: [],
						selector_name: null,
						goes_elsewhere: false,
						dust_minimum: false,
						allowance: false,
						allowance_adds: false,
						shape_intent: null,
						collection_name: null
					}
				}
			} as ClearSigningView,
			guard: INITIAL_GUARD_VIEW,
			fee,
			feeOpen: false,
			speed,
			currency: COMMITTED,
			space,
			m: sm2,
			identity: { name: 'Main', address: balance.address ?? '', identiconSvg: '<svg/>' },
			identicon
		});
		const batchInputs = (space: NetworkSpace) => ({
			batch: {
				opened: true,
				unit: 'token' as const,
				fiat_code: 'CNY',
				raw_text: '',
				file_name: null,
				busy: false,
				file_error: false,
				file_failure: null,
				template_saved: false,
				priced: true,
				rate_status: 'ready',
				rate_input: '',
				rate_edited: false,
				preview: [],
				over_cap: false,
				errors: [],
				rejected: 0,
				recipient_count: 1,
				total_token: '25',
				total_fiat: '180',
				over_balance: false,
				can_apply: true,
				recipients: [],
				applied: false
			} as unknown as BatchView,
			m: fm,
			symbol: 'USDC',
			balance: '376.54321',
			identicon,
			space
		});
		const batchModel = () => {
			const state = buildFlowState('sd2c', fm, identicon);
			if (state.sheet?.kind !== 'batch-import') throw new Error('sd2c raises the importer');
			return state.sheet.model;
		};

		/** Every money surface's model, in `space`. */
		const surfaces = (space: NetworkSpace): Record<string, unknown> => {
			const flows = { ...flowInputs(COMMITTED), space };
			const page = withLiveFlow(buildFlowState('t2', fm, identicon), {
				...flows,
				selectedToken: stable
			}).sheet;
			const assets = withLiveFlow(buildFlowState('t1', fm, identicon), flows).base;
			const priced = items.filter((item) => item.priced);
			const sweep = sendInputs(space, {
				multi_select_mode: true,
				multi_selected_ids: ['1:' + stable.token_address],
				multi_chain_id: 1,
				multi_specs: [{ chain_id: 1, token_address: stable.token_address, amount: '25' }]
			} as unknown as Partial<SendView>);
			const sheet = buildSigningModel(signInputs(space))!;
			return {
				panel: liveAssetDetail(
					stable,
					{ balance, currency: COMMITTED, m, feed, space },
					buildDesktopState('d1', m, identicon).panels.assetDetail
				),
				tokenPage: page,
				assets,
				activityDetail: priced.map((item) =>
					liveTxDetail(item, {
						m: fm,
						wm: m,
						currency: COMMITTED,
						space,
						hidden: false,
						identicon,
						now: NOW
					})
				),
				balanceDetail: liveBalanceDetail(balance, COMMITTED, rescue, m.balance.unpriced, space),
				unreachable: liveUnreachable(balance, COMMITTED, rescue, space),
				sendCoins: liveSendPick(sendPickModel(), sendInputs(space)).rows,
				sendForm: (() => {
					const form = liveSendForm(sendFormModel(), sendInputs(space));
					return [form.amount, form.fee, form.speed];
				})(),
				sendReview: liveSendConfirm(sendConfirmModel(), sendInputs(space)),
				sendSweep: liveSendConfirm(sendConfirmModel(), sweep),
				signing: [sheet.fee, sheet.blocks],
				batch: (() => {
					const built = liveBatchImport(batchModel(), batchInputs(space));
					return [built.units, built.total];
				})()
			};
		};

		// The mainnet space draws these figures — the check below is not vacuous.
		const mainnet = surfaces('mainnet');
		for (const name of [
			'panel',
			'tokenPage',
			'activityDetail',
			'balanceDetail',
			'unreachable',
			'sendCoins',
			'sendForm',
			'sendReview',
			'signing',
			'batch'
		]) {
			expect(JSON.stringify(mainnet[name]), `${name} draws fiat in the mainnet space`).toMatch(
				NO_MONEY
			);
		}
		// The testnet space: not one currency, on any of them.
		const testnet = surfaces('testnet');
		for (const [name, model] of Object.entries(testnet)) {
			const text = JSON.stringify(model);
			expect(
				text.match(NO_MONEY)?.[0] ?? null,
				`${name}: ${text.slice(Math.max(0, (text.match(NO_MONEY)?.index ?? 0) - 300), (text.match(NO_MONEY)?.index ?? 0) + 100)}`
			).toBeNull();
		}
		// …and the mainnet space does price the relay's fee coin, on Send and the sheet.
		expect(JSON.stringify(mainnet.sendForm)).toMatch(/valueFiat":"≈ CN¥/);
		expect(JSON.stringify(mainnet.signing)).toMatch(/valueFiat":"≈ CN¥/);
		// The coin figures are still there: a fee in its coin is not money.
		expect(JSON.stringify(testnet.sendForm)).toContain('0.21 USDC');
		expect(JSON.stringify(testnet.signing)).toContain('25 USDC');
	});
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
			integer: 'CN¥6,852',
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
		// The third row is BNB Chain's, kept while it does not answer (fix-balance).
		expect(withheld.map((row) => row.fiat)).toEqual([
			{ kind: 'value', text: MONEY_PENDING },
			{ kind: 'value', text: MONEY_PENDING },
			{ kind: 'value', text: MONEY_PENDING }
		]);
		// A token amount is not fiat: it does not wait.
		expect(withheld.map((row) => row.balance)).toEqual(['418.25', '376.5432', '0.5']);
		expect(rows(COMMITTED).map((row) => row.fiat)).toEqual([
			{ kind: 'value', text: 'CN¥3,011.40' },
			{ kind: 'value', text: 'CN¥2,711.11' },
			{ kind: 'value', text: 'CN¥1,130.40' }
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

	/**
	 * §18 item 2 (the desktop's DESKTOP-1): a stored record this browser
	 * cannot read is said under the switcher's list, on every surface of it.
	 */
	it('account_switcher: a stored record this browser cannot read is said under the list', () => {
		const said = { ...accounts(ON_ITS_WAY), unreadable: 1 };
		expect(sm.accounts.unreadable).not.toMatch(/^onboarding\./);
		expect(liveAccountsSheet(said, sm.accounts).unreadable).toBe(sm.accounts.unreadable);
		expect(liveAccountsSheet(accounts(ON_ITS_WAY), sm.accounts).unreadable).toBeUndefined();
		const phone = withLiveAccounts(buildSettingsMobileState('st2', sm, identicon), said, sm);
		expect(phone.accountsSheet.unreadable).toBe(sm.accounts.unreadable);
		const wide = withLiveAccountsDesktop(
			buildSettingsDesktopState('dst1', sm, identicon),
			said,
			sm
		);
		expect(wide.account.unreadable).toBe(sm.accounts.unreadable);
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
		expect(liveUnreachable(balance, COMMITTED, rescue).rows[0].line).toBe('Last seen CN¥1,130.40');
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
			recipient: null,
			save_failed: false
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
		expect(detail(received, COMMITTED).fiat).toBe('≈ CN¥2,084.40');
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

	it('money is formatted in one place: nothing else pairs a currency symbol with digits', () => {
		// `fiatSymbol` is the core's rule for what turns a code into "¥";
		// outside the formatter it is only the picker's row badge (a symbol
		// beside a currency's name, no figure).
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
			[...source.matchAll(/fiatSymbol\(/g)].map(() => file)
		);
		// The formatter's one lookup, and the picker's two branches (the rate
		// sources' list, and the drawn eight while none has answered).
		expect(calls.sort()).toEqual([
			'src/lib/settings/live.ts',
			'src/lib/settings/live.ts',
			'src/lib/wallet/live.ts'
		]);
		// …and the web keeps no table of its own to disagree with it.
		expect(readFileSync('src/lib/settings/fixtures.ts', 'utf8')).not.toMatch(/glyph: '[^']/);
		// …and the formatter itself answers "withheld" before anything else.
		const formatter = users[0][1];
		const body = formatter.slice(formatter.indexOf('export function moneyParts('));
		expect(body.indexOf('if (!currency.committed) return null;')).toBeGreaterThan(0);
		expect(body.indexOf('if (!currency.committed) return null;')).toBeLessThan(
			body.indexOf('leadingSymbol(')
		);
	});
});
