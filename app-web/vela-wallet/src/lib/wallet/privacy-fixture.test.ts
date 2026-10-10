/**
 * Balance privacy, replayed from the core's shared fixture (correctness batch
 * item 2): `rust/crates/vela-core/tests/fixtures/privacy-hidden.json` is a REAL
 * balance and feed, driven through the core's two machines, once shown and
 * once hidden, with figures whose digit runs appear nowhere else. Every shell
 * feeds both through every surface builder it has and asserts the same four
 * things (`tests/app_privacy.rs`):
 *
 * 1. shown: each `forbidden` run appears in some masked surface's output — so
 *    the check below is not vacuous;
 * 2. hidden: no masked surface's output contains any of them, and its figures
 *    read `mask` (`balance_mask` on the home total);
 * 3. hidden: the visible surfaces keep their figures (Send — in
 *    `flows/live-send.test.ts`, beside the send builders it drives);
 * 4. a feed row's figure masks exactly when the core's `figure_maskable` says.
 *
 * The web has no Explore network picker (`network_picker`): nothing to replay.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { ContactsView } from '$lib/core/generated/ContactsView';
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
	liveAccountsSheet,
	liveBalanceDetail,
	liveUnreachable,
	pickRescueMessages
} from '$lib/settings/live';
import { BALANCE_MASK, MASK } from './fixtures';
import { liveActivityGroups, liveActivityRow, liveAssetRow, liveBalance } from './live';
import { liveTxDetail } from './live-detail';

interface Side {
	balance: BalanceView;
	feed: FeedView;
}

const FIXTURE = JSON.parse(
	readFileSync('../../rust/crates/vela-core/tests/fixtures/privacy-hidden.json', 'utf8')
) as {
	forbidden: string[];
	mask: string;
	balance_mask: string;
	masked_surfaces: string[];
	visible_surfaces: string[];
	figure_maskable: Record<string, boolean>;
	shown: Side;
	hidden: Side;
};

const m = resolveWalletMessages('en');
const fm = resolveWalletFlowMessages('en');
const cm = resolveContactsMessages('en');
const sm = resolveSettingsMessages('en');
const USD = { code: 'USD', rate: 1, committed: true, pending: null };
const identicon = (seed: string) => `<svg data-seed="${seed}"></svg>`;
const NOW = 1_700_000_000_000;

function feedItems(feed: FeedView): FeedItem[] {
	return feed.rows.flatMap((row) => (row.type === 'item' ? [row.item] : []));
}

/** The two accounts the switcher lists: the fixture's own and its neighbour. */
function sessionRows(balance: BalanceView): SessionAccountRow[] {
	const other = '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
	return [balance.address ?? '', other].map((address, index) => ({
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
}

/** Bea, whose page shows what passed between you (`contact_rows`). */
const BEA = '0xdddddddddddddddddddddddddddddddddddddddd';
const CONTACTS: ContactsView = {
	loaded: true,
	contacts: [
		{
			address: BEA,
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
	sections: [{ letter: 'B', addresses: [BEA] }],
	groups: [],
	last_import: null,
	import_failure: null,
	import_failure_key: null,
	export: null,
	recipient: null
};

/**
 * Every masked surface the web draws, each as the text its builder hands the
 * screen. The flag each one reads is the one the page passes it: the balance's
 * for holdings and totals, the feed's own (`FeedView.hidden`) for feed rows.
 */
function maskedSurfaces({ balance, feed }: Side): Record<string, string> {
	const emptyCopy = (() => {
		const t4 = buildFlowState('t4', fm, identicon);
		return t4.base.kind === 'assets' ? t4.base.model.empty : undefined;
	})();
	const flowInputs = { balance, currency: USD, m, emptyCopy, feed, fm };
	const assets = withLiveFlow(buildFlowState('t1', fm, identicon), flowInputs);
	const history = withLiveFlow(buildFlowState('a1', fm, identicon), flowInputs);
	const tokenDetails = balance.tokens.map((selectedToken) => {
		const state = withLiveFlow(buildFlowState('t2', fm, identicon), {
			...flowInputs,
			selectedToken
		});
		return state.sheet;
	});
	const detail = (item: FeedItem) =>
		liveTxDetail(item, {
			m: fm,
			wm: m,
			currency: USD,
			hidden: feed.hidden,
			identicon,
			now: NOW
		});
	const items = feedItems(feed);
	const transfers = items.filter((item) => item.dapp == null);
	const dapps = items.filter((item) => item.dapp != null);
	const rescue = pickRescueMessages(sm);
	const switcher = liveAccountsSheet(
		{
			rows: sessionRows(balance),
			activeIndex: 0,
			balances: new Map(
				balance.switcher.balances.map((row) => [row.address.toLowerCase(), row.usd])
			),
			hidden: balance.switcher.hidden,
			currency: USD,
			identicon
		},
		sm.accounts
	);
	const contact = liveContactDetail(CONTACTS.contacts[0]!, CONTACTS, cm, identicon, {
		feed,
		contactAddress: BEA,
		rowMessages: { activity: m.activity },
		allActivity: true,
		now: NOW
	});
	return {
		home_total: JSON.stringify(liveBalance(balance, USD, m)),
		holdings: JSON.stringify(balance.tokens.map((t) => liveAssetRow(t, USD, m, balance.hidden))),
		assets: JSON.stringify(assets.base.kind === 'assets' ? assets.base.model.rows : null),
		token_detail: JSON.stringify(tokenDetails),
		// The home draws the core's own cut (`home_rows`, issue 469).
		home_activity: JSON.stringify(liveActivityGroups(feed.home_rows, m, feed.hidden)),
		history: JSON.stringify(history.base.kind === 'history' ? history.base.model.groups : null),
		transfer_detail: JSON.stringify(transfers.map(detail)),
		dapp_detail: JSON.stringify(dapps.map(detail)),
		contact_activity: JSON.stringify(contact.rows),
		account_switcher: JSON.stringify(switcher),
		balance_detail: JSON.stringify([
			liveBalanceDetail(balance, USD, rescue, m.balance.unpriced),
			liveUnreachable(balance, USD, rescue)
		]),
		// The core withholds it while hidden (`FeedView.toast`); the page draws
		// only what it is handed.
		receipt_toast: JSON.stringify(feed.toast)
	};
}

describe('the shared hidden-balance fixture, through every web surface builder', () => {
	it('names the surfaces the core names, and the web has every masked one but Explore', () => {
		const drawn = Object.keys(maskedSurfaces(FIXTURE.shown));
		expect(FIXTURE.masked_surfaces.filter((s) => !drawn.includes(s))).toEqual(['network_picker']);
		expect(drawn.every((s) => FIXTURE.masked_surfaces.includes(s))).toBe(true);
		expect(FIXTURE.mask).toBe(MASK);
		expect(FIXTURE.balance_mask).toBe(BALANCE_MASK);
	});

	it('shown: every forbidden figure is drawn somewhere — the check is not vacuous', () => {
		const all = Object.values(maskedSurfaces(FIXTURE.shown)).join('\n');
		for (const run of FIXTURE.forbidden) expect(all, run).toContain(run);
	});

	it('hidden: no masked surface draws any of them', () => {
		const surfaces = maskedSurfaces(FIXTURE.hidden);
		for (const [surface, text] of Object.entries(surfaces)) {
			for (const run of FIXTURE.forbidden) {
				expect(text.includes(run), `${surface} draws ${run}`).toBe(false);
			}
		}
	});

	it('hidden: the masked figures read the mask — the hero the wider one', () => {
		const { balance, feed } = FIXTURE.hidden;
		const hero = liveBalance(balance, USD, m);
		expect(hero.state).toBe('hidden');
		expect(hero.integer).toBe(BALANCE_MASK);
		for (const row of balance.tokens.map((t) => liveAssetRow(t, USD, m, balance.hidden))) {
			expect(row.balance).toBe(MASK);
			expect(row.fiat).toEqual({ kind: 'masked' });
		}
		const switcher = JSON.parse(maskedSurfaces(FIXTURE.hidden).account_switcher) as {
			rows: { amount: string }[];
			summary: string;
		};
		expect(switcher.rows.map((r) => r.amount)).toEqual([MASK, MASK]);
		expect(switcher.summary).toContain(MASK);
		// The swap's "received" masks with the row (a dApp row's second figure).
		const swap = feedItems(feed).find((item) => item.id === 'swap')!;
		expect(liveActivityRow(swap, m, feed.hidden, NOW).received?.amount).toBe(MASK);
		expect(feed.toast).toBeNull();
	});

	// PR 3 item 12: one rule for a masked token figure — the amount is the
	// mask, the unit is kept ("•••• USDT"). It says what kind of money without
	// saying how much. A hidden transfer's detail read "•••• xDAI" on iOS and
	// a bare "••••" on Android and on the web.
	it('hidden: a masked token figure keeps its unit, wherever the two are one string', () => {
		const { balance, feed } = FIXTURE.hidden;
		const detail = (id: string) =>
			liveTxDetail(
				feedItems(feed).find((item) => item.id === id)!,
				{
					m: fm,
					wm: m,
					currency: USD,
					hidden: feed.hidden,
					identicon,
					now: NOW
				}
			);
		// A transfer in, a transfer out, and a dApp's swap.
		expect(detail('received').amount).toBe(`${MASK} USDT`);
		expect(detail('sent').amount).toBe(`${MASK} USDC`);
		expect(detail('swap').amount).toBe(`${MASK} USDC`);
		// A permit's cap is money too (the core's `figure_maskable`)…
		expect(detail('permit').amount).toMatch(new RegExp(`^${MASK} \\S+$`));
		// …an unlimited one is a risk to see, and a signature has no figure.
		expect(detail('permit-unlimited').amount).not.toContain(MASK);
		expect(detail('signature').amount).toBe('');
		// The fiat worth has no unit apart from its figure: the bare mask.
		expect(detail('received').fiat).toBe(MASK);
		// A token's own detail: its balance, masked, still says which token.
		for (const token of balance.tokens) {
			const sheet = withLiveFlow(buildFlowState('t2', fm, identicon), {
				balance,
				currency: USD,
				m,
				emptyCopy: undefined,
				feed,
				fm,
				selectedToken: token
			}).sheet;
			if (sheet?.kind !== 'token-detail') throw new Error('t2 raises the token detail');
			expect(sheet.model.balance, token.symbol).toBe(`${MASK} ${token.symbol}`);
		}
		// The rows draw the amount and the unit apart, and always kept it.
		const row = liveActivityRow(
			feedItems(feed).find((i) => i.id === 'received')!,
			m,
			true,
			NOW
		);
		expect([row.amount, row.unit]).toEqual([MASK, 'USDT']);
	});

	it('a row’s figure masks exactly when the core says it is money', () => {
		const { feed } = FIXTURE.hidden;
		const items = feedItems(feed);
		expect(items.map((item) => item.id).sort()).toEqual(
			Object.keys(FIXTURE.figure_maskable).sort()
		);
		for (const item of items) {
			const maskable = FIXTURE.figure_maskable[item.id];
			expect(item.figure_maskable, item.id).toBe(maskable);
			const row = liveActivityRow(item, m, true, NOW);
			expect(row.masked, item.id).toBe(maskable);
			if (maskable) expect(row.amount, item.id).toBe(MASK);
			else expect(row.amount, item.id).not.toBe(MASK);
			// Shown, nothing masks.
			expect(liveActivityRow(item, m, false, NOW).masked, item.id).toBe(false);
		}
	});
});
